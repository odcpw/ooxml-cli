package cli

import (
	"fmt"
	"os"
	"strconv"
	"strings"
	"unicode/utf8"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/spf13/cobra"
)

type ReplaceTextMapFromXLSXResult struct {
	File         string                          `json:"file"`
	Output       string                          `json:"output,omitempty"`
	DryRun       bool                            `json:"dryRun,omitempty"`
	Source       XLSXRangeSource                 `json:"source"`
	Map          ReplaceTextMapFromXLSXMap       `json:"map"`
	Replacements []ReplaceTextMapFromXLSXReplace `json:"replacements"`
	PPTXBridgeReadbackCommands
}

type ReplaceTextMapFromXLSXMap struct {
	Mode         string `json:"mode"`
	FormulaMode  string `json:"formulaMode"`
	Rows         int    `json:"rows"`
	Applied      int    `json:"applied"`
	SlideColumn  string `json:"slideColumn"`
	TargetColumn string `json:"targetColumn"`
	TextColumn   string `json:"textColumn"`
}

type ReplaceTextMapFromXLSXReplace struct {
	SourceRow   int                            `json:"sourceRow"`
	Slide       int                            `json:"slide"`
	Target      string                         `json:"target"`
	Chars       int                            `json:"chars"`
	Text        string                         `json:"text"`
	Destination ReplaceTextFromXLSXDestination `json:"destination"`
	PPTXBridgeReadbackCommands
}

type replaceTextMapRecord struct {
	SourceRow int
	Slide     int
	Target    string
	Text      string
}

var (
	replaceTextMapFromXLSXWorkbook          string
	replaceTextMapFromXLSXSheet             string
	replaceTextMapFromXLSXRange             string
	replaceTextMapFromXLSXTable             string
	replaceTextMapFromXLSXMaxCells          int
	replaceTextMapFromXLSXFormulaMode       string
	replaceTextMapFromXLSXMode              string
	replaceTextMapFromXLSXSlideCol          string
	replaceTextMapFromXLSXTargetCol         string
	replaceTextMapFromXLSXTextCol           string
	replaceTextMapFromXLSXExpectSourceRange string
)

var replaceTextMapFromXLSXCmd = &cobra.Command{
	Use:   "text-map-from-xlsx <file>",
	Short: "Replace multiple PPTX text targets from XLSX mapping rows",
	Long: `Replace multiple targetable PPTX shapes or placeholders from an XLSX mapping range or named workbook table.

The first source row is a header row. By default the command reads columns named
slide, target, and text. Column flags can be exact header names or 1-based
column numbers.`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if strings.TrimSpace(replaceTextMapFromXLSXWorkbook) == "" {
			return InvalidArgsError("--workbook is required")
		}
		if _, err := os.Stat(replaceTextMapFromXLSXWorkbook); err != nil {
			return FileNotFoundError(replaceTextMapFromXLSXWorkbook)
		}
		formulaMode, err := normalizeXLSXFormulaMode(replaceTextMapFromXLSXFormulaMode, "--formula-mode")
		if err != nil {
			return err
		}
		mode, err := normalizeReplaceTextFromXLSXMode(replaceTextMapFromXLSXMode)
		if err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performReplaceTextMapFromXLSX(filePath, mutOpts, mode, formulaMode)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputReplaceTextMapFromXLSXJSON(cmd, result)
		}
		return outputReplaceTextMapFromXLSXText(cmd, result)
	},
}

func performReplaceTextMapFromXLSX(filePath string, mutOpts *MutationOptions, mode, formulaMode string) (*ReplaceTextMapFromXLSXResult, error) {
	source, matrix, err := loadXLSXRangeOrTableSourceForCLI(
		replaceTextMapFromXLSXWorkbook,
		replaceTextMapFromXLSXSheet,
		replaceTextMapFromXLSXRange,
		replaceTextMapFromXLSXTable,
		replaceTextMapFromXLSXMaxCells,
	)
	if err != nil {
		return nil, err
	}
	if err := checkExpectedXLSXSourceRange(source.Range, replaceTextMapFromXLSXExpectSourceRange); err != nil {
		return nil, err
	}

	values := xlsxRangeStringsFromMatrix(matrix, formulaMode)
	records, columnSummary, err := replaceTextMapRecordsFromValues(values)
	if err != nil {
		return nil, err
	}

	writer, err := NewMutationWriter(filePath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *ReplaceTextMapFromXLSXResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", err)
		}
		for _, record := range records {
			if record.Slide > len(graph.Slides) {
				return InvalidArgsError(fmt.Sprintf("row %d: slide %d out of range (1-%d)", record.SourceRow, record.Slide, len(graph.Slides)))
			}
		}

		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		replacements := make([]ReplaceTextMapFromXLSXReplace, 0, len(records))
		for _, record := range records {
			request := &mutate.ReplaceTextRequest{
				Package:     pkg,
				SlideNumber: record.Slide,
				Target:      record.Target,
				NewText:     record.Text,
				Mode:        mode,
			}
			if err := mutate.ReplaceText(request); err != nil {
				return mapReplaceTextFromXLSXMutationError(err, fmt.Sprintf("row %d target %s", record.SourceRow, record.Target))
			}
			destination, err := collectReplaceTextFromXLSXDestination(pkg, record.Slide, record.Target, destinationFile)
			if err != nil {
				return err
			}
			replacement := ReplaceTextMapFromXLSXReplace{
				SourceRow:   record.SourceRow,
				Slide:       record.Slide,
				Target:      record.Target,
				Chars:       utf8.RuneCountInString(record.Text),
				Text:        record.Text,
				Destination: *destination,
			}
			replacement.PPTXBridgeReadbackCommands = pptxBridgeReadbackCommands(destinationFile, record.Slide, func(path string) string {
				return pptxShapeTextReadbackCommand(path, record.Slide, destination.PrimarySelector)
			})
			replacements = append(replacements, replacement)
		}
		result = &ReplaceTextMapFromXLSXResult{
			File:   filePath,
			Output: destinationFile,
			DryRun: mutOpts.DryRun,
			Source: *source,
			Map: ReplaceTextMapFromXLSXMap{
				Mode:         mode,
				FormulaMode:  formulaMode,
				Rows:         len(records),
				Applied:      len(replacements),
				SlideColumn:  columnSummary.slide,
				TargetColumn: columnSummary.target,
				TextColumn:   columnSummary.text,
			},
			Replacements: replacements,
		}
		result.PPTXBridgeReadbackCommands = pptxBridgeOutputVerificationCommands(destinationFile)
		return nil
	}); err != nil {
		if cliErr, ok := AsCLIError(err); ok {
			return nil, cliErr
		}
		return nil, NewCLIErrorf(ExitUnexpected, "failed to replace text map from XLSX: %v", err)
	}
	if result == nil {
		return nil, NewCLIErrorf(ExitUnexpected, "text-map-from-xlsx did not produce a result")
	}
	return result, nil
}

type replaceTextMapColumns struct {
	slide       string
	target      string
	text        string
	slideIndex  int
	targetIndex int
	textIndex   int
}

func replaceTextMapRecordsFromValues(values [][]string) ([]replaceTextMapRecord, replaceTextMapColumns, error) {
	if len(values) < 2 {
		return nil, replaceTextMapColumns{}, InvalidArgsError("source map must include a header row and at least one replacement row")
	}
	header := values[0]
	columns, err := resolveReplaceTextMapColumns(header)
	if err != nil {
		return nil, replaceTextMapColumns{}, err
	}

	records := make([]replaceTextMapRecord, 0, len(values)-1)
	for rowIndex := 1; rowIndex < len(values); rowIndex++ {
		row := values[rowIndex]
		slideText := strings.TrimSpace(row[columns.slideIndex])
		if slideText == "" {
			return nil, replaceTextMapColumns{}, InvalidArgsError(fmt.Sprintf("row %d: slide value is required", rowIndex+1))
		}
		slide, err := strconv.Atoi(slideText)
		if err != nil || slide < 1 {
			return nil, replaceTextMapColumns{}, InvalidArgsError(fmt.Sprintf("row %d: slide must be a positive integer", rowIndex+1))
		}
		target := strings.TrimSpace(row[columns.targetIndex])
		if target == "" {
			return nil, replaceTextMapColumns{}, InvalidArgsError(fmt.Sprintf("row %d: target value is required", rowIndex+1))
		}
		records = append(records, replaceTextMapRecord{
			SourceRow: rowIndex + 1,
			Slide:     slide,
			Target:    target,
			Text:      row[columns.textIndex],
		})
	}
	return records, columns, nil
}

func resolveReplaceTextMapColumns(header []string) (replaceTextMapColumns, error) {
	if len(header) == 0 {
		return replaceTextMapColumns{}, InvalidArgsError("source map header row is empty")
	}
	slideIndex, slideName, err := resolveReplaceTextMapColumn(header, replaceTextMapFromXLSXSlideCol, "--slide-col")
	if err != nil {
		return replaceTextMapColumns{}, err
	}
	targetIndex, targetName, err := resolveReplaceTextMapColumn(header, replaceTextMapFromXLSXTargetCol, "--target-col")
	if err != nil {
		return replaceTextMapColumns{}, err
	}
	textIndex, textName, err := resolveReplaceTextMapColumn(header, replaceTextMapFromXLSXTextCol, "--text-col")
	if err != nil {
		return replaceTextMapColumns{}, err
	}
	if slideIndex == targetIndex || slideIndex == textIndex || targetIndex == textIndex {
		return replaceTextMapColumns{}, InvalidArgsError("--slide-col, --target-col, and --text-col must resolve to distinct columns")
	}
	return replaceTextMapColumns{
		slide:       slideName,
		target:      targetName,
		text:        textName,
		slideIndex:  slideIndex,
		targetIndex: targetIndex,
		textIndex:   textIndex,
	}, nil
}

func resolveReplaceTextMapColumn(header []string, selector, flagName string) (int, string, error) {
	selector = strings.TrimSpace(selector)
	if selector == "" {
		return 0, "", InvalidArgsError(flagName + " is required")
	}
	if index, err := strconv.Atoi(selector); err == nil {
		if index < 1 || index > len(header) {
			return 0, "", InvalidArgsError(fmt.Sprintf("%s index %d out of range (1-%d)", flagName, index, len(header)))
		}
		return index - 1, nonEmpty(strings.TrimSpace(header[index-1]), selector), nil
	}

	normalizedSelector := normalizeReplaceTextMapHeader(selector)
	var matchedIndex = -1
	for idx, name := range header {
		if normalizeReplaceTextMapHeader(name) != normalizedSelector {
			continue
		}
		if matchedIndex >= 0 {
			return 0, "", InvalidArgsError(fmt.Sprintf("%s header %q is ambiguous", flagName, selector))
		}
		matchedIndex = idx
	}
	if matchedIndex < 0 {
		return 0, "", InvalidArgsError(fmt.Sprintf("%s header %q not found", flagName, selector))
	}
	return matchedIndex, strings.TrimSpace(header[matchedIndex]), nil
}

func normalizeReplaceTextMapHeader(value string) string {
	return strings.ToLower(strings.TrimSpace(value))
}

func outputReplaceTextMapFromXLSXJSON(cmd *cobra.Command, result *ReplaceTextMapFromXLSXResult) error {
	data, err := marshalWithConfig(GetGlobalConfig(cmd), result)
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal text-map-from-xlsx JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputReplaceTextMapFromXLSXText(cmd *cobra.Command, result *ReplaceTextMapFromXLSXResult) error {
	text := fmt.Sprintf("replaced %d text targets from %s!%s", result.Map.Applied, result.Source.Sheet, result.Source.Range)
	return writeCLIOutput(cmd, []byte(text))
}

func init() {
	replaceTextMapFromXLSXCmd.Flags().StringVar(&replaceTextMapFromXLSXWorkbook, "workbook", "", "source XLSX workbook path (required)")
	replaceTextMapFromXLSXCmd.Flags().StringVar(&replaceTextMapFromXLSXSheet, "sheet", "", "source sheet selector")
	replaceTextMapFromXLSXCmd.Flags().StringVar(&replaceTextMapFromXLSXRange, "range", "", "source A1 range")
	replaceTextMapFromXLSXCmd.Flags().StringVar(&replaceTextMapFromXLSXTable, "table", "", "source workbook table selector")
	replaceTextMapFromXLSXCmd.Flags().IntVar(&replaceTextMapFromXLSXMaxCells, "max-cells", 100000, "maximum source cells to read (0 for unlimited)")
	replaceTextMapFromXLSXCmd.Flags().StringVar(&replaceTextMapFromXLSXFormulaMode, "formula-mode", "value", "formula handling: value or formula")
	replaceTextMapFromXLSXCmd.Flags().StringVar(&replaceTextMapFromXLSXMode, "mode", "plain-text", "replacement mode: plain-text or preserve-format")
	replaceTextMapFromXLSXCmd.Flags().StringVar(&replaceTextMapFromXLSXSlideCol, "slide-col", "slide", "header name or 1-based column index containing slide numbers")
	replaceTextMapFromXLSXCmd.Flags().StringVar(&replaceTextMapFromXLSXTargetCol, "target-col", "target", "header name or 1-based column index containing PPTX target selectors")
	replaceTextMapFromXLSXCmd.Flags().StringVar(&replaceTextMapFromXLSXTextCol, "text-col", "text", "header name or 1-based column index containing replacement text")
	replaceTextMapFromXLSXCmd.Flags().StringVar(&replaceTextMapFromXLSXExpectSourceRange, "expect-source-range", "", "fail if the resolved XLSX source range differs from this A1 range")
	AddMutationFlags(replaceTextMapFromXLSXCmd)
	replaceCmd.AddCommand(replaceTextMapFromXLSXCmd)
}
