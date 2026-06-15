package cli

import (
	"fmt"
	"os"
	"strconv"
	"strings"
	"unicode/utf8"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/spf13/cobra"
)

type ReplaceTextFromXLSXResult struct {
	File        string                  `json:"file"`
	Output      string                  `json:"output,omitempty"`
	DryRun      bool                    `json:"dryRun,omitempty"`
	Source      XLSXRangeSource         `json:"source"`
	Text        ReplaceTextFromXLSXText `json:"text"`
	Destination PPTXShapeDestination    `json:"destination"`
	PPTXBridgeReadbackCommands
}

type ReplaceTextFromXLSXText struct {
	Mode         string `json:"mode"`
	FormulaMode  string `json:"formulaMode"`
	RowSeparator string `json:"rowSeparator"`
	ColSeparator string `json:"colSeparator"`
	Chars        int    `json:"chars"`
	Value        string `json:"value"`
}

type ReplaceTextFromXLSXDestination = PPTXShapeDestination

var (
	replaceTextFromXLSXSlide       int
	replaceTextFromXLSXTarget      string
	replaceTextFromXLSXWorkbook    string
	replaceTextFromXLSXSheet       string
	replaceTextFromXLSXRange       string
	replaceTextFromXLSXMaxCells    int
	replaceTextFromXLSXFormulaMode string
	replaceTextFromXLSXMode        string
	replaceTextFromXLSXRowSep      string
	replaceTextFromXLSXColSep      string
)

var replaceTextFromXLSXCmd = &cobra.Command{
	Use:   "text-from-xlsx <file>",
	Short: "Replace PPTX text from an XLSX range",
	Long: `Replace one targetable slide shape or placeholder with text read from an XLSX range.

Range values are joined row-major. The default separators produce tab-delimited
rows separated by newlines, which is useful for compact pasted spreadsheet text.
Use --row-sep and --col-sep to change that behavior; escape sequences such as
\n and \t are supported.`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if replaceTextFromXLSXSlide < 1 {
			return InvalidArgsError("--slide must be >= 1")
		}
		if strings.TrimSpace(replaceTextFromXLSXTarget) == "" {
			return InvalidArgsError("--target is required")
		}
		if strings.TrimSpace(replaceTextFromXLSXWorkbook) == "" {
			return InvalidArgsError("--workbook is required")
		}
		if _, err := os.Stat(replaceTextFromXLSXWorkbook); err != nil {
			return FileNotFoundError(replaceTextFromXLSXWorkbook)
		}
		if strings.TrimSpace(replaceTextFromXLSXSheet) == "" {
			return InvalidArgsError("--sheet is required")
		}
		if strings.TrimSpace(replaceTextFromXLSXRange) == "" {
			return InvalidArgsError("--range is required")
		}
		formulaMode, err := normalizeXLSXFormulaMode(replaceTextFromXLSXFormulaMode, "--formula-mode")
		if err != nil {
			return err
		}
		mode, err := normalizeReplaceTextFromXLSXMode(replaceTextFromXLSXMode)
		if err != nil {
			return err
		}
		rowSep, err := decodeTextSeparatorFlag(replaceTextFromXLSXRowSep, "--row-sep")
		if err != nil {
			return err
		}
		colSep, err := decodeTextSeparatorFlag(replaceTextFromXLSXColSep, "--col-sep")
		if err != nil {
			return err
		}

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performReplaceTextFromXLSX(filePath, mutOpts, mode, formulaMode, rowSep, colSep)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputReplaceTextFromXLSXJSON(cmd, result)
		}
		return outputReplaceTextFromXLSXText(cmd, result)
	},
}

func performReplaceTextFromXLSX(filePath string, mutOpts *MutationOptions, mode, formulaMode, rowSep, colSep string) (*ReplaceTextFromXLSXResult, error) {
	source, text, err := loadReplaceTextFromXLSXSource(formulaMode, rowSep, colSep)
	if err != nil {
		return nil, err
	}

	writer, err := NewMutationWriter(filePath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *ReplaceTextFromXLSXResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		request := &mutate.ReplaceTextRequest{
			Package:     pkg,
			SlideNumber: replaceTextFromXLSXSlide,
			Target:      replaceTextFromXLSXTarget,
			NewText:     text,
			Mode:        mode,
		}
		if err := mutate.ReplaceText(request); err != nil {
			return mapReplaceTextFromXLSXMutationError(err, replaceTextFromXLSXTarget)
		}

		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		destination, err := collectReplaceTextFromXLSXDestination(pkg, replaceTextFromXLSXSlide, replaceTextFromXLSXTarget, destinationFile)
		if err != nil {
			return err
		}
		result = &ReplaceTextFromXLSXResult{
			File:   filePath,
			Output: destinationFile,
			DryRun: mutOpts.DryRun,
			Source: *source,
			Text: ReplaceTextFromXLSXText{
				Mode:         mode,
				FormulaMode:  formulaMode,
				RowSeparator: rowSep,
				ColSeparator: colSep,
				Chars:        utf8.RuneCountInString(text),
				Value:        text,
			},
			Destination: *destination,
		}
		result.PPTXBridgeReadbackCommands = pptxBridgeReadbackCommands(destinationFile, replaceTextFromXLSXSlide, func(path string) string {
			return pptxShapeTextReadbackCommand(path, replaceTextFromXLSXSlide, destination.PrimarySelector)
		})
		return nil
	}); err != nil {
		if cliErr, ok := AsCLIError(err); ok {
			return nil, cliErr
		}
		return nil, NewCLIErrorf(ExitUnexpected, "failed to replace text from XLSX: %v", err)
	}
	if result == nil {
		return nil, NewCLIErrorf(ExitUnexpected, "text-from-xlsx did not produce a result")
	}
	return result, nil
}

func loadReplaceTextFromXLSXSource(formulaMode, rowSep, colSep string) (*XLSXRangeSource, string, error) {
	rangeRef, err := address.ParseRange(replaceTextFromXLSXRange)
	if err != nil {
		return nil, "", NewCLIErrorf(ExitInvalidArgs, "invalid --range: %v", err)
	}
	if err := checkXLSXRangeMaxCells(rangeRef, replaceTextFromXLSXMaxCells); err != nil {
		return nil, "", err
	}
	source, matrix, err := readXLSXRangeSourceForCLI(replaceTextFromXLSXWorkbook, replaceTextFromXLSXSheet, rangeRef)
	if err != nil {
		return nil, "", err
	}
	values := xlsxRangeStringsFromMatrix(matrix, formulaMode)
	return source, joinXLSXTextMatrix(values, rowSep, colSep), nil
}

func joinXLSXTextMatrix(values [][]string, rowSep, colSep string) string {
	rows := make([]string, len(values))
	for rowIdx, row := range values {
		rows[rowIdx] = strings.Join(row, colSep)
	}
	return strings.Join(rows, rowSep)
}

func normalizeReplaceTextFromXLSXMode(value string) (string, error) {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "", "plain-text":
		return "plain-text", nil
	case "preserve-format":
		return "preserve-format", nil
	default:
		return "", InvalidArgsError("--mode must be plain-text or preserve-format")
	}
}

func decodeTextSeparatorFlag(value, flagName string) (string, error) {
	if !strings.Contains(value, `\`) {
		return value, nil
	}
	quoted := `"` + strings.ReplaceAll(value, `"`, `\"`) + `"`
	decoded, err := strconv.Unquote(quoted)
	if err != nil {
		return "", InvalidArgsError(fmt.Sprintf("%s contains invalid escape sequence: %v", flagName, err))
	}
	return decoded, nil
}

func collectReplaceTextFromXLSXDestination(pkg opc.PackageSession, slide int, targetSelector string, destinationFile string) (*ReplaceTextFromXLSXDestination, error) {
	return collectPPTXShapeDestination(pkg, slide, targetSelector, destinationFile, true, false)
}

func mapReplaceTextFromXLSXMutationError(err error, target string) error {
	if err == nil {
		return nil
	}
	msg := err.Error()
	if strings.Contains(msg, "target not found") {
		return TargetNotFoundError(target)
	}
	if strings.Contains(msg, "ambiguous target") || strings.Contains(msg, "non-text") {
		return InvalidArgsError(msg)
	}
	return err
}

func outputReplaceTextFromXLSXJSON(cmd *cobra.Command, result *ReplaceTextFromXLSXResult) error {
	data, err := marshalWithConfig(GetGlobalConfig(cmd), result)
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal text-from-xlsx JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputReplaceTextFromXLSXText(cmd *cobra.Command, result *ReplaceTextFromXLSXResult) error {
	text := fmt.Sprintf("replaced slide %d %s from %s!%s (%dx%d)", result.Destination.Slide, result.Destination.PrimarySelector, result.Source.Sheet, result.Source.Range, result.Source.Rows, result.Source.Cols)
	return writeCLIOutput(cmd, []byte(text))
}

func init() {
	replaceTextFromXLSXCmd.Flags().IntVarP(&replaceTextFromXLSXSlide, "slide", "s", 0, "slide number (1-based, required)")
	replaceTextFromXLSXCmd.Flags().StringVar(&replaceTextFromXLSXTarget, "target", "", "target selector such as title, body:1, shape:3, or ~Shape Name (required)")
	replaceTextFromXLSXCmd.Flags().StringVar(&replaceTextFromXLSXWorkbook, "workbook", "", "source XLSX workbook path (required)")
	replaceTextFromXLSXCmd.Flags().StringVar(&replaceTextFromXLSXSheet, "sheet", "", "source sheet selector (required)")
	replaceTextFromXLSXCmd.Flags().StringVar(&replaceTextFromXLSXRange, "range", "", "source A1 range (required)")
	replaceTextFromXLSXCmd.Flags().IntVar(&replaceTextFromXLSXMaxCells, "max-cells", 100000, "maximum source cells to read (0 for unlimited)")
	replaceTextFromXLSXCmd.Flags().StringVar(&replaceTextFromXLSXFormulaMode, "formula-mode", "value", "formula handling: value or formula")
	replaceTextFromXLSXCmd.Flags().StringVar(&replaceTextFromXLSXMode, "mode", "plain-text", "replacement mode: plain-text or preserve-format")
	replaceTextFromXLSXCmd.Flags().StringVar(&replaceTextFromXLSXRowSep, "row-sep", "\n", "separator between source rows; escape sequences like \\n are supported")
	replaceTextFromXLSXCmd.Flags().StringVar(&replaceTextFromXLSXColSep, "col-sep", "\t", "separator between source columns; escape sequences like \\t are supported")
	AddMutationFlags(replaceTextFromXLSXCmd)
	replaceCmd.AddCommand(replaceTextFromXLSXCmd)
}
