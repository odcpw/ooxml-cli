package cli

import (
	"fmt"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	xlsxmutate "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

var xlsxFreezeCmd = &cobra.Command{
	Use:   "freeze",
	Short: "Inspect and set worksheet freeze panes",
	Long:  "Show, set, and clear frozen rows/columns (the worksheet sheetView <pane state=\"frozen\"> element).",
	Args:  cobra.NoArgs,
	RunE:  showHelp,
}

// ---- JSON shapes ----

// XLSXFreezeStateJSON mirrors mutate.FreezeState for readback.
type XLSXFreezeStateJSON struct {
	Rows        int    `json:"rows"`
	Cols        int    `json:"cols"`
	TopLeftCell string `json:"topLeftCell"`
	Frozen      bool   `json:"frozen"`
}

type XLSXFreezeShowResult struct {
	File        string               `json:"file"`
	Sheet       string               `json:"sheet"`
	SheetNumber int                  `json:"sheetNumber"`
	State       *XLSXFreezeStateJSON `json:"state"`

	SetCommand   string `json:"setCommand,omitempty"`
	ClearCommand string `json:"clearCommand,omitempty"`
	ShowCommand  string `json:"showCommand,omitempty"`
}

type XLSXFreezeMutationResult struct {
	File        string               `json:"file"`
	Sheet       string               `json:"sheet"`
	SheetNumber int                  `json:"sheetNumber"`
	Action      string               `json:"action"`
	State       *XLSXFreezeStateJSON `json:"state"`
	Output      string               `json:"output,omitempty"`
	DryRun      bool                 `json:"dryRun"`

	ValidateCommand string `json:"validateCommand,omitempty"`
	ShowCommand     string `json:"showCommand,omitempty"`
}

func freezeStateJSON(state *xlsxmutate.FreezeState) *XLSXFreezeStateJSON {
	if state == nil {
		return nil
	}
	return &XLSXFreezeStateJSON{
		Rows:        state.Rows,
		Cols:        state.Cols,
		TopLeftCell: state.TopLeftCell,
		Frozen:      state.Frozen,
	}
}

// ---- flags ----

var (
	xlsxFreezeShowSheet string

	xlsxFreezeSheet       string
	xlsxFreezeRows        int
	xlsxFreezeCols        int
	xlsxFreezeExpectState string
)

func freezeShowCommand(filePath string, sheet model.SheetRef) string {
	return fmt.Sprintf("ooxml --json xlsx freeze show %s --sheet %s", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(xlsxSheetSelectorForRef(sheet)))
}

func mapFreezeError(action string, err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	return NewCLIErrorf(ExitInvalidArgs, "failed to %s freeze panes: %v", action, err)
}

// resolveFreezeSheet selects and validates the target worksheet within an open package.
func resolveFreezeSheet(pkg opc.PackageSession, selector string) (model.SheetRef, error) {
	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		return model.SheetRef{}, NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
	}
	sheetRef, err := selectXLSXSheet(workbook.Sheets, selector)
	if err != nil {
		return model.SheetRef{}, err
	}
	if err := requireXLSXWorksheetRef(sheetRef); err != nil {
		return model.SheetRef{}, err
	}
	return sheetRef, nil
}

// runFreezeMutation wires a mutate.* freeze call into the standard mutation writer.
func runFreezeMutation(cmd *cobra.Command, filePath, sheetSel, action string, apply func(pkg opc.PackageSession, sheet model.SheetRef) (*xlsxmutate.FreezeResult, error)) error {
	mutOpts, err := GetValidatedMutationOptions(cmd)
	if err != nil {
		return err
	}
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return err
	}
	var result *XLSXFreezeMutationResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		sheetRef, err := resolveFreezeSheet(pkg, sheetSel)
		if err != nil {
			return err
		}
		mutResult, err := apply(pkg, sheetRef)
		if err != nil {
			return mapFreezeError(action, err)
		}
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		result = &XLSXFreezeMutationResult{
			File:        filePath,
			Sheet:       sheetRef.Name,
			SheetNumber: sheetRef.Number,
			Action:      action,
			State:       freezeStateJSON(mutResult.State),
			Output:      destinationFile,
			DryRun:      mutOpts != nil && mutOpts.DryRun,
		}
		if destinationFile != "" {
			result.ValidateCommand = xlsxValidateCommand(destinationFile)
			result.ShowCommand = freezeShowCommand(destinationFile, sheetRef)
		}
		return nil
	}); err != nil {
		return err
	}
	if GetGlobalConfig(cmd).Format == "json" {
		return writeJSONResult(cmd, result, "freeze "+action)
	}
	return writeXLSXOutput(cmd, []byte(formatFreezeMutationText(result)))
}

func formatFreezeMutationText(result *XLSXFreezeMutationResult) string {
	if result.State == nil {
		return fmt.Sprintf("freeze %s on %s: no frozen panes", result.Action, result.Sheet)
	}
	return fmt.Sprintf("freeze %s on %s: %d rows, %d cols frozen (topLeftCell %s)", result.Action, result.Sheet, result.State.Rows, result.State.Cols, result.State.TopLeftCell)
}

// ---- show ----

var xlsxFreezeShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Display current freeze panes state",
	Long:  "Display the frozen rows/columns on a worksheet (null when no panes are frozen).",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		pkg, err := openPackageExpectType(filePath, opc.PackageTypeXLSX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		sheetRef, err := resolveFreezeSheet(pkg, xlsxFreezeShowSheet)
		if err != nil {
			return err
		}
		state, err := xlsxmutate.ReadFreeze(pkg, sheetRef)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to read freeze panes: %v", err)
		}
		selector := xlsxSheetSelectorForRef(sheetRef)
		result := &XLSXFreezeShowResult{
			File:         filePath,
			Sheet:        sheetRef.Name,
			SheetNumber:  sheetRef.Number,
			State:        freezeStateJSON(state),
			ShowCommand:  freezeShowCommand(filePath, sheetRef),
			SetCommand:   fmt.Sprintf("ooxml xlsx freeze set %s --sheet %s --rows 1 --cols 1 --in-place", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(selector)),
			ClearCommand: fmt.Sprintf("ooxml xlsx freeze clear %s --sheet %s --in-place", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(selector)),
		}

		if GetGlobalConfig(cmd).Format == "json" {
			return writeJSONResult(cmd, result, "freeze show")
		}
		if result.State == nil {
			return writeXLSXOutput(cmd, []byte(fmt.Sprintf("freeze panes on %s: none", result.Sheet)))
		}
		return writeXLSXOutput(cmd, []byte(fmt.Sprintf("freeze panes on %s: %d rows, %d cols frozen (topLeftCell %s)", result.Sheet, result.State.Rows, result.State.Cols, result.State.TopLeftCell)))
	},
}

// ---- set ----

var xlsxFreezeSetCmd = &cobra.Command{
	Use:   "set <file>",
	Short: "Set frozen rows/columns on a worksheet",
	Long:  "Freeze --rows rows and/or --cols columns on a worksheet. Provide at least one of --rows or --cols.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		return runFreezeMutation(cmd, filePath, xlsxFreezeSheet, "set", func(pkg opc.PackageSession, sheet model.SheetRef) (*xlsxmutate.FreezeResult, error) {
			return xlsxmutate.SetFreeze(&xlsxmutate.SetFreezeRequest{
				Package:     pkg,
				SheetRef:    sheet,
				Rows:        xlsxFreezeRows,
				Cols:        xlsxFreezeCols,
				ExpectState: xlsxFreezeExpectState,
				HasExpect:   cmd.Flags().Changed("expect-state"),
			})
		})
	},
}

// ---- clear ----

var xlsxFreezeClearCmd = &cobra.Command{
	Use:   "clear <file>",
	Short: "Remove freeze panes from a worksheet",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		return runFreezeMutation(cmd, filePath, xlsxFreezeSheet, "clear", func(pkg opc.PackageSession, sheet model.SheetRef) (*xlsxmutate.FreezeResult, error) {
			return xlsxmutate.ClearFreeze(&xlsxmutate.ClearFreezeRequest{
				Package:     pkg,
				SheetRef:    sheet,
				ExpectState: xlsxFreezeExpectState,
				HasExpect:   cmd.Flags().Changed("expect-state"),
			})
		})
	},
}

func init() {
	xlsxFreezeShowCmd.Flags().StringVar(&xlsxFreezeShowSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxFreezeCmd.AddCommand(xlsxFreezeShowCmd)

	xlsxFreezeSetCmd.Flags().StringVar(&xlsxFreezeSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxFreezeSetCmd.Flags().IntVar(&xlsxFreezeRows, "rows", 0, "number of top rows to freeze (0 = none)")
	xlsxFreezeSetCmd.Flags().IntVar(&xlsxFreezeCols, "cols", 0, "number of left columns to freeze (0 = none)")
	xlsxFreezeSetCmd.Flags().StringVar(&xlsxFreezeExpectState, "expect-state", "", "guard: require the current state to match (none|frozen)")
	AddMutationFlags(xlsxFreezeSetCmd)
	xlsxFreezeCmd.AddCommand(xlsxFreezeSetCmd)

	xlsxFreezeClearCmd.Flags().StringVar(&xlsxFreezeSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxFreezeClearCmd.Flags().StringVar(&xlsxFreezeExpectState, "expect-state", "", "guard: require the current state to match (none|frozen)")
	AddMutationFlags(xlsxFreezeClearCmd)
	xlsxFreezeCmd.AddCommand(xlsxFreezeClearCmd)

	xlsxCmd.AddCommand(xlsxFreezeCmd)
}
