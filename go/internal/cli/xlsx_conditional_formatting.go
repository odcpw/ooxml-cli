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
	Long:    "Commands for listing, showing, adding, deleting, and reordering worksheet conditional-formatting expression, cellIs, color-scale, data-bar, and icon-set rules.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

type XLSXConditionalFormatCFVOJSON struct {
	Type  string `json:"type"`
	Value string `json:"value,omitempty"`
}

type XLSXConditionalFormatColorJSON struct {
	RGB string `json:"rgb"`
}

type XLSXConditionalFormatColorScaleJSON struct {
	CFVO   []XLSXConditionalFormatCFVOJSON  `json:"cfvo"`
	Colors []XLSXConditionalFormatColorJSON `json:"colors"`
}

type XLSXConditionalFormatDataBarJSON struct {
	CFVO  []XLSXConditionalFormatCFVOJSON `json:"cfvo"`
	Color XLSXConditionalFormatColorJSON  `json:"color"`
}

type XLSXConditionalFormatIconSetJSON struct {
	IconSet   string                          `json:"iconSet"`
	CFVO      []XLSXConditionalFormatCFVOJSON `json:"cfvo"`
	ShowValue *bool                           `json:"showValue,omitempty"`
	Percent   *bool                           `json:"percent,omitempty"`
	Reverse   *bool                           `json:"reverse,omitempty"`
}

type conditionalFormatRepeatedFlag []string

func (f *conditionalFormatRepeatedFlag) String() string {
	if f == nil {
		return ""
	}
	return strings.Join(*f, ",")
}

func (f *conditionalFormatRepeatedFlag) Set(value string) error {
	if value == "" {
		*f = nil
		return nil
	}
	*f = append(*f, value)
	return nil
}

func (f *conditionalFormatRepeatedFlag) Type() string {
	return "stringArray"
}

type XLSXConditionalFormatRuleJSON struct {
	Index           int                                  `json:"index"`
	BlockIndex      int                                  `json:"blockIndex"`
	RuleIndex       int                                  `json:"ruleIndex"`
	PrimarySelector string                               `json:"primarySelector,omitempty"`
	Selectors       []string                             `json:"selectors,omitempty"`
	Sqref           string                               `json:"sqref"`
	Type            string                               `json:"type,omitempty"`
	Operator        string                               `json:"operator,omitempty"`
	Priority        *int                                 `json:"priority,omitempty"`
	Formula         string                               `json:"formula,omitempty"`
	Formulas        []string                             `json:"formulas,omitempty"`
	DxfID           *int                                 `json:"dxfId,omitempty"`
	StopIfTrue      bool                                 `json:"stopIfTrue,omitempty"`
	ColorScale      *XLSXConditionalFormatColorScaleJSON `json:"colorScale,omitempty"`
	DataBar         *XLSXConditionalFormatDataBarJSON    `json:"dataBar,omitempty"`
	IconSet         *XLSXConditionalFormatIconSetJSON    `json:"iconSet,omitempty"`
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
	OldPriority                   *int                           `json:"oldPriority,omitempty"`
	NewPriority                   *int                           `json:"newPriority,omitempty"`
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
	xlsxCFIconSet    string
	xlsxCFCFVO       conditionalFormatRepeatedFlag
	xlsxCFColor      conditionalFormatRepeatedFlag
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
	if rule.ColorScale != nil {
		scale := &XLSXConditionalFormatColorScaleJSON{}
		for _, cfvo := range rule.ColorScale.CFVO {
			scale.CFVO = append(scale.CFVO, XLSXConditionalFormatCFVOJSON{Type: cfvo.Type, Value: cfvo.Value})
		}
		for _, color := range rule.ColorScale.Colors {
			scale.Colors = append(scale.Colors, XLSXConditionalFormatColorJSON{RGB: color.RGB})
		}
		out.ColorScale = scale
	}
	if rule.DataBar != nil {
		bar := &XLSXConditionalFormatDataBarJSON{
			Color: XLSXConditionalFormatColorJSON{RGB: rule.DataBar.Color.RGB},
		}
		for _, cfvo := range rule.DataBar.CFVO {
			bar.CFVO = append(bar.CFVO, XLSXConditionalFormatCFVOJSON{Type: cfvo.Type, Value: cfvo.Value})
		}
		out.DataBar = bar
	}
	if rule.IconSet != nil {
		icons := &XLSXConditionalFormatIconSetJSON{
			IconSet:   rule.IconSet.IconSet,
			ShowValue: rule.IconSet.ShowValue,
			Percent:   rule.IconSet.Percent,
			Reverse:   rule.IconSet.Reverse,
		}
		for _, cfvo := range rule.IconSet.CFVO {
			icons.CFVO = append(icons.CFVO, XLSXConditionalFormatCFVOJSON{Type: cfvo.Type, Value: cfvo.Value})
		}
		out.IconSet = icons
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

func conditionalFormatRuleSummary(rule XLSXConditionalFormatRuleJSON) string {
	if rule.ColorScale != nil {
		parts := make([]string, 0, len(rule.ColorScale.CFVO))
		for i, cfvo := range rule.ColorScale.CFVO {
			text := cfvo.Type
			if cfvo.Value != "" {
				text += ":" + cfvo.Value
			}
			if i < len(rule.ColorScale.Colors) {
				text += "=" + rule.ColorScale.Colors[i].RGB
			}
			parts = append(parts, text)
		}
		return strings.Join(parts, ", ")
	}
	if rule.DataBar != nil {
		parts := make([]string, 0, len(rule.DataBar.CFVO)+1)
		for _, cfvo := range rule.DataBar.CFVO {
			text := cfvo.Type
			if cfvo.Value != "" {
				text += ":" + cfvo.Value
			}
			parts = append(parts, text)
		}
		if rule.DataBar.Color.RGB != "" {
			parts = append(parts, rule.DataBar.Color.RGB)
		}
		return strings.Join(parts, ", ")
	}
	if rule.IconSet != nil {
		parts := make([]string, 0, len(rule.IconSet.CFVO)+4)
		if rule.IconSet.IconSet != "" {
			parts = append(parts, rule.IconSet.IconSet)
		}
		for _, cfvo := range rule.IconSet.CFVO {
			text := cfvo.Type
			if cfvo.Value != "" {
				text += ":" + cfvo.Value
			}
			parts = append(parts, text)
		}
		if rule.IconSet.ShowValue != nil {
			parts = append(parts, fmt.Sprintf("showValue=%t", *rule.IconSet.ShowValue))
		}
		if rule.IconSet.Percent != nil {
			parts = append(parts, fmt.Sprintf("percent=%t", *rule.IconSet.Percent))
		}
		if rule.IconSet.Reverse != nil {
			parts = append(parts, fmt.Sprintf("reverse=%t", *rule.IconSet.Reverse))
		}
		return strings.Join(parts, ", ")
	}
	return strings.Join(rule.Formulas, ", ")
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
			fmt.Fprintf(&b, "  %s %s [%s%s]%s %s\n", rule.PrimarySelector, rule.Sqref, rule.Type, operator, priority, conditionalFormatRuleSummary(rule))
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
		return writeXLSXOutput(cmd, []byte(fmt.Sprintf("%s %s [%s%s] %s", j.PrimarySelector, j.Sqref, j.Type, operator, conditionalFormatRuleSummary(j))))
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
		if mutResult.OldPriority > 0 {
			oldPriority := mutResult.OldPriority
			result.OldPriority = &oldPriority
		}
		if mutResult.NewPriority > 0 {
			newPriority := mutResult.NewPriority
			result.NewPriority = &newPriority
		}
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
	pastAction := action + "d"
	if action == "reorder" {
		pastAction = "reordered"
	}
	return writeXLSXOutput(cmd, []byte(fmt.Sprintf("%s conditional format on %s!%s", pastAction, result.Sheet, result.Range)))
}

func normalizeConditionalFormatAddType(ruleType string) string {
	switch strings.TrimSpace(ruleType) {
	case "", "expression":
		return "expression"
	case "cell-is", "cellIs":
		return "cellIs"
	case "color-scale", "colorScale":
		return "colorScale"
	case "data-bar", "dataBar":
		return "dataBar"
	case "icon-set", "iconSet":
		return "iconSet"
	default:
		return strings.TrimSpace(ruleType)
	}
}

func parseConditionalFormatCFVOFlags(values []string) ([]mutate.ConditionalFormatCFVO, error) {
	out := make([]mutate.ConditionalFormatCFVO, 0, len(values))
	for _, value := range values {
		if strings.TrimSpace(value) == "[]" {
			continue
		}
		cfvo, err := mutate.ParseConditionalFormatCFVO(value)
		if err != nil {
			return nil, err
		}
		out = append(out, cfvo)
	}
	return out, nil
}

func parseConditionalFormatColorFlags(values []string) []mutate.ConditionalFormatColor {
	out := make([]mutate.ConditionalFormatColor, 0, len(values))
	for _, value := range values {
		if strings.TrimSpace(value) == "[]" {
			continue
		}
		out = append(out, mutate.ConditionalFormatColor{RGB: value})
	}
	return out
}

var xlsxConditionalFormatsAddCmd = &cobra.Command{
	Use:   "add <file>",
	Short: "Add an expression, cellIs, color-scale, data-bar, or icon-set conditional-formatting rule",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		ruleType := normalizeConditionalFormatAddType(xlsxCFType)
		if strings.TrimSpace(xlsxCFRange) == "" {
			return InvalidArgsError("--range is required")
		}
		switch ruleType {
		case "expression":
			if strings.TrimSpace(xlsxCFFormula) == "" {
				return InvalidArgsError("--formula is required")
			}
			if cmd.Flags().Changed("operator") {
				return InvalidArgsError("--operator is only valid with --type cell-is")
			}
			if cmd.Flags().Changed("formula2") {
				return InvalidArgsError("--formula2 is only valid with --type cell-is")
			}
			if cmd.Flags().Changed("cfvo") {
				return InvalidArgsError("--cfvo is only valid with --type color-scale, data-bar, or icon-set")
			}
			if cmd.Flags().Changed("color") {
				return InvalidArgsError("--color is only valid with --type color-scale or data-bar")
			}
			if cmd.Flags().Changed("icon-set") {
				return InvalidArgsError("--icon-set is only valid with --type icon-set")
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
			if strings.TrimSpace(xlsxCFFormula) == "" {
				return InvalidArgsError("--formula is required")
			}
			if cmd.Flags().Changed("cfvo") {
				return InvalidArgsError("--cfvo is only valid with --type color-scale, data-bar, or icon-set")
			}
			if cmd.Flags().Changed("color") {
				return InvalidArgsError("--color is only valid with --type color-scale or data-bar")
			}
			if cmd.Flags().Changed("icon-set") {
				return InvalidArgsError("--icon-set is only valid with --type icon-set")
			}
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
		case "colorScale":
			if cmd.Flags().Changed("operator") {
				return InvalidArgsError("--operator is only valid with --type cell-is")
			}
			if cmd.Flags().Changed("formula") || cmd.Flags().Changed("formula2") {
				return InvalidArgsError("--formula and --formula2 are not valid with --type color-scale")
			}
			if cmd.Flags().Changed("stop-if-true") {
				return InvalidArgsError("--stop-if-true is not valid with --type color-scale")
			}
			if cmd.Flags().Changed("dxf-id") {
				return InvalidArgsError("--dxf-id is not valid with --type color-scale")
			}
			if cmd.Flags().Changed("icon-set") {
				return InvalidArgsError("--icon-set is only valid with --type icon-set")
			}
			cfvos, err := parseConditionalFormatCFVOFlags([]string(xlsxCFCFVO))
			if err != nil {
				return InvalidArgsError(err.Error())
			}
			colors := parseConditionalFormatColorFlags([]string(xlsxCFColor))
			return runConditionalFormatMutation(cmd, filePath, xlsxCFSheet, "add", func(pkg opc.PackageSession, sheet model.SheetRef) (*mutate.ConditionalFormatMutationResult, error) {
				return mutate.AddConditionalFormatColorScale(&mutate.AddConditionalFormatColorScaleRequest{
					Package:     pkg,
					SheetRef:    sheet,
					Range:       xlsxCFRange,
					CFVO:        cfvos,
					Colors:      colors,
					Priority:    xlsxCFPriority,
					HasPriority: cmd.Flags().Changed("priority"),
				})
			})
		case "dataBar":
			if cmd.Flags().Changed("operator") {
				return InvalidArgsError("--operator is only valid with --type cell-is")
			}
			if cmd.Flags().Changed("formula") || cmd.Flags().Changed("formula2") {
				return InvalidArgsError("--formula and --formula2 are not valid with --type data-bar")
			}
			if cmd.Flags().Changed("stop-if-true") {
				return InvalidArgsError("--stop-if-true is not valid with --type data-bar")
			}
			if cmd.Flags().Changed("dxf-id") {
				return InvalidArgsError("--dxf-id is not valid with --type data-bar")
			}
			if cmd.Flags().Changed("icon-set") {
				return InvalidArgsError("--icon-set is only valid with --type icon-set")
			}
			cfvos, err := parseConditionalFormatCFVOFlags([]string(xlsxCFCFVO))
			if err != nil {
				return InvalidArgsError(err.Error())
			}
			colors := parseConditionalFormatColorFlags([]string(xlsxCFColor))
			return runConditionalFormatMutation(cmd, filePath, xlsxCFSheet, "add", func(pkg opc.PackageSession, sheet model.SheetRef) (*mutate.ConditionalFormatMutationResult, error) {
				return mutate.AddConditionalFormatDataBar(&mutate.AddConditionalFormatDataBarRequest{
					Package:     pkg,
					SheetRef:    sheet,
					Range:       xlsxCFRange,
					CFVO:        cfvos,
					Colors:      colors,
					Priority:    xlsxCFPriority,
					HasPriority: cmd.Flags().Changed("priority"),
				})
			})
		case "iconSet":
			if cmd.Flags().Changed("operator") {
				return InvalidArgsError("--operator is only valid with --type cell-is")
			}
			if cmd.Flags().Changed("formula") || cmd.Flags().Changed("formula2") {
				return InvalidArgsError("--formula and --formula2 are not valid with --type icon-set")
			}
			if cmd.Flags().Changed("color") {
				return InvalidArgsError("--color is not valid with --type icon-set")
			}
			if cmd.Flags().Changed("stop-if-true") {
				return InvalidArgsError("--stop-if-true is not valid with --type icon-set")
			}
			if cmd.Flags().Changed("dxf-id") {
				return InvalidArgsError("--dxf-id is not valid with --type icon-set")
			}
			cfvos, err := parseConditionalFormatCFVOFlags([]string(xlsxCFCFVO))
			if err != nil {
				return InvalidArgsError(err.Error())
			}
			return runConditionalFormatMutation(cmd, filePath, xlsxCFSheet, "add", func(pkg opc.PackageSession, sheet model.SheetRef) (*mutate.ConditionalFormatMutationResult, error) {
				return mutate.AddConditionalFormatIconSet(&mutate.AddConditionalFormatIconSetRequest{
					Package:     pkg,
					SheetRef:    sheet,
					Range:       xlsxCFRange,
					IconSet:     xlsxCFIconSet,
					CFVO:        cfvos,
					Priority:    xlsxCFPriority,
					HasPriority: cmd.Flags().Changed("priority"),
				})
			})
		default:
			return InvalidArgsError("--type must be expression, cell-is, cellIs, color-scale, colorScale, data-bar, dataBar, icon-set, or iconSet")
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

var xlsxConditionalFormatsReorderCmd = &cobra.Command{
	Use:   "reorder <file>",
	Short: "Reorder a conditional-formatting rule by priority",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if strings.TrimSpace(xlsxCFRule) == "" {
			return InvalidArgsError("--rule is required")
		}
		if !cmd.Flags().Changed("priority") {
			return InvalidArgsError("--priority is required")
		}
		if xlsxCFPriority < 1 {
			return InvalidArgsError("--priority must be greater than zero")
		}
		return runConditionalFormatMutation(cmd, filePath, xlsxCFSheet, "reorder", func(pkg opc.PackageSession, sheet model.SheetRef) (*mutate.ConditionalFormatMutationResult, error) {
			return mutate.ReorderConditionalFormatRule(&mutate.ReorderConditionalFormatRuleRequest{
				Package:      pkg,
				SheetRef:     sheet,
				RuleSelector: xlsxCFRule,
				Priority:     xlsxCFPriority,
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

	for _, c := range []*cobra.Command{xlsxConditionalFormatsAddCmd, xlsxConditionalFormatsDeleteCmd, xlsxConditionalFormatsReorderCmd} {
		c.Flags().StringVar(&xlsxCFSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
		AddMutationFlags(c)
	}

	xlsxConditionalFormatsAddCmd.Flags().StringVar(&xlsxCFRange, "range", "", "target range (sqref); space-separated allowed, e.g. \"A1:A10 C1:C5\"")
	xlsxConditionalFormatsAddCmd.Flags().StringVar(&xlsxCFType, "type", "expression", "conditional-formatting rule type: expression|cell-is|color-scale|data-bar|icon-set")
	xlsxConditionalFormatsAddCmd.Flags().StringVar(&xlsxCFOperator, "operator", "", "cellIs operator: between|notBetween|equal|notEqual|greaterThan|lessThan|greaterThanOrEqual|lessThanOrEqual")
	xlsxConditionalFormatsAddCmd.Flags().StringVar(&xlsxCFFormula, "formula", "", "expression formula or first cellIs formula/bound, e.g. A1>0")
	xlsxConditionalFormatsAddCmd.Flags().StringVar(&xlsxCFFormula2, "formula2", "", "second cellIs formula/bound (for between/notBetween)")
	xlsxConditionalFormatsAddCmd.Flags().StringVar(&xlsxCFIconSet, "icon-set", "", "icon-set name starting with 3, 4, or 5, e.g. 3TrafficLights1")
	xlsxConditionalFormatsAddCmd.Flags().Var(&xlsxCFCFVO, "cfvo", "color-scale/data-bar/icon-set threshold: min|max|num:0|percent:10|percentile:50")
	xlsxConditionalFormatsAddCmd.Flags().Var(&xlsxCFColor, "color", "color-scale/data-bar color hex: #F8696B|FFEB84|FF63BE7B")
	xlsxConditionalFormatsAddCmd.Flags().IntVar(&xlsxCFPriority, "priority", 0, "optional cfRule priority (positive integer)")
	xlsxConditionalFormatsAddCmd.Flags().BoolVar(&xlsxCFStopIfTrue, "stop-if-true", false, "set stopIfTrue on the rule")
	xlsxConditionalFormatsAddCmd.Flags().IntVar(&xlsxCFDxfID, "dxf-id", 0, "optional differential style id to reference")

	xlsxConditionalFormatsDeleteCmd.Flags().StringVar(&xlsxCFRule, "rule", "", "rule selector such as cfRule:1, rule:1, or priority:1")
	xlsxConditionalFormatsReorderCmd.Flags().StringVar(&xlsxCFRule, "rule", "", "rule selector such as cfRule:1, rule:1, block:1/rule:1, or priority:1")
	xlsxConditionalFormatsReorderCmd.Flags().IntVar(&xlsxCFPriority, "priority", 0, "target cfRule priority/order position (1-based)")

	xlsxConditionalFormatsCmd.AddCommand(xlsxConditionalFormatsAddCmd)
	xlsxConditionalFormatsCmd.AddCommand(xlsxConditionalFormatsDeleteCmd)
	xlsxConditionalFormatsCmd.AddCommand(xlsxConditionalFormatsReorderCmd)
	xlsxCmd.AddCommand(xlsxConditionalFormatsCmd)
}
