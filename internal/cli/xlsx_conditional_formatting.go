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

var xlsxConditionalFormatsCmd = &cobra.Command{
	Use:     "conditional-formats",
	Aliases: []string{"conditional-formatting", "conditional-format", "cf"},
	Short:   "Inspect and mutate worksheet conditional formatting",
	Long:    "Commands for listing, showing, adding, and deleting worksheet conditional-formatting expression and cellIs rules.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

type XLSXConditionalFormatRuleJSON struct {
	Index           int      `json:"index"`
	BlockIndex      int      `json:"blockIndex"`
	RuleIndex       int      `json:"ruleIndex"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
	Sqref           string   `json:"sqref"`
	Type            string   `json:"type,omitempty"`
	Operator        string   `json:"operator,omitempty"`
	Priority        *int     `json:"priority,omitempty"`
	Formula         string   `json:"formula,omitempty"`
	Formulas        []string `json:"formulas,omitempty"`
	DxfID           *int     `json:"dxfId,omitempty"`
	StopIfTrue      bool     `json:"stopIfTrue,omitempty"`
}

type XLSXConditionalFormatBlockJSON struct {
	Index int                             `json:"index"`
	Sqref string                          `json:"sqref"`
	Rules []XLSXConditionalFormatRuleJSON `json:"rules"`
}

type XLSXConditionalFormatsListResult struct {
	File               string                           `json:"file"`
	Sheet              string                           `json:"sheet"`
	SheetNumber        int                              `json:"sheetNumber"`
	SheetSelector      string                           `json:"sheetSelector,omitempty"`
	Count              int                              `json:"count"`
	ConditionalFormats []XLSXConditionalFormatBlockJSON `json:"conditionalFormats"`
	Rules              []XLSXConditionalFormatRuleJSON  `json:"rules"`
}

type XLSXConditionalFormatMutationResult struct {
	File                          string                         `json:"file"`
	Sheet                         string                         `json:"sheet"`
	SheetNumber                   int                            `json:"sheetNumber"`
	SheetSelector                 string                         `json:"sheetSelector,omitempty"`
	Action                        string                         `json:"action"`
	Range                         string                         `json:"range"`
	Rule                          *XLSXConditionalFormatRuleJSON `json:"rule,omitempty"`
	CellsAffected                 int                            `json:"cellsAffected"`
	Output                        string                         `json:"output,omitempty"`
	DryRun                        bool                           `json:"dryRun"`
	ValidateCommand               string                         `json:"validateCommand,omitempty"`
	ConditionalFormatsListCommand string                         `json:"conditionalFormatsListCommand,omitempty"`
	ConditionalFormatsShowCommand string                         `json:"conditionalFormatsShowCommand,omitempty"`
}

var (
	xlsxCFListSheet string
	xlsxCFListRange string
	xlsxCFShowSheet string
	xlsxCFShowRule  string

	xlsxCFSheet      string
	xlsxCFRange      string
	xlsxCFType       string
	xlsxCFOperator   string
	xlsxCFFormula    string
	xlsxCFFormula2   string
	xlsxCFRule       string
	xlsxCFPriority   int
	xlsxCFStopIfTrue bool
	xlsxCFDxfID      int
)

func conditionalFormatRuleToJSON(rule mutate.ConditionalFormatRule) XLSXConditionalFormatRuleJSON {
	out := XLSXConditionalFormatRuleJSON{
		Index:           rule.Index,
		BlockIndex:      rule.BlockIndex,
		RuleIndex:       rule.RuleIndex,
		PrimarySelector: rule.PrimarySelector,
		Selectors:       rule.Selectors,
		Sqref:           rule.Sqref,
		Type:            rule.Type,
		Operator:        rule.Operator,
		Formulas:        rule.Formulas,
		StopIfTrue:      rule.StopIfTrue,
	}
	if rule.Priority > 0 {
		priority := rule.Priority
		out.Priority = &priority
	}
	if len(rule.Formulas) > 0 {
		out.Formula = rule.Formulas[0]
	}
	if rule.HasDxfID {
		dxfID := rule.DxfID
		out.DxfID = &dxfID
	}
	return out
}

func conditionalFormatsToJSON(blocks []mutate.ConditionalFormatBlock, filterSqref string) ([]XLSXConditionalFormatBlockJSON, []XLSXConditionalFormatRuleJSON) {
	var out []XLSXConditionalFormatBlockJSON
	var rules []XLSXConditionalFormatRuleJSON
	for _, block := range blocks {
		if filterSqref != "" {
			norm, err := mutate.NormalizeSqref(block.Sqref)
			if err != nil || norm != filterSqref {
				continue
			}
		}
		item := XLSXConditionalFormatBlockJSON{Index: block.Index, Sqref: block.Sqref}
		for _, rule := range block.Rules {
			j := conditionalFormatRuleToJSON(rule)
			item.Rules = append(item.Rules, j)
			rules = append(rules, j)
		}
		out = append(out, item)
	}
	return out, rules
}

var xlsxConditionalFormatsListCmd = &cobra.Command{
	Use:   "list <file>",
	Short: "List worksheet conditional-formatting rules",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		var normRange string
		if strings.TrimSpace(xlsxCFListRange) != "" {
			var err error
			normRange, err = mutate.NormalizeSqref(xlsxCFListRange)
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "invalid --range: %v", err)
			}
		}
		pkg, _, sheetRef, closeFn, err := resolveXLSXWorksheet(filePath, xlsxCFListSheet)
		if err != nil {
			return err
		}
		defer closeFn()
		blocks, err := mutate.ListConditionalFormats(pkg, sheetRef)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to list conditional formats: %v", err)
		}
		jsonBlocks, rules := conditionalFormatsToJSON(blocks, normRange)
		result := &XLSXConditionalFormatsListResult{
			File:               filePath,
			Sheet:              sheetRef.Name,
			SheetNumber:        sheetRef.Number,
			SheetSelector:      xlsxSheetSelectorForRef(sheetRef),
			ConditionalFormats: jsonBlocks,
			Rules:              rules,
			Count:              len(rules),
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeJSONResult(cmd, result, "conditional-formats list")
		}
		var b strings.Builder
		fmt.Fprintf(&b, "%d conditional-formatting rule(s) on %s:\n", result.Count, sheetRef.Name)
		for _, rule := range result.Rules {
			priority := ""
			if rule.Priority != nil {
				priority = fmt.Sprintf(" priority=%d", *rule.Priority)
			}
			operator := ""
			if rule.Operator != "" {
				operator = " " + rule.Operator
			}
			fmt.Fprintf(&b, "  %s %s [%s%s]%s %s\n", rule.PrimarySelector, rule.Sqref, rule.Type, operator, priority, strings.Join(rule.Formulas, ", "))
		}
		return writeXLSXOutput(cmd, []byte(strings.TrimRight(b.String(), "\n")))
	},
}

var xlsxConditionalFormatsShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Show one conditional-formatting rule",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if strings.TrimSpace(xlsxCFShowRule) == "" {
			return InvalidArgsError("--rule is required")
		}
		pkg, _, sheetRef, closeFn, err := resolveXLSXWorksheet(filePath, xlsxCFShowSheet)
		if err != nil {
			return err
		}
		defer closeFn()
		blocks, err := mutate.ListConditionalFormats(pkg, sheetRef)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to list conditional formats: %v", err)
		}
		rule, err := mutate.SelectConditionalFormatRule(blocks, xlsxCFShowRule)
		if err != nil {
			if strings.Contains(err.Error(), "ambiguous") {
				return NewCLIErrorf(ExitInvalidArgs, "%v", err)
			}
			return conditionalFormatRuleNotFoundError(blocks, xlsxCFShowRule, sheetRef)
		}
		j := conditionalFormatRuleToJSON(rule)
		if GetGlobalConfig(cmd).Format == "json" {
			return writeJSONResult(cmd, &j, "conditional-formats show")
		}
		operator := ""
		if j.Operator != "" {
			operator = " " + j.Operator
		}
		return writeXLSXOutput(cmd, []byte(fmt.Sprintf("%s %s [%s%s] %s", j.PrimarySelector, j.Sqref, j.Type, operator, strings.Join(j.Formulas, ", "))))
	},
}

func runConditionalFormatMutation(cmd *cobra.Command, filePath, sheetSel, action string, apply func(pkg opc.PackageSession, sheet model.SheetRef) (*mutate.ConditionalFormatMutationResult, error)) error {
	mutOpts, err := GetValidatedMutationOptions(cmd)
	if err != nil {
		return err
	}
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return err
	}
	var result *XLSXConditionalFormatMutationResult
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
			return NewCLIErrorf(ExitInvalidArgs, "failed to %s conditional format: %v", action, err)
		}
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		selector := xlsxSheetSelectorForRef(sheetRef)
		result = &XLSXConditionalFormatMutationResult{
			File:          filePath,
			Sheet:         sheetRef.Name,
			SheetNumber:   sheetRef.Number,
			SheetSelector: selector,
			Action:        action,
			Range:         mutResult.Sqref,
			CellsAffected: mutResult.CellsAffected,
			Output:        destinationFile,
			DryRun:        mutOpts != nil && mutOpts.DryRun,
		}
		j := conditionalFormatRuleToJSON(mutResult.Rule)
		result.Rule = &j
		if destinationFile != "" {
			result.ValidateCommand = xlsxValidateCommand(destinationFile)
			result.ConditionalFormatsListCommand = fmt.Sprintf("ooxml --json xlsx conditional-formats list %s --sheet %s", pptxXLSXCommandArg(destinationFile), pptxXLSXCommandArg(selector))
			if action != "delete" && j.PrimarySelector != "" {
				result.ConditionalFormatsShowCommand = fmt.Sprintf("ooxml --json xlsx conditional-formats show %s --sheet %s --rule %s", pptxXLSXCommandArg(destinationFile), pptxXLSXCommandArg(selector), pptxXLSXCommandArg(j.PrimarySelector))
			}
		}
		return nil
	}); err != nil {
		return err
	}
	if GetGlobalConfig(cmd).Format == "json" {
		return writeJSONResult(cmd, result, "conditional-formats "+action)
	}
	return writeXLSXOutput(cmd, []byte(fmt.Sprintf("%sd conditional format on %s!%s", action, result.Sheet, result.Range)))
}

func normalizeConditionalFormatAddType(ruleType string) string {
	switch strings.TrimSpace(ruleType) {
	case "", "expression":
		return "expression"
	case "cell-is", "cellIs":
		return "cellIs"
	default:
		return strings.TrimSpace(ruleType)
	}
}

var xlsxConditionalFormatsAddCmd = &cobra.Command{
	Use:   "add <file>",
	Short: "Add an expression or cellIs conditional-formatting rule",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		ruleType := normalizeConditionalFormatAddType(xlsxCFType)
		if strings.TrimSpace(xlsxCFRange) == "" {
			return InvalidArgsError("--range is required")
		}
		if strings.TrimSpace(xlsxCFFormula) == "" {
			return InvalidArgsError("--formula is required")
		}
		switch ruleType {
		case "expression":
			if cmd.Flags().Changed("operator") {
				return InvalidArgsError("--operator is only valid with --type cell-is")
			}
			if cmd.Flags().Changed("formula2") {
				return InvalidArgsError("--formula2 is only valid with --type cell-is")
			}
			return runConditionalFormatMutation(cmd, filePath, xlsxCFSheet, "add", func(pkg opc.PackageSession, sheet model.SheetRef) (*mutate.ConditionalFormatMutationResult, error) {
				return mutate.AddConditionalFormatExpression(&mutate.AddConditionalFormatExpressionRequest{
					Package:       pkg,
					SheetRef:      sheet,
					Range:         xlsxCFRange,
					Formula:       xlsxCFFormula,
					Priority:      xlsxCFPriority,
					HasPriority:   cmd.Flags().Changed("priority"),
					StopIfTrue:    xlsxCFStopIfTrue,
					HasStopIfTrue: cmd.Flags().Changed("stop-if-true"),
					DxfID:         xlsxCFDxfID,
					HasDxfID:      cmd.Flags().Changed("dxf-id"),
				})
			})
		case "cellIs":
			return runConditionalFormatMutation(cmd, filePath, xlsxCFSheet, "add", func(pkg opc.PackageSession, sheet model.SheetRef) (*mutate.ConditionalFormatMutationResult, error) {
				return mutate.AddConditionalFormatCellIs(&mutate.AddConditionalFormatCellIsRequest{
					Package:       pkg,
					SheetRef:      sheet,
					Range:         xlsxCFRange,
					Operator:      xlsxCFOperator,
					Formula:       xlsxCFFormula,
					Formula2:      xlsxCFFormula2,
					HasFormula2:   cmd.Flags().Changed("formula2"),
					Priority:      xlsxCFPriority,
					HasPriority:   cmd.Flags().Changed("priority"),
					StopIfTrue:    xlsxCFStopIfTrue,
					HasStopIfTrue: cmd.Flags().Changed("stop-if-true"),
					DxfID:         xlsxCFDxfID,
					HasDxfID:      cmd.Flags().Changed("dxf-id"),
				})
			})
		default:
			return InvalidArgsError("--type must be expression, cell-is, or cellIs")
		}
	},
}

var xlsxConditionalFormatsDeleteCmd = &cobra.Command{
	Use:   "delete <file>",
	Short: "Delete a conditional-formatting rule",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if strings.TrimSpace(xlsxCFRule) == "" {
			return InvalidArgsError("--rule is required")
		}
		return runConditionalFormatMutation(cmd, filePath, xlsxCFSheet, "delete", func(pkg opc.PackageSession, sheet model.SheetRef) (*mutate.ConditionalFormatMutationResult, error) {
			return mutate.DeleteConditionalFormatRule(&mutate.DeleteConditionalFormatRuleRequest{
				Package:      pkg,
				SheetRef:     sheet,
				RuleSelector: xlsxCFRule,
			})
		})
	},
}

func conditionalFormatRuleNotFoundError(blocks []mutate.ConditionalFormatBlock, selector string, sheetRef model.SheetRef) error {
	var candidates []SelectorCandidate
	for _, block := range blocks {
		for _, rule := range block.Rules {
			candidates = append(candidates, SelectorCandidate{Primary: rule.PrimarySelector, Selectors: rule.Selectors})
		}
	}
	discovery := fmt.Sprintf("ooxml --json xlsx conditional-formats list <file> --sheet %s", pptxXLSXCommandArg(xlsxSheetSelectorForRef(sheetRef)))
	if len(candidates) == 0 {
		return SelectorNotFoundError("conditional format rule", selector, nil, discovery)
	}
	return SelectorNotFoundError("conditional format rule", selector, BuildSelectorCandidates(candidates, selector, maxSelectorCandidates), discovery)
}

func init() {
	xlsxConditionalFormatsListCmd.Flags().StringVar(&xlsxCFListSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxConditionalFormatsListCmd.Flags().StringVar(&xlsxCFListRange, "range", "", "optional target range (sqref) filter")
	xlsxConditionalFormatsCmd.AddCommand(xlsxConditionalFormatsListCmd)

	xlsxConditionalFormatsShowCmd.Flags().StringVar(&xlsxCFShowSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxConditionalFormatsShowCmd.Flags().StringVar(&xlsxCFShowRule, "rule", "", "rule selector such as cfRule:1, rule:1, or priority:1")
	xlsxConditionalFormatsCmd.AddCommand(xlsxConditionalFormatsShowCmd)

	for _, c := range []*cobra.Command{xlsxConditionalFormatsAddCmd, xlsxConditionalFormatsDeleteCmd} {
		c.Flags().StringVar(&xlsxCFSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
		AddMutationFlags(c)
	}

	xlsxConditionalFormatsAddCmd.Flags().StringVar(&xlsxCFRange, "range", "", "target range (sqref); space-separated allowed, e.g. \"A1:A10 C1:C5\"")
	xlsxConditionalFormatsAddCmd.Flags().StringVar(&xlsxCFType, "type", "expression", "conditional-formatting rule type: expression|cell-is")
	xlsxConditionalFormatsAddCmd.Flags().StringVar(&xlsxCFOperator, "operator", "", "cellIs operator: between|notBetween|equal|notEqual|greaterThan|lessThan|greaterThanOrEqual|lessThanOrEqual")
	xlsxConditionalFormatsAddCmd.Flags().StringVar(&xlsxCFFormula, "formula", "", "expression formula or first cellIs formula/bound, e.g. A1>0")
	xlsxConditionalFormatsAddCmd.Flags().StringVar(&xlsxCFFormula2, "formula2", "", "second cellIs formula/bound (for between/notBetween)")
	xlsxConditionalFormatsAddCmd.Flags().IntVar(&xlsxCFPriority, "priority", 0, "optional cfRule priority (positive integer)")
	xlsxConditionalFormatsAddCmd.Flags().BoolVar(&xlsxCFStopIfTrue, "stop-if-true", false, "set stopIfTrue on the rule")
	xlsxConditionalFormatsAddCmd.Flags().IntVar(&xlsxCFDxfID, "dxf-id", 0, "optional differential style id to reference")

	xlsxConditionalFormatsDeleteCmd.Flags().StringVar(&xlsxCFRule, "rule", "", "rule selector such as cfRule:1, rule:1, or priority:1")

	xlsxConditionalFormatsCmd.AddCommand(xlsxConditionalFormatsAddCmd)
	xlsxConditionalFormatsCmd.AddCommand(xlsxConditionalFormatsDeleteCmd)
	xlsxCmd.AddCommand(xlsxConditionalFormatsCmd)
}
