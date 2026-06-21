package cli

import (
	"fmt"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

var xlsxDataValidationsCmd = &cobra.Command{
	Use:     "data-validations",
	Aliases: []string{"data-validation", "datavalidations", "dv"},
	Short:   "Inspect and mutate worksheet data validations",
	Long:    "Commands for listing, showing, creating, updating, and deleting worksheet data validation rules (dropdown lists, number/date/text-length constraints).",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

type XLSXDataValidationJSON struct {
	Sqref           string   `json:"sqref"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`

	Type             string `json:"type,omitempty"`
	Operator         string `json:"operator,omitempty"`
	Formula1         string `json:"formula1,omitempty"`
	Formula2         string `json:"formula2,omitempty"`
	AllowBlank       bool   `json:"allowBlank"`
	ShowInputMessage bool   `json:"showInputMessage"`
	ShowErrorMessage bool   `json:"showErrorMessage"`
	PromptTitle      string `json:"promptTitle,omitempty"`
	Prompt           string `json:"prompt,omitempty"`
	ErrorTitle       string `json:"errorTitle,omitempty"`
	Error            string `json:"error,omitempty"`
	ErrorStyle       string `json:"errorStyle,omitempty"`
}

func dataValidationToJSON(dv mutate.DataValidation) XLSXDataValidationJSON {
	return XLSXDataValidationJSON{
		Sqref:           dv.Sqref,
		PrimarySelector: dv.Sqref,
		Selectors:       xlsxDataValidationSelectors(dv.Sqref),

		Type:             dv.Type,
		Operator:         dv.Operator,
		Formula1:         dv.Formula1,
		Formula2:         dv.Formula2,
		AllowBlank:       dv.AllowBlank,
		ShowInputMessage: dv.ShowInputMessage,
		ShowErrorMessage: dv.ShowErrorMessage,
		PromptTitle:      dv.PromptTitle,
		Prompt:           dv.Prompt,
		ErrorTitle:       dv.ErrorTitle,
		Error:            dv.Error,
		ErrorStyle:       dv.ErrorStyle,
	}
}

func xlsxDataValidationSelectors(sqref string) []string {
	if sqref == "" {
		return nil
	}
	return []string{sqref}
}

type XLSXDataValidationsListResult struct {
	File            string                   `json:"file"`
	Sheet           string                   `json:"sheet"`
	SheetNumber     int                      `json:"sheetNumber"`
	Count           int                      `json:"count"`
	DataValidations []XLSXDataValidationJSON `json:"dataValidations"`
}

type XLSXDataValidationMutationResult struct {
	File                       string                  `json:"file"`
	Sheet                      string                  `json:"sheet"`
	SheetNumber                int                     `json:"sheetNumber"`
	Action                     string                  `json:"action"`
	Range                      string                  `json:"range"`
	CellsAffected              int                     `json:"cellsAffected"`
	DataValidation             *XLSXDataValidationJSON `json:"dataValidation,omitempty"`
	Output                     string                  `json:"output,omitempty"`
	DryRun                     bool                    `json:"dryRun"`
	ValidateCommand            string                  `json:"validateCommand,omitempty"`
	DataValidationsListCommand string                  `json:"dataValidationsListCommand,omitempty"`
	DataValidationsShowCommand string                  `json:"dataValidationsShowCommand,omitempty"`
}

var (
	xlsxDVListSheet string
	xlsxDVShowSheet string
	xlsxDVShowRange string

	xlsxDVSheet            string
	xlsxDVRange            string
	xlsxDVType             string
	xlsxDVListValues       string
	xlsxDVListRange        string
	xlsxDVOperator         string
	xlsxDVFormula1         string
	xlsxDVFormula2         string
	xlsxDVAllowBlank       bool
	xlsxDVShowInputMessage bool
	xlsxDVInputTitle       string
	xlsxDVInputMessage     string
	xlsxDVShowErrorMessage bool
	xlsxDVErrorTitle       string
	xlsxDVErrorMessage     string
	xlsxDVErrorStyle       string
	xlsxDVExpectType       string
	xlsxDVExpectFormula1   string
)

// ---- list ----

var xlsxDataValidationsListCmd = &cobra.Command{
	Use:   "list <file>",
	Short: "List worksheet data validations",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		pkg, _, sheetRef, closeFn, err := resolveXLSXWorksheet(filePath, xlsxDVListSheet)
		if err != nil {
			return err
		}
		defer closeFn()
		validations, err := mutate.ListDataValidations(pkg, sheetRef)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to list data validations: %v", err)
		}
		result := &XLSXDataValidationsListResult{File: filePath, Sheet: sheetRef.Name, SheetNumber: sheetRef.Number}
		for _, dv := range validations {
			result.DataValidations = append(result.DataValidations, dataValidationToJSON(dv))
		}
		result.Count = len(result.DataValidations)
		if GetGlobalConfig(cmd).Format == "json" {
			return writeJSONResult(cmd, result, "data-validations list")
		}
		var b strings.Builder
		fmt.Fprintf(&b, "%d data validation(s) on %s:\n", result.Count, sheetRef.Name)
		for _, dv := range result.DataValidations {
			fmt.Fprintf(&b, "  %s [%s] %s\n", dv.Sqref, dv.Type, dv.Formula1)
		}
		return writeXLSXOutput(cmd, []byte(strings.TrimRight(b.String(), "\n")))
	},
}

// ---- show ----

var xlsxDataValidationsShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Show the data validation for a range (by sqref)",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if strings.TrimSpace(xlsxDVShowRange) == "" {
			return InvalidArgsError("--range is required")
		}
		normRange, err := mutate.NormalizeSqref(xlsxDVShowRange)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "invalid --range: %v", err)
		}
		pkg, _, sheetRef, closeFn, err := resolveXLSXWorksheet(filePath, xlsxDVShowSheet)
		if err != nil {
			return err
		}
		defer closeFn()
		validations, err := mutate.ListDataValidations(pkg, sheetRef)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to list data validations: %v", err)
		}
		var sqrefs []string
		var candidates []SelectorCandidate
		for _, dv := range validations {
			if norm, err := mutate.NormalizeSqref(dv.Sqref); err == nil && norm == normRange {
				j := dataValidationToJSON(dv)
				if GetGlobalConfig(cmd).Format == "json" {
					return writeJSONResult(cmd, &j, "data-validations show")
				}
				return writeXLSXOutput(cmd, []byte(fmt.Sprintf("%s [%s] %s", j.Sqref, j.Type, j.Formula1)))
			}
			sqrefs = append(sqrefs, dv.Sqref)
			candidates = append(candidates, SelectorCandidate{Primary: dv.Sqref, Selectors: xlsxDataValidationSelectors(dv.Sqref)})
		}
		discovery := fmt.Sprintf("ooxml --json xlsx data-validations list <file> --sheet %s", pptxXLSXCommandArg(xlsxSheetSelectorForRef(sheetRef)))
		if len(sqrefs) == 0 {
			return SelectorNotFoundError("data validation", normRange, nil, discovery)
		}
		return SelectorNotFoundError("data validation", normRange, BuildSelectorCandidates(candidates, normRange, maxSelectorCandidates), discovery)
	},
}

// ---- mutation helper ----

func runDataValidationMutation(cmd *cobra.Command, filePath, sheetSel, action string, apply func(pkg opc.PackageSession, sheet model.SheetRef) (*mutate.DataValidationMutationResult, error)) error {
	mutOpts, err := GetValidatedMutationOptions(cmd)
	if err != nil {
		return err
	}
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return err
	}
	var result *XLSXDataValidationMutationResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheetRef, err := selectXLSXSheet(workbook.Sheets, sheetSel)
		if err != nil {
			return err
		}
		if err := requireXLSXWorksheetRef(sheetRef); err != nil {
			return err
		}
		mutResult, err := apply(pkg, sheetRef)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "failed to %s data validation: %v", action, err)
		}
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		result = &XLSXDataValidationMutationResult{
			File:          filePath,
			Sheet:         sheetRef.Name,
			SheetNumber:   sheetRef.Number,
			Action:        action,
			Range:         mutResult.Sqref,
			CellsAffected: mutResult.CellsAffected,
			Output:        destinationFile,
			DryRun:        mutOpts != nil && mutOpts.DryRun,
		}
		if action != "delete" {
			j := dataValidationToJSON(mutResult.Validation)
			result.DataValidation = &j
		}
		if destinationFile != "" {
			selector := xlsxSheetSelectorForRef(sheetRef)
			result.ValidateCommand = xlsxValidateCommand(destinationFile)
			result.DataValidationsListCommand = fmt.Sprintf("ooxml --json xlsx data-validations list %s --sheet %s", pptxXLSXCommandArg(destinationFile), pptxXLSXCommandArg(selector))
			if action != "delete" {
				result.DataValidationsShowCommand = fmt.Sprintf("ooxml --json xlsx data-validations show %s --sheet %s --range %s", pptxXLSXCommandArg(destinationFile), pptxXLSXCommandArg(selector), pptxXLSXCommandArg(mutResult.Sqref))
			}
		}
		return nil
	}); err != nil {
		return err
	}
	if GetGlobalConfig(cmd).Format == "json" {
		return writeJSONResult(cmd, result, "data-validations "+action)
	}
	return writeXLSXOutput(cmd, []byte(fmt.Sprintf("%sd data validation on %s!%s", action, result.Sheet, result.Range)))
}

func dataValidationFieldsFromFlags(cmd *cobra.Command) mutate.DataValidationFields {
	return mutate.DataValidationFields{
		Type:             xlsxDVType,
		Operator:         xlsxDVOperator,
		Formula1:         xlsxDVFormula1,
		Formula2:         xlsxDVFormula2,
		ListValues:       xlsxDVListValues,
		ListRange:        xlsxDVListRange,
		AllowBlank:       xlsxDVAllowBlank,
		ShowInputMessage: xlsxDVShowInputMessage,
		ShowErrorMessage: xlsxDVShowErrorMessage,
		PromptTitle:      xlsxDVInputTitle,
		Prompt:           xlsxDVInputMessage,
		ErrorTitle:       xlsxDVErrorTitle,
		Error:            xlsxDVErrorMessage,
		ErrorStyle:       xlsxDVErrorStyle,

		SetType:             cmd.Flags().Changed("type"),
		SetOperator:         cmd.Flags().Changed("operator"),
		SetFormula1:         cmd.Flags().Changed("formula1"),
		SetFormula2:         cmd.Flags().Changed("formula2"),
		SetListValues:       cmd.Flags().Changed("list-values"),
		SetListRange:        cmd.Flags().Changed("list-range"),
		SetAllowBlank:       cmd.Flags().Changed("allow-blank"),
		SetShowInputMessage: cmd.Flags().Changed("show-input-message"),
		SetShowErrorMessage: cmd.Flags().Changed("show-error-message"),
		SetPromptTitle:      cmd.Flags().Changed("input-title"),
		SetPrompt:           cmd.Flags().Changed("input-message"),
		SetErrorTitle:       cmd.Flags().Changed("error-title"),
		SetError:            cmd.Flags().Changed("error-message"),
		SetErrorStyle:       cmd.Flags().Changed("error-style"),
	}
}

// ---- create ----

var xlsxDataValidationsCreateCmd = &cobra.Command{
	Use:   "create <file>",
	Short: "Create a data validation rule",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if strings.TrimSpace(xlsxDVRange) == "" {
			return InvalidArgsError("--range is required")
		}
		if strings.TrimSpace(xlsxDVType) == "" {
			return InvalidArgsError("--type is required (list|whole|decimal|date|text-length)")
		}
		return runDataValidationMutation(cmd, filePath, xlsxDVSheet, "create", func(pkg opc.PackageSession, sheet model.SheetRef) (*mutate.DataValidationMutationResult, error) {
			return mutate.CreateDataValidation(&mutate.CreateDataValidationRequest{
				Package:  pkg,
				SheetRef: sheet,
				Range:    xlsxDVRange,
				Fields:   dataValidationFieldsFromFlags(cmd),
			})
		})
	},
}

// ---- update ----

var xlsxDataValidationsUpdateCmd = &cobra.Command{
	Use:   "update <file>",
	Short: "Update an existing data validation rule",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if strings.TrimSpace(xlsxDVRange) == "" {
			return InvalidArgsError("--range is required")
		}
		return runDataValidationMutation(cmd, filePath, xlsxDVSheet, "update", func(pkg opc.PackageSession, sheet model.SheetRef) (*mutate.DataValidationMutationResult, error) {
			return mutate.UpdateDataValidation(&mutate.UpdateDataValidationRequest{
				Package:        pkg,
				SheetRef:       sheet,
				Range:          xlsxDVRange,
				Fields:         dataValidationFieldsFromFlags(cmd),
				ExpectType:     xlsxDVExpectType,
				HasExpectType:  cmd.Flags().Changed("expect-type"),
				ExpectFormula1: xlsxDVExpectFormula1,
				HasExpectF1:    cmd.Flags().Changed("expect-formula1"),
			})
		})
	},
}

// ---- delete ----

var xlsxDataValidationsDeleteCmd = &cobra.Command{
	Use:   "delete <file>",
	Short: "Delete a data validation rule by range (sqref)",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if strings.TrimSpace(xlsxDVRange) == "" {
			return InvalidArgsError("--range is required")
		}
		return runDataValidationMutation(cmd, filePath, xlsxDVSheet, "delete", func(pkg opc.PackageSession, sheet model.SheetRef) (*mutate.DataValidationMutationResult, error) {
			return mutate.DeleteDataValidation(&mutate.DeleteDataValidationRequest{
				Package:        pkg,
				SheetRef:       sheet,
				Range:          xlsxDVRange,
				ExpectType:     xlsxDVExpectType,
				HasExpectType:  cmd.Flags().Changed("expect-type"),
				ExpectFormula1: xlsxDVExpectFormula1,
				HasExpectF1:    cmd.Flags().Changed("expect-formula1"),
			})
		})
	},
}

func init() {
	xlsxDataValidationsListCmd.Flags().StringVar(&xlsxDVListSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxDataValidationsCmd.AddCommand(xlsxDataValidationsListCmd)

	xlsxDataValidationsShowCmd.Flags().StringVar(&xlsxDVShowSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxDataValidationsShowCmd.Flags().StringVar(&xlsxDVShowRange, "range", "", "target range (sqref) such as A1:A10")
	xlsxDataValidationsCmd.AddCommand(xlsxDataValidationsShowCmd)

	for _, c := range []*cobra.Command{xlsxDataValidationsCreateCmd, xlsxDataValidationsUpdateCmd, xlsxDataValidationsDeleteCmd} {
		c.Flags().StringVar(&xlsxDVSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
		c.Flags().StringVar(&xlsxDVRange, "range", "", "target range (sqref); space-separated allowed, e.g. \"A1:A10 C1:C5\"")
		AddMutationFlags(c)
	}

	for _, c := range []*cobra.Command{xlsxDataValidationsCreateCmd, xlsxDataValidationsUpdateCmd} {
		c.Flags().StringVar(&xlsxDVType, "type", "", "validation type: list|whole|decimal|date|text-length")
		c.Flags().StringVar(&xlsxDVListValues, "list-values", "", "comma-separated inline values for list type, e.g. \"a,b,c\"")
		c.Flags().StringVar(&xlsxDVListRange, "list-range", "", "range reference source for list type, e.g. Sheet1!$A$1:$A$10")
		c.Flags().StringVar(&xlsxDVOperator, "operator", "", "operator: between|notBetween|equal|notEqual|greaterThan|lessThan|greaterThanOrEqual|lessThanOrEqual")
		c.Flags().StringVar(&xlsxDVFormula1, "formula1", "", "first formula/bound")
		c.Flags().StringVar(&xlsxDVFormula2, "formula2", "", "second formula/bound (for between/notBetween)")
		c.Flags().BoolVar(&xlsxDVAllowBlank, "allow-blank", false, "allow blank cells")
		c.Flags().BoolVar(&xlsxDVShowInputMessage, "show-input-message", false, "show the input prompt message")
		c.Flags().StringVar(&xlsxDVInputTitle, "input-title", "", "input prompt title")
		c.Flags().StringVar(&xlsxDVInputMessage, "input-message", "", "input prompt message")
		c.Flags().BoolVar(&xlsxDVShowErrorMessage, "show-error-message", false, "show the error alert message")
		c.Flags().StringVar(&xlsxDVErrorTitle, "error-title", "", "error alert title")
		c.Flags().StringVar(&xlsxDVErrorMessage, "error-message", "", "error alert message")
		c.Flags().StringVar(&xlsxDVErrorStyle, "error-style", "", "error alert style: stop|warning|information")
	}

	for _, c := range []*cobra.Command{xlsxDataValidationsUpdateCmd, xlsxDataValidationsDeleteCmd} {
		c.Flags().StringVar(&xlsxDVExpectType, "expect-type", "", "guard: require the current validation type to match")
		c.Flags().StringVar(&xlsxDVExpectFormula1, "expect-formula1", "", "guard: require the current formula1 to match")
	}

	xlsxDataValidationsCmd.AddCommand(xlsxDataValidationsCreateCmd)
	xlsxDataValidationsCmd.AddCommand(xlsxDataValidationsUpdateCmd)
	xlsxDataValidationsCmd.AddCommand(xlsxDataValidationsDeleteCmd)

	xlsxCmd.AddCommand(xlsxDataValidationsCmd)
}
