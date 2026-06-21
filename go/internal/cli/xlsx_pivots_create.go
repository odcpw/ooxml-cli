package cli

import (
	"fmt"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

type XLSXPivotsCreateResult struct {
	File              string   `json:"file"`
	Name              string   `json:"name"`
	SourceSheet       string   `json:"sourceSheet"`
	SourceRange       string   `json:"sourceRange"`
	TargetSheet       string   `json:"targetSheet"`
	Location          string   `json:"location"`
	CacheID           int      `json:"cacheId"`
	CacheDefURI       string   `json:"cacheDefinitionUri"`
	CacheRecordURI    string   `json:"cacheRecordsUri"`
	PivotTableURI     string   `json:"pivotTableUri"`
	RowFields         []string `json:"rowFields,omitempty"`
	ColFields         []string `json:"colFields,omitempty"`
	PageFields        []string `json:"pageFields,omitempty"`
	ValueFields       []string `json:"valueFields,omitempty"`
	Warnings          []string `json:"warnings,omitempty"`
	Output            string   `json:"output,omitempty"`
	DryRun            bool     `json:"dryRun"`
	ValidateCommand   string   `json:"validateCommand,omitempty"`
	PivotsListCommand string   `json:"pivotsListCommand,omitempty"`
}

var (
	xlsxPivotsCreateSheet       string
	xlsxPivotsCreateRange       string
	xlsxPivotsCreateTable       string
	xlsxPivotsCreateTargetSheet string
	xlsxPivotsCreateAnchor      string
	xlsxPivotsCreateName        string
	xlsxPivotsCreateRows        string
	xlsxPivotsCreateCols        string
	xlsxPivotsCreateFilters     string
	xlsxPivotsCreateValues      string
	xlsxPivotsCreateExpectRange string
	xlsxPivotsCreateMaxCells    int
)

var xlsxPivotsCreateCmd = &cobra.Command{
	Use:   "create <file>",
	Short: "Author a new PivotTable from a table or range",
	Long:  "Create a PivotTable from a table or range source with row, column, filter, and value fields. The cache is set to refresh on load so Excel/LibreOffice compute the layout when the workbook opens.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		rowFields := splitCommaList(xlsxPivotsCreateRows)
		colFields := splitCommaList(xlsxPivotsCreateCols)
		pageFields := splitCommaList(xlsxPivotsCreateFilters)
		if len(rowFields) == 0 && len(colFields) == 0 {
			return InvalidArgsError("specify at least one --rows or --cols field")
		}
		if strings.TrimSpace(xlsxPivotsCreateValues) == "" {
			return InvalidArgsError("specify at least one --values field (name or name:agg)")
		}
		valueSpecs, err := parsePivotValueFields(splitCommaList(xlsxPivotsCreateValues))
		if err != nil {
			return err
		}
		source, matrix, err := loadXLSXRangeOrTableSourceForCLI(filePath, xlsxPivotsCreateSheet, xlsxPivotsCreateRange, xlsxPivotsCreateTable, xlsxPivotsCreateMaxCells)
		if err != nil {
			return err
		}
		if xlsxPivotsCreateExpectRange != "" && !strings.EqualFold(source.Range, xlsxPivotsCreateExpectRange) {
			return NewCLIErrorf(ExitInvalidArgs, "source range mismatch: expected %s but found %s", xlsxPivotsCreateExpectRange, source.Range)
		}
		srcRange, err := address.ParseRange(source.Range)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "invalid source range: %v", err)
		}
		var anchor address.CellRef
		anchorText := strings.TrimSpace(xlsxPivotsCreateAnchor)
		if anchorText == "" {
			// Default to two columns right of the source so the pivot never
			// lands on top of the source data when placed on the same sheet.
			_, minRow, maxCol, _ := srcRange.Bounds()
			anchor = address.CellRef{Column: maxCol + 2, Row: minRow}
		} else {
			anchor, err = address.ParseCell(anchorText)
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "invalid --anchor: %v", err)
			}
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}
		writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
		if err != nil {
			return err
		}
		var result *XLSXPivotsCreateResult
		if err := writer.Write(func(pkg opc.PackageSession) error {
			workbook, err := xlsxinspect.ParseWorkbook(pkg)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
			}
			targetSelector := strings.TrimSpace(xlsxPivotsCreateTargetSheet)
			if targetSelector == "" {
				targetSelector = source.Sheet
			}
			targetRef, err := selectXLSXSheet(workbook.Sheets, targetSelector)
			if err != nil {
				return err
			}
			if err := requireXLSXWorksheetRef(targetRef); err != nil {
				return err
			}
			createResult, err := mutate.CreatePivot(&mutate.CreatePivotRequest{
				Package:      pkg,
				WorkbookURI:  workbook.PartURI,
				SourceSheet:  source.Sheet,
				SourceRange:  srcRange,
				SourceCells:  matrix,
				TargetSheet:  targetRef,
				TargetAnchor: anchor,
				Name:         xlsxPivotsCreateName,
				RowFields:    rowFields,
				ColFields:    colFields,
				PageFields:   pageFields,
				ValueFields:  valueSpecs,
			})
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "failed to create pivot: %v", err)
			}
			destinationFile := mutationOutputPathForResult(filePath, mutOpts)
			result = &XLSXPivotsCreateResult{
				File:           filePath,
				Name:           createResult.Name,
				SourceSheet:    source.Sheet,
				SourceRange:    source.Range,
				TargetSheet:    targetRef.Name,
				Location:       createResult.Location,
				CacheID:        createResult.CacheID,
				CacheDefURI:    createResult.CacheDefURI,
				CacheRecordURI: createResult.CacheRecordURI,
				PivotTableURI:  createResult.PivotTableURI,
				RowFields:      createResult.RowFields,
				ColFields:      createResult.ColFields,
				PageFields:     createResult.PageFields,
				ValueFields:    createResult.ValueFields,
				Warnings:       createResult.Warnings,
				Output:         destinationFile,
				DryRun:         mutOpts != nil && mutOpts.DryRun,
			}
			if destinationFile != "" {
				result.ValidateCommand = xlsxValidateCommand(destinationFile)
				result.PivotsListCommand = fmt.Sprintf("ooxml --json xlsx pivots list %s", pptxXLSXCommandArg(destinationFile))
			}
			return nil
		}); err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeJSONResult(cmd, result, "pivots create")
		}
		return writeXLSXOutput(cmd, []byte(fmt.Sprintf("created pivot %s on %s at %s", result.Name, result.TargetSheet, result.Location)))
	},
}

// splitCommaList splits a comma-separated flag value into trimmed, non-empty items.
func splitCommaList(value string) []string {
	var out []string
	for _, part := range strings.Split(value, ",") {
		if p := strings.TrimSpace(part); p != "" {
			out = append(out, p)
		}
	}
	return out
}

// parsePivotValueFields parses "Amount" or "Amount:sum" specs.
func parsePivotValueFields(specs []string) ([]mutate.PivotValueSpec, error) {
	var out []mutate.PivotValueSpec
	for _, raw := range specs {
		raw = strings.TrimSpace(raw)
		if raw == "" {
			continue
		}
		name := raw
		agg := "sum"
		if i := strings.LastIndex(raw, ":"); i >= 0 {
			name = strings.TrimSpace(raw[:i])
			agg = strings.TrimSpace(raw[i+1:])
		}
		if name == "" {
			return nil, InvalidArgsError("value field name cannot be empty")
		}
		out = append(out, mutate.PivotValueSpec{Name: name, Aggregation: agg})
	}
	if len(out) == 0 {
		return nil, InvalidArgsError("specify at least one --values field")
	}
	return out, nil
}

func init() {
	f := xlsxPivotsCreateCmd.Flags()
	f.StringVar(&xlsxPivotsCreateSheet, "sheet", "", "source sheet number (1-based) or exact name")
	f.StringVar(&xlsxPivotsCreateRange, "range", "", "source A1 range with a header row")
	f.StringVar(&xlsxPivotsCreateTable, "table", "", "source table name (alternative to --range)")
	f.StringVar(&xlsxPivotsCreateTargetSheet, "target-sheet", "", "sheet to place the pivot on (default: source sheet)")
	f.StringVar(&xlsxPivotsCreateAnchor, "anchor", "", "top-left anchor cell for the pivot table (default: just right of the source)")
	f.StringVar(&xlsxPivotsCreateName, "name", "", "pivot table name (default: PivotTableN)")
	f.StringVar(&xlsxPivotsCreateRows, "rows", "", "row field names (by source header), comma-separated")
	f.StringVar(&xlsxPivotsCreateCols, "cols", "", "column field names, comma-separated")
	f.StringVar(&xlsxPivotsCreateFilters, "filters", "", "page/filter field names, comma-separated")
	f.StringVar(&xlsxPivotsCreateValues, "values", "", "value fields as name or name:agg (sum,count,average,max,min,...), comma-separated")
	f.StringVar(&xlsxPivotsCreateExpectRange, "expect-source-range", "", "guard: require the resolved source range to match")
	f.IntVar(&xlsxPivotsCreateMaxCells, "max-cells", 100000, "maximum source cells to read (0 for unlimited)")
	AddMutationFlags(xlsxPivotsCreateCmd)
	xlsxPivotsCmd.AddCommand(xlsxPivotsCreateCmd)
}
