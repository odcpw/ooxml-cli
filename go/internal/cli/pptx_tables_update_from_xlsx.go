package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/spf13/cobra"
)

type PPTXTablesUpdateFromXLSXResult struct {
	File        string                      `json:"file"`
	Output      string                      `json:"output,omitempty"`
	DryRun      bool                        `json:"dryRun,omitempty"`
	Source      XLSXRangeSource             `json:"source"`
	Update      PPTXTablesUpdateFromXLSXRun `json:"update"`
	Destination PPTXTableSummary            `json:"destination"`
	PPTXBridgeReadbackCommands
}

type PPTXTablesUpdateFromXLSXRun struct {
	FormulaMode  string `json:"formulaMode"`
	UpdatedCells int    `json:"updatedCells"`
	ChangedCells int    `json:"changedCells"`
}

var (
	pptxTablesUpdateFromXLSXSlide             int
	pptxTablesUpdateFromXLSXTableID           int
	pptxTablesUpdateFromXLSXTarget            string
	pptxTablesUpdateFromXLSXWorkbook          string
	pptxTablesUpdateFromXLSXSheet             string
	pptxTablesUpdateFromXLSXRange             string
	pptxTablesUpdateFromXLSXTable             string
	pptxTablesUpdateFromXLSXMaxCells          int
	pptxTablesUpdateFromXLSXFormulaMode       string
	pptxTablesUpdateFromXLSXExpectSourceRange string
)

var pptxTablesUpdateFromXLSXCmd = &cobra.Command{
	Use:   "update-from-xlsx <file>",
	Short: "Refresh an existing PPTX table from an XLSX range or table",
	Long: `Refresh an existing PowerPoint table's plain text cell contents from a rectangular XLSX worksheet range or named workbook table.

The source and destination dimensions must match exactly. This command does not
resize the PPTX table and refuses merged PPTX tables in this first version.`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if pptxTablesUpdateFromXLSXSlide < 1 {
			return InvalidArgsError("--slide must be >= 1")
		}
		if strings.TrimSpace(pptxTablesUpdateFromXLSXWorkbook) == "" {
			return InvalidArgsError("--workbook is required")
		}
		if _, err := os.Stat(pptxTablesUpdateFromXLSXWorkbook); err != nil {
			return FileNotFoundError(pptxTablesUpdateFromXLSXWorkbook)
		}
		formulaMode, err := normalizeXLSXFormulaMode(pptxTablesUpdateFromXLSXFormulaMode, "--formula-mode")
		if err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performPPTXTablesUpdateFromXLSX(filePath, mutOpts, formulaMode)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputPPTXTablesJSON(cmd, result, "tables update-from-xlsx")
		}
		return writePPTXTablesUpdateFromXLSXText(cmd, result)
	},
}

func performPPTXTablesUpdateFromXLSX(filePath string, mutOpts *MutationOptions, formulaMode string) (*PPTXTablesUpdateFromXLSXResult, error) {
	source, matrix, err := loadXLSXRangeOrTableSourceForCLI(
		pptxTablesUpdateFromXLSXWorkbook,
		pptxTablesUpdateFromXLSXSheet,
		pptxTablesUpdateFromXLSXRange,
		pptxTablesUpdateFromXLSXTable,
		pptxTablesUpdateFromXLSXMaxCells,
	)
	if err != nil {
		return nil, err
	}
	if err := checkExpectedXLSXSourceRange(source.Range, pptxTablesUpdateFromXLSXExpectSourceRange); err != nil {
		return nil, err
	}
	data := xlsxRangeStringsFromMatrix(matrix, formulaMode)

	writer, err := NewMutationWriter(filePath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *PPTXTablesUpdateFromXLSXResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		slideRef, err := resolvePPTXSlideRef(pkg, pptxTablesUpdateFromXLSXSlide)
		if err != nil {
			return err
		}
		tableID, err := resolveRequiredPPTXTableTarget(pkg, pptxTablesUpdateFromXLSXSlide, pptxTablesUpdateFromXLSXTableID, pptxTablesUpdateFromXLSXTarget)
		if err != nil {
			return err
		}
		before, err := collectPPTXSingleTable(pkg, slideRef, tableID)
		if err != nil {
			return err
		}
		if before.Rows != source.Rows || before.Cols != source.Cols {
			return InvalidArgsError(fmt.Sprintf("source/destination dimension mismatch: source is %dx%d, destination table is %dx%d", source.Rows, source.Cols, before.Rows, before.Cols))
		}
		update, err := mutate.SetTableTextMatrix(&mutate.SetTableTextMatrixRequest{
			Package:  pkg,
			SlideRef: slideRef,
			TableID:  tableID,
			Data:     data,
		})
		if err != nil {
			return mapPPTXTableMutationError(err)
		}
		destination, err := collectPPTXSingleTable(pkg, slideRef, tableID)
		if err != nil {
			return err
		}
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		destination.File = destinationFile
		result = &PPTXTablesUpdateFromXLSXResult{
			File:   filePath,
			Output: destinationFile,
			DryRun: mutOpts.DryRun,
			Source: *source,
			Update: PPTXTablesUpdateFromXLSXRun{
				FormulaMode:  formulaMode,
				UpdatedCells: update.UpdatedCells,
				ChangedCells: update.ChangedCells,
			},
			Destination: *destination,
		}
		result.PPTXBridgeReadbackCommands = pptxBridgeReadbackCommands(destinationFile, destination.Slide, func(path string) string {
			return pptxTableReadbackCommand(path, destination.Slide, destination.PrimarySelector)
		})
		return nil
	}); err != nil {
		return nil, err
	}
	if result == nil {
		return nil, NewCLIErrorf(ExitUnexpected, "update-from-xlsx did not produce a result")
	}
	return result, nil
}

func checkExpectedXLSXSourceRange(actualRange, expectedRange string) error {
	expectedRange = strings.TrimSpace(expectedRange)
	if expectedRange == "" {
		return nil
	}
	ref, err := address.ParseRange(expectedRange)
	if err != nil {
		return NewCLIErrorf(ExitInvalidArgs, "invalid --expect-source-range: %v", err)
	}
	expected := ref.String()
	if actualRange != expected {
		return InvalidArgsError(fmt.Sprintf("--expect-source-range mismatch: source resolved to %s, expected %s", actualRange, expected))
	}
	return nil
}

func writePPTXTablesUpdateFromXLSXText(cmd *cobra.Command, result *PPTXTablesUpdateFromXLSXResult) error {
	text := fmt.Sprintf(
		"updated slide %d table %s from %s!%s (%dx%d, changed %d/%d)",
		result.Destination.Slide,
		result.Destination.PrimarySelector,
		result.Source.Sheet,
		result.Source.Range,
		result.Source.Rows,
		result.Source.Cols,
		result.Update.ChangedCells,
		result.Update.UpdatedCells,
	)
	return writeCLIOutput(cmd, []byte(text))
}

func init() {
	pptxTablesUpdateFromXLSXCmd.Flags().IntVarP(&pptxTablesUpdateFromXLSXSlide, "slide", "s", 0, "slide number (1-based, required)")
	pptxTablesUpdateFromXLSXCmd.Flags().IntVar(&pptxTablesUpdateFromXLSXTableID, "table-id", 0, "table graphic-frame shape ID")
	pptxTablesUpdateFromXLSXCmd.Flags().StringVar(&pptxTablesUpdateFromXLSXTarget, "target", "", "table selector (e.g., table:1, shape:2, ~Table 1)")
	pptxTablesUpdateFromXLSXCmd.Flags().StringVar(&pptxTablesUpdateFromXLSXWorkbook, "workbook", "", "source XLSX workbook path (required)")
	pptxTablesUpdateFromXLSXCmd.Flags().StringVar(&pptxTablesUpdateFromXLSXSheet, "sheet", "", "source sheet selector")
	pptxTablesUpdateFromXLSXCmd.Flags().StringVar(&pptxTablesUpdateFromXLSXRange, "range", "", "source A1 range")
	pptxTablesUpdateFromXLSXCmd.Flags().StringVar(&pptxTablesUpdateFromXLSXTable, "table", "", "source workbook table selector")
	pptxTablesUpdateFromXLSXCmd.Flags().IntVar(&pptxTablesUpdateFromXLSXMaxCells, "max-cells", 100000, "maximum source cells to read (0 for unlimited)")
	pptxTablesUpdateFromXLSXCmd.Flags().StringVar(&pptxTablesUpdateFromXLSXFormulaMode, "formula-mode", "value", "formula handling: value or formula")
	pptxTablesUpdateFromXLSXCmd.Flags().StringVar(&pptxTablesUpdateFromXLSXExpectSourceRange, "expect-source-range", "", "fail if the resolved XLSX source range differs from this A1 range")
	AddMutationFlags(pptxTablesUpdateFromXLSXCmd)
	tablesCmd.AddCommand(pptxTablesUpdateFromXLSXCmd)
}
