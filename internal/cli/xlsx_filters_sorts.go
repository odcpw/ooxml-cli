package cli

import (
	"fmt"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	xlsxmutate "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	xlsxtable "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/table"
	"github.com/spf13/cobra"
)

const filtersSortsHonesty = "Note: applying a filter or sort does NOT physically hide or reorder rows in the file. Excel/Calc re-evaluates the autoFilter/sortState when the workbook is opened."

var xlsxFiltersSortsCmd = &cobra.Command{
	Use:     "filters-sorts",
	Aliases: []string{"filters", "sort"},
	Short:   "Auto-filter and sort for table/range workflows",
	Long:    "Set/clear worksheet or table autoFilter, add/clear per-column filter criteria, and set/clear worksheet sortState. " + filtersSortsHonesty,
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

// ---- JSON shapes ----

type XLSXFilterColumnJSON struct {
	ColID        int                   `json:"colId"`
	Values       []string              `json:"values,omitempty"`
	CustomFilter *XLSXCustomFilterJSON `json:"customFilter,omitempty"`
}

type XLSXCustomFilterJSON struct {
	And      bool                            `json:"and"`
	Criteria []XLSXCustomFilterCriterionJSON `json:"criteria"`
}

type XLSXCustomFilterCriterionJSON struct {
	Operator string `json:"operator,omitempty"`
	Val      string `json:"val"`
}

type XLSXAutoFilterJSON struct {
	Ref     string                 `json:"ref"`
	Columns []XLSXFilterColumnJSON `json:"columns,omitempty"`
}

type XLSXSortConditionJSON struct {
	Ref        string `json:"ref"`
	Descending bool   `json:"descending"`
}

type XLSXSortStateJSON struct {
	Ref        string                  `json:"ref"`
	Conditions []XLSXSortConditionJSON `json:"conditions,omitempty"`
}

type XLSXFiltersSortsShowResult struct {
	File        string              `json:"file"`
	Sheet       string              `json:"sheet"`
	SheetNumber int                 `json:"sheetNumber"`
	Table       string              `json:"table,omitempty"`
	Note        string              `json:"note"`
	AutoFilter  *XLSXAutoFilterJSON `json:"autoFilter,omitempty"`
	SortState   *XLSXSortStateJSON  `json:"sortState,omitempty"`

	SetAutoFilterCommand   string `json:"setAutoFilterCommand,omitempty"`
	AddColumnFilterCommand string `json:"addColumnFilterCommand,omitempty"`
	SetSortCommand         string `json:"setSortCommand,omitempty"`
	ShowCommand            string `json:"showCommand,omitempty"`
}

type XLSXFiltersSortsMutationResult struct {
	File        string              `json:"file"`
	Sheet       string              `json:"sheet"`
	SheetNumber int                 `json:"sheetNumber"`
	Table       string              `json:"table,omitempty"`
	Action      string              `json:"action"`
	Ref         string              `json:"ref,omitempty"`
	Note        string              `json:"note"`
	AutoFilter  *XLSXAutoFilterJSON `json:"autoFilter,omitempty"`
	SortState   *XLSXSortStateJSON  `json:"sortState,omitempty"`
	Output      string              `json:"output,omitempty"`
	DryRun      bool                `json:"dryRun"`

	ValidateCommand string `json:"validateCommand,omitempty"`
	ShowCommand     string `json:"showCommand,omitempty"`
}

func autoFilterJSON(state *xlsxmutate.AutoFilterState) *XLSXAutoFilterJSON {
	if state == nil {
		return nil
	}
	out := &XLSXAutoFilterJSON{Ref: state.Ref}
	for _, col := range state.Columns {
		cj := XLSXFilterColumnJSON{ColID: col.ColID, Values: col.Values}
		if col.CustomFilter != nil {
			cf := &XLSXCustomFilterJSON{And: col.CustomFilter.And}
			for _, c := range col.CustomFilter.Criteria {
				cf.Criteria = append(cf.Criteria, XLSXCustomFilterCriterionJSON{Operator: c.Operator, Val: c.Val})
			}
			cj.CustomFilter = cf
		}
		out.Columns = append(out.Columns, cj)
	}
	return out
}

func sortStateJSON(state *xlsxmutate.SortStateInfo) *XLSXSortStateJSON {
	if state == nil {
		return nil
	}
	out := &XLSXSortStateJSON{Ref: state.Ref}
	for _, c := range state.Conditions {
		out.Conditions = append(out.Conditions, XLSXSortConditionJSON{Ref: c.Ref, Descending: c.Descending})
	}
	return out
}

// ---- flags ----

var (
	xlsxFSShowSheet string
	xlsxFSShowRange string
	xlsxFSShowTable string

	xlsxFSSheet       string
	xlsxFSRange       string
	xlsxFSTable       string
	xlsxFSExpectRange string

	xlsxFSColumn       int
	xlsxFSValues       string
	xlsxFSCustomOp     string
	xlsxFSCustomVal1   string
	xlsxFSCustomVal2   string
	xlsxFSExpectFilter string

	xlsxFSSortRef    string
	xlsxFSSortColumn string
	xlsxFSDescending bool
	xlsxFSExpectSort string
)

// resolveTableInPackage finds a TableRef within an already-open package session.
func resolveTableInPackage(pkg opc.PackageSession, sheetSelector, tableSelector string) (model.TableRef, error) {
	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		return model.TableRef{}, NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
	}
	sheets := workbook.Sheets
	if sheetSelector != "" {
		selected, err := selectXLSXSheet(workbook.Sheets, sheetSelector)
		if err != nil {
			return model.TableRef{}, err
		}
		if err := requireXLSXWorksheetRef(selected); err != nil {
			return model.TableRef{}, err
		}
		sheets = []model.SheetRef{selected}
	}
	tables, err := xlsxtable.List(pkg, workbook, sheets)
	if err != nil {
		return model.TableRef{}, NewCLIErrorf(ExitUnexpected, "failed to list tables: %v", err)
	}
	return selectXLSXTable(tables, tableSelector)
}

// runFiltersSortsMutation wires a mutate.* call into the standard mutation
// writer, building the JSON readback result.
func runFiltersSortsMutation(cmd *cobra.Command, filePath, sheetSel, tableSel, action string, apply func(pkg opc.PackageSession, sheet model.SheetRef, table *model.TableRef) (*xlsxmutate.FiltersSortsResult, error)) error {
	mutOpts, err := GetValidatedMutationOptions(cmd)
	if err != nil {
		return err
	}
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return err
	}
	useTable := strings.TrimSpace(tableSel) != ""
	var result *XLSXFiltersSortsMutationResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		var (
			sheetRef model.SheetRef
			tablePtr *model.TableRef
		)
		if useTable {
			tableRef, err := resolveTableInPackage(pkg, sheetSel, tableSel)
			if err != nil {
				return err
			}
			tablePtr = &tableRef
			sheetRef = model.SheetRef{Name: tableRef.Sheet, Number: tableRef.SheetNumber, PartURI: tableRef.SheetPartURI}
		} else {
			workbook, err := xlsxinspect.ParseWorkbook(pkg)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
			}
			sheetRef, err = selectXLSXSheet(workbook.Sheets, sheetSel)
			if err != nil {
				return err
			}
			if err := requireXLSXWorksheetRef(sheetRef); err != nil {
				return err
			}
		}
		mutResult, err := apply(pkg, sheetRef, tablePtr)
		if err != nil {
			return mapFiltersSortsError(action, err)
		}
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		result = &XLSXFiltersSortsMutationResult{
			File:        filePath,
			Sheet:       sheetRef.Name,
			SheetNumber: sheetRef.Number,
			Action:      action,
			Ref:         mutResult.Ref,
			Note:        filtersSortsHonesty,
			AutoFilter:  autoFilterJSON(mutResult.AutoFilter),
			SortState:   sortStateJSON(mutResult.SortState),
			Output:      destinationFile,
			DryRun:      mutOpts != nil && mutOpts.DryRun,
		}
		if tablePtr != nil {
			result.Table = tablePtr.DisplayName
		}
		if destinationFile != "" {
			result.ValidateCommand = xlsxValidateCommand(destinationFile)
			result.ShowCommand = filtersSortsShowCommand(destinationFile, sheetRef, tablePtr)
		}
		return nil
	}); err != nil {
		return err
	}
	if GetGlobalConfig(cmd).Format == "json" {
		return writeJSONResult(cmd, result, "filters-sorts "+action)
	}
	target := result.Sheet
	if result.Table != "" {
		target = result.Table
	}
	return writeXLSXOutput(cmd, []byte(fmt.Sprintf("%s on %s (%s)", action, target, filtersSortsHonesty)))
}

func filtersSortsShowCommand(filePath string, sheet model.SheetRef, table *model.TableRef) string {
	if table != nil {
		return fmt.Sprintf("ooxml --json xlsx filters-sorts show %s --table %s", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(table.DisplayName))
	}
	return fmt.Sprintf("ooxml --json xlsx filters-sorts show %s --sheet %s", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(xlsxSheetSelectorForRef(sheet)))
}

func mapFiltersSortsError(action string, err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	return NewCLIErrorf(ExitInvalidArgs, "failed to %s: %v", action, err)
}

// ---- show ----

var xlsxFiltersSortsShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Display current filter/sort state on a worksheet or table",
	Long:  "Display the current autoFilter and sortState. " + filtersSortsHonesty,
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		useTable := strings.TrimSpace(xlsxFSShowTable) != ""
		pkg, err := openPackageExpectType(filePath, opc.PackageTypeXLSX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		result := &XLSXFiltersSortsShowResult{File: filePath, Note: filtersSortsHonesty}
		var sheetRef model.SheetRef
		var tablePtr *model.TableRef
		if useTable {
			tableRef, err := resolveTableInPackage(pkg, xlsxFSShowSheet, xlsxFSShowTable)
			if err != nil {
				return err
			}
			tablePtr = &tableRef
			sheetRef = model.SheetRef{Name: tableRef.Sheet, Number: tableRef.SheetNumber, PartURI: tableRef.SheetPartURI}
			af, err := xlsxmutate.ReadTableAutoFilter(pkg, tableRef)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to read table autoFilter: %v", err)
			}
			result.Table = tableRef.DisplayName
			result.AutoFilter = autoFilterJSON(af)
			// Tables carry autoFilter, but sortState is read from the worksheet.
			ss, err := xlsxmutate.ReadSortState(pkg, sheetRef)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to read sortState: %v", err)
			}
			result.SortState = sortStateJSON(ss)
		} else {
			workbook, err := xlsxinspect.ParseWorkbook(pkg)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
			}
			sheetRef, err = selectXLSXSheet(workbook.Sheets, xlsxFSShowSheet)
			if err != nil {
				return err
			}
			if err := requireXLSXWorksheetRef(sheetRef); err != nil {
				return err
			}
			af, err := xlsxmutate.ReadAutoFilter(pkg, sheetRef)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to read autoFilter: %v", err)
			}
			ss, err := xlsxmutate.ReadSortState(pkg, sheetRef)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to read sortState: %v", err)
			}
			result.AutoFilter = autoFilterJSON(af)
			result.SortState = sortStateJSON(ss)
		}
		result.Sheet = sheetRef.Name
		result.SheetNumber = sheetRef.Number
		result.ShowCommand = filtersSortsShowCommand(filePath, sheetRef, tablePtr)
		if !useTable {
			selector := xlsxSheetSelectorForRef(sheetRef)
			result.SetAutoFilterCommand = fmt.Sprintf("ooxml xlsx filters-sorts set-autofilter %s --sheet %s --range <A1:D10> --in-place", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(selector))
			result.AddColumnFilterCommand = fmt.Sprintf("ooxml xlsx filters-sorts add-column-filter %s --sheet %s --column 0 --values <a,b,c> --in-place", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(selector))
			result.SetSortCommand = fmt.Sprintf("ooxml xlsx filters-sorts set-sort %s --sheet %s --ref <A1:D10> --column A --in-place", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(selector))
		}

		if GetGlobalConfig(cmd).Format == "json" {
			return writeJSONResult(cmd, result, "filters-sorts show")
		}
		return writeXLSXOutput(cmd, []byte(formatFiltersSortsText(result)))
	},
}

func formatFiltersSortsText(result *XLSXFiltersSortsShowResult) string {
	var b strings.Builder
	target := result.Sheet
	if result.Table != "" {
		target = result.Table
	}
	fmt.Fprintf(&b, "filters/sorts on %s:\n", target)
	if result.AutoFilter != nil {
		fmt.Fprintf(&b, "  autoFilter ref=%s\n", result.AutoFilter.Ref)
		for _, col := range result.AutoFilter.Columns {
			if len(col.Values) > 0 {
				fmt.Fprintf(&b, "    col %d values: %s\n", col.ColID, strings.Join(col.Values, ", "))
			}
			if col.CustomFilter != nil {
				var parts []string
				for _, c := range col.CustomFilter.Criteria {
					op := c.Operator
					if op == "" {
						op = "equal"
					}
					parts = append(parts, op+" "+c.Val)
				}
				fmt.Fprintf(&b, "    col %d custom: %s\n", col.ColID, strings.Join(parts, " and "))
			}
		}
	} else {
		b.WriteString("  autoFilter: none\n")
	}
	if result.SortState != nil {
		fmt.Fprintf(&b, "  sortState ref=%s\n", result.SortState.Ref)
		for _, c := range result.SortState.Conditions {
			dir := "asc"
			if c.Descending {
				dir = "desc"
			}
			fmt.Fprintf(&b, "    sort %s (%s)\n", c.Ref, dir)
		}
	} else {
		b.WriteString("  sortState: none\n")
	}
	b.WriteString(filtersSortsHonesty)
	return b.String()
}

// ---- set-autofilter ----

var xlsxFiltersSortsSetAutoFilterCmd = &cobra.Command{
	Use:   "set-autofilter <file>",
	Short: "Add an autoFilter to a range or table",
	Long:  "Add (or replace) an autoFilter element on a worksheet --range or a --table. " + filtersSortsHonesty,
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		useTable := strings.TrimSpace(xlsxFSTable) != ""
		if !useTable && strings.TrimSpace(xlsxFSRange) == "" {
			return InvalidArgsError("--range is required (or use --table)")
		}
		if useTable && strings.TrimSpace(xlsxFSRange) != "" {
			return InvalidArgsError("specify only one of --range or --table")
		}
		return runFiltersSortsMutation(cmd, filePath, xlsxFSSheet, xlsxFSTable, "set-autofilter", func(pkg opc.PackageSession, sheet model.SheetRef, table *model.TableRef) (*xlsxmutate.FiltersSortsResult, error) {
			rangeArg := xlsxFSRange
			if table != nil {
				// Default to the table's own range when --range is omitted.
				if strings.TrimSpace(rangeArg) == "" {
					rangeArg = table.Range
				}
			}
			return xlsxmutate.SetAutoFilter(&xlsxmutate.SetAutoFilterRequest{
				Package:     pkg,
				SheetRef:    sheet,
				Table:       table,
				Range:       rangeArg,
				ExpectRange: xlsxFSExpectRange,
				HasExpect:   cmd.Flags().Changed("expect-range"),
			})
		})
	},
}

// ---- clear-autofilter ----

var xlsxFiltersSortsClearAutoFilterCmd = &cobra.Command{
	Use:   "clear-autofilter <file>",
	Short: "Remove the autoFilter from a range or table",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		return runFiltersSortsMutation(cmd, filePath, xlsxFSSheet, xlsxFSTable, "clear-autofilter", func(pkg opc.PackageSession, sheet model.SheetRef, table *model.TableRef) (*xlsxmutate.FiltersSortsResult, error) {
			return xlsxmutate.ClearAutoFilter(&xlsxmutate.ClearAutoFilterRequest{
				Package:     pkg,
				SheetRef:    sheet,
				Table:       table,
				ExpectRange: xlsxFSExpectRange,
				HasExpect:   cmd.Flags().Changed("expect-range"),
			})
		})
	},
}

// ---- add-column-filter ----

var xlsxFiltersSortsAddColumnFilterCmd = &cobra.Command{
	Use:   "add-column-filter <file>",
	Short: "Add filter criteria to a column in the autoFilter",
	Long:  "Add a values list and/or a custom criterion to a 0-based column of an existing worksheet autoFilter (run set-autofilter first). " + filtersSortsHonesty,
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		hasCustom := cmd.Flags().Changed("custom-op")
		var values []string
		if v := strings.TrimSpace(xlsxFSValues); v != "" {
			values = strings.Split(v, ",")
			for i := range values {
				values[i] = strings.TrimSpace(values[i])
			}
		}
		if len(values) == 0 && !hasCustom {
			return InvalidArgsError("provide --values and/or --custom-op")
		}
		return runFiltersSortsMutation(cmd, filePath, xlsxFSSheet, "", "add-column-filter", func(pkg opc.PackageSession, sheet model.SheetRef, _ *model.TableRef) (*xlsxmutate.FiltersSortsResult, error) {
			return xlsxmutate.AddColumnFilter(&xlsxmutate.AddColumnFilterRequest{
				Package:      pkg,
				SheetRef:     sheet,
				ColID:        xlsxFSColumn,
				Values:       values,
				CustomOp:     xlsxFSCustomOp,
				CustomVal1:   xlsxFSCustomVal1,
				CustomVal2:   xlsxFSCustomVal2,
				HasCustom:    hasCustom,
				ExpectFilter: xlsxFSExpectFilter,
				HasExpect:    cmd.Flags().Changed("expect-filter"),
			})
		})
	},
}

// ---- clear-column-filter ----

var xlsxFiltersSortsClearColumnFilterCmd = &cobra.Command{
	Use:   "clear-column-filter <file>",
	Short: "Remove a column's filter from the autoFilter",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		return runFiltersSortsMutation(cmd, filePath, xlsxFSSheet, "", "clear-column-filter", func(pkg opc.PackageSession, sheet model.SheetRef, _ *model.TableRef) (*xlsxmutate.FiltersSortsResult, error) {
			return xlsxmutate.ClearColumnFilter(&xlsxmutate.ClearColumnFilterRequest{
				Package:  pkg,
				SheetRef: sheet,
				ColID:    xlsxFSColumn,
			})
		})
	},
}

// ---- set-sort ----

var xlsxFiltersSortsSetSortCmd = &cobra.Command{
	Use:   "set-sort <file>",
	Short: "Add a sort condition to the worksheet sortState",
	Long:  "Add a sortCondition for --column (a column letter) within the --ref range; the sortState is created on first call. " + filtersSortsHonesty,
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if strings.TrimSpace(xlsxFSSortRef) == "" {
			return InvalidArgsError("--ref is required (e.g. A1:D10)")
		}
		if strings.TrimSpace(xlsxFSSortColumn) == "" {
			return InvalidArgsError("--column is required (a column letter such as A)")
		}
		return runFiltersSortsMutation(cmd, filePath, xlsxFSSheet, "", "set-sort", func(pkg opc.PackageSession, sheet model.SheetRef, _ *model.TableRef) (*xlsxmutate.FiltersSortsResult, error) {
			return xlsxmutate.SetSort(&xlsxmutate.SetSortRequest{
				Package:    pkg,
				SheetRef:   sheet,
				Ref:        xlsxFSSortRef,
				Column:     xlsxFSSortColumn,
				Descending: xlsxFSDescending,
				ExpectSort: xlsxFSExpectSort,
				HasExpect:  cmd.Flags().Changed("expect-sort"),
			})
		})
	},
}

// ---- clear-sort ----

var xlsxFiltersSortsClearSortCmd = &cobra.Command{
	Use:   "clear-sort <file>",
	Short: "Remove the worksheet sortState",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		return runFiltersSortsMutation(cmd, filePath, xlsxFSSheet, "", "clear-sort", func(pkg opc.PackageSession, sheet model.SheetRef, _ *model.TableRef) (*xlsxmutate.FiltersSortsResult, error) {
			return xlsxmutate.ClearSort(&xlsxmutate.ClearSortRequest{
				Package:  pkg,
				SheetRef: sheet,
			})
		})
	},
}

func init() {
	xlsxFiltersSortsShowCmd.Flags().StringVar(&xlsxFSShowSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxFiltersSortsShowCmd.Flags().StringVar(&xlsxFSShowRange, "range", "", "informational range hint (state is read from the worksheet/table)")
	xlsxFiltersSortsShowCmd.Flags().StringVar(&xlsxFSShowTable, "table", "", "table number, name, or displayName")
	xlsxFiltersSortsCmd.AddCommand(xlsxFiltersSortsShowCmd)

	// autoFilter set/clear: support both --range and --table.
	for _, c := range []*cobra.Command{xlsxFiltersSortsSetAutoFilterCmd, xlsxFiltersSortsClearAutoFilterCmd} {
		c.Flags().StringVar(&xlsxFSSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
		c.Flags().StringVar(&xlsxFSRange, "range", "", "target range such as A1:D10")
		c.Flags().StringVar(&xlsxFSTable, "table", "", "table number, name, or displayName (mutually exclusive with --range)")
		c.Flags().StringVar(&xlsxFSExpectRange, "expect-range", "", "guard: require the current autoFilter ref to match")
		AddMutationFlags(c)
	}

	// column filter add/clear: worksheet-only.
	for _, c := range []*cobra.Command{xlsxFiltersSortsAddColumnFilterCmd, xlsxFiltersSortsClearColumnFilterCmd} {
		c.Flags().StringVar(&xlsxFSSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
		c.Flags().IntVar(&xlsxFSColumn, "column", 0, "0-based column offset within the autoFilter ref")
		AddMutationFlags(c)
	}
	xlsxFiltersSortsAddColumnFilterCmd.Flags().StringVar(&xlsxFSValues, "values", "", "comma-separated filter values, e.g. \"Apple,Banana\"")
	xlsxFiltersSortsAddColumnFilterCmd.Flags().StringVar(&xlsxFSCustomOp, "custom-op", "", "custom operator: equal|notEqual|lessThan|lessThanOrEqual|greaterThan|greaterThanOrEqual|between|notBetween")
	xlsxFiltersSortsAddColumnFilterCmd.Flags().StringVar(&xlsxFSCustomVal1, "custom-val1", "", "first custom criterion value")
	xlsxFiltersSortsAddColumnFilterCmd.Flags().StringVar(&xlsxFSCustomVal2, "custom-val2", "", "second custom criterion value (for between/notBetween)")
	xlsxFiltersSortsAddColumnFilterCmd.Flags().StringVar(&xlsxFSExpectFilter, "expect-filter", "", "guard: require the current column filter summary to match (none|values:..|custom:..)")

	// sort set/clear: worksheet-only.
	for _, c := range []*cobra.Command{xlsxFiltersSortsSetSortCmd, xlsxFiltersSortsClearSortCmd} {
		c.Flags().StringVar(&xlsxFSSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
		AddMutationFlags(c)
	}
	xlsxFiltersSortsSetSortCmd.Flags().StringVar(&xlsxFSSortRef, "ref", "", "sortState range such as A1:D10")
	xlsxFiltersSortsSetSortCmd.Flags().StringVar(&xlsxFSSortColumn, "column", "", "column letter to sort by, e.g. A")
	xlsxFiltersSortsSetSortCmd.Flags().BoolVar(&xlsxFSDescending, "descending", false, "sort descending (default ascending)")
	xlsxFiltersSortsSetSortCmd.Flags().StringVar(&xlsxFSExpectSort, "expect-sort", "", "guard: require the current sortState ref to match")

	xlsxFiltersSortsCmd.AddCommand(xlsxFiltersSortsSetAutoFilterCmd)
	xlsxFiltersSortsCmd.AddCommand(xlsxFiltersSortsClearAutoFilterCmd)
	xlsxFiltersSortsCmd.AddCommand(xlsxFiltersSortsAddColumnFilterCmd)
	xlsxFiltersSortsCmd.AddCommand(xlsxFiltersSortsClearColumnFilterCmd)
	xlsxFiltersSortsCmd.AddCommand(xlsxFiltersSortsSetSortCmd)
	xlsxFiltersSortsCmd.AddCommand(xlsxFiltersSortsClearSortCmd)

	xlsxCmd.AddCommand(xlsxFiltersSortsCmd)
}
