package mutate

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

// ConditionalFormatBlock describes one worksheet conditionalFormatting block.
type ConditionalFormatBlock struct {
	Index int                     `json:"index"`
	Sqref string                  `json:"sqref"`
	Rules []ConditionalFormatRule `json:"rules"`
}

// ConditionalFormatRule describes one cfRule inside a conditionalFormatting block.
type ConditionalFormatRule struct {
	Index           int      `json:"index"`
	BlockIndex      int      `json:"blockIndex"`
	RuleIndex       int      `json:"ruleIndex"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
	Sqref           string   `json:"sqref"`
	Type            string   `json:"type,omitempty"`
	Priority        int      `json:"priority,omitempty"`
	Formulas        []string `json:"formulas,omitempty"`
	DxfID           int      `json:"dxfId,omitempty"`
	HasDxfID        bool     `json:"hasDxfId,omitempty"`
	StopIfTrue      bool     `json:"stopIfTrue,omitempty"`
}

// AddConditionalFormatExpressionRequest creates an expression cfRule.
type AddConditionalFormatExpressionRequest struct {
	Package       opc.PackageSession
	SheetRef      model.SheetRef
	Range         string
	Formula       string
	Priority      int
	HasPriority   bool
	StopIfTrue    bool
	HasStopIfTrue bool
	DxfID         int
	HasDxfID      bool
}

// DeleteConditionalFormatRuleRequest removes one cfRule by selector.
type DeleteConditionalFormatRuleRequest struct {
	Package      opc.PackageSession
	SheetRef     model.SheetRef
	RuleSelector string
}

// ConditionalFormatMutationResult reports a conditional-formatting mutation.
type ConditionalFormatMutationResult struct {
	Sqref         string
	Rule          ConditionalFormatRule
	CellsAffected int
}

// ListConditionalFormats returns worksheet conditional-formatting blocks.
func ListConditionalFormats(session opc.PackageSession, sheet model.SheetRef) ([]ConditionalFormatBlock, error) {
	_, root, err := readWorksheetRoot(session, sheet)
	if err != nil {
		return nil, err
	}
	return conditionalFormatsFromRoot(root), nil
}

func conditionalFormatsFromRoot(root *etree.Element) []ConditionalFormatBlock {
	var out []ConditionalFormatBlock
	globalRuleIndex := 0
	for _, cf := range namespaces.FindChildren(root, namespaces.NsSpreadsheetML, "conditionalFormatting") {
		block := ConditionalFormatBlock{
			Index: len(out) + 1,
			Sqref: cf.SelectAttrValue("sqref", ""),
		}
		for _, ruleElem := range namespaces.FindChildren(cf, namespaces.NsSpreadsheetML, "cfRule") {
			globalRuleIndex++
			block.Rules = append(block.Rules, conditionalFormatRuleFromElem(ruleElem, block.Index, len(block.Rules)+1, globalRuleIndex, block.Sqref))
		}
		out = append(out, block)
	}
	return out
}

func conditionalFormatRuleFromElem(rule *etree.Element, blockIndex, ruleIndex, globalIndex int, sqref string) ConditionalFormatRule {
	entry := ConditionalFormatRule{
		Index:           globalIndex,
		BlockIndex:      blockIndex,
		RuleIndex:       ruleIndex,
		PrimarySelector: fmt.Sprintf("cfRule:%d", globalIndex),
		Sqref:           sqref,
		Type:            rule.SelectAttrValue("type", ""),
		StopIfTrue:      attrIsTrue(rule, "stopIfTrue"),
	}
	entry.Selectors = conditionalFormatRuleSelectors(entry)
	if priorityText := rule.SelectAttrValue("priority", ""); priorityText != "" {
		if priority, err := strconv.Atoi(priorityText); err == nil {
			entry.Priority = priority
		}
	}
	if dxfText := rule.SelectAttrValue("dxfId", ""); dxfText != "" {
		if dxfID, err := strconv.Atoi(dxfText); err == nil {
			entry.DxfID = dxfID
			entry.HasDxfID = true
		}
	}
	for _, formula := range namespaces.FindChildren(rule, namespaces.NsSpreadsheetML, "formula") {
		entry.Formulas = append(entry.Formulas, formula.Text())
	}
	entry.Selectors = conditionalFormatRuleSelectors(entry)
	return entry
}

func conditionalFormatRuleSelectors(rule ConditionalFormatRule) []string {
	selectors := []string{
		fmt.Sprintf("cfRule:%d", rule.Index),
		fmt.Sprintf("rule:%d", rule.Index),
		fmt.Sprintf("block:%d/rule:%d", rule.BlockIndex, rule.RuleIndex),
	}
	if rule.Priority > 0 {
		selectors = append(selectors, fmt.Sprintf("priority:%d", rule.Priority))
	}
	if rule.Sqref != "" {
		selectors = append(selectors, "sqref:"+rule.Sqref)
	}
	return selectors
}

func nextConditionalFormatPriority(root *etree.Element) int {
	maxPriority := 0
	ruleCount := 0
	for _, block := range conditionalFormatsFromRoot(root) {
		for _, rule := range block.Rules {
			ruleCount++
			if rule.Priority > maxPriority {
				maxPriority = rule.Priority
			}
		}
	}
	if maxPriority > 0 {
		return maxPriority + 1
	}
	return ruleCount + 1
}

func findConditionalFormattingBlock(root *etree.Element, normSqref string) *etree.Element {
	for _, cf := range namespaces.FindChildren(root, namespaces.NsSpreadsheetML, "conditionalFormatting") {
		existing, err := NormalizeSqref(cf.SelectAttrValue("sqref", ""))
		if err == nil && existing == normSqref {
			return cf
		}
	}
	return nil
}

// AddConditionalFormatExpression adds an expression conditional-formatting rule.
func AddConditionalFormatExpression(req *AddConditionalFormatExpressionRequest) (*ConditionalFormatMutationResult, error) {
	if req == nil {
		return nil, fmt.Errorf("add conditional format request is nil")
	}
	normSqref, err := NormalizeSqref(req.Range)
	if err != nil {
		return nil, err
	}
	formula := strings.TrimSpace(req.Formula)
	if formula == "" {
		return nil, fmt.Errorf("--formula is required")
	}
	if req.HasPriority && req.Priority < 1 {
		return nil, fmt.Errorf("--priority must be greater than zero")
	}
	if req.HasDxfID && req.DxfID < 0 {
		return nil, fmt.Errorf("--dxf-id must be zero or greater")
	}
	doc, root, err := readWorksheetRoot(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	prefix := root.Space
	container := findConditionalFormattingBlock(root, normSqref)
	if container == nil {
		container = newElement(prefix, "conditionalFormatting")
		container.CreateAttr("sqref", normSqref)
		insertWorksheetChild(root, container, "conditionalFormatting")
	}

	rule := newElement(prefix, "cfRule")
	rule.CreateAttr("type", "expression")
	priority := req.Priority
	if !req.HasPriority {
		priority = nextConditionalFormatPriority(root)
	}
	rule.CreateAttr("priority", strconv.Itoa(priority))
	if req.HasStopIfTrue {
		setBoolAttr(rule, "stopIfTrue", req.StopIfTrue)
	}
	if req.HasDxfID {
		rule.CreateAttr("dxfId", strconv.Itoa(req.DxfID))
	}
	formulaElem := newElement(prefix, "formula")
	formulaElem.SetText(formula)
	rule.AddChild(formulaElem)
	container.AddChild(rule)

	blocks := conditionalFormatsFromRoot(root)
	added := findAddedConditionalFormatRule(blocks, normSqref, priority, formula)
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return &ConditionalFormatMutationResult{Sqref: normSqref, Rule: added, CellsAffected: sqrefCellCount(normSqref)}, nil
}

func findAddedConditionalFormatRule(blocks []ConditionalFormatBlock, sqref string, priority int, formula string) ConditionalFormatRule {
	for i := len(blocks) - 1; i >= 0; i-- {
		if existing, err := NormalizeSqref(blocks[i].Sqref); err != nil || existing != sqref {
			continue
		}
		for j := len(blocks[i].Rules) - 1; j >= 0; j-- {
			rule := blocks[i].Rules[j]
			if rule.Priority == priority && len(rule.Formulas) == 1 && rule.Formulas[0] == formula {
				return rule
			}
		}
	}
	return ConditionalFormatRule{}
}

// DeleteConditionalFormatRule removes one cfRule by selector and drops an empty block.
func DeleteConditionalFormatRule(req *DeleteConditionalFormatRuleRequest) (*ConditionalFormatMutationResult, error) {
	if req == nil {
		return nil, fmt.Errorf("delete conditional format request is nil")
	}
	selector := strings.TrimSpace(req.RuleSelector)
	if selector == "" {
		return nil, fmt.Errorf("--rule is required")
	}
	doc, root, err := readWorksheetRoot(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	cf, ruleElem, rule, err := findConditionalFormatRuleElement(root, selector)
	if err != nil {
		return nil, err
	}
	cf.RemoveChild(ruleElem)
	if len(namespaces.FindChildren(cf, namespaces.NsSpreadsheetML, "cfRule")) == 0 {
		root.RemoveChild(cf)
	}
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return &ConditionalFormatMutationResult{Sqref: rule.Sqref, Rule: rule, CellsAffected: sqrefCellCount(rule.Sqref)}, nil
}

func findConditionalFormatRuleElement(root *etree.Element, selector string) (*etree.Element, *etree.Element, ConditionalFormatRule, error) {
	blocks := conditionalFormatsFromRoot(root)
	target, err := SelectConditionalFormatRule(blocks, selector)
	if err != nil {
		return nil, nil, ConditionalFormatRule{}, err
	}
	blockIndex := 0
	for _, cf := range namespaces.FindChildren(root, namespaces.NsSpreadsheetML, "conditionalFormatting") {
		blockIndex++
		if blockIndex != target.BlockIndex {
			continue
		}
		ruleIndex := 0
		for _, ruleElem := range namespaces.FindChildren(cf, namespaces.NsSpreadsheetML, "cfRule") {
			ruleIndex++
			if ruleIndex == target.RuleIndex {
				return cf, ruleElem, target, nil
			}
		}
	}
	return nil, nil, ConditionalFormatRule{}, fmt.Errorf("conditional format rule %q disappeared during lookup", selector)
}

// SelectConditionalFormatRule resolves a stable cfRule selector against listed blocks.
func SelectConditionalFormatRule(blocks []ConditionalFormatBlock, selector string) (ConditionalFormatRule, error) {
	var matches []ConditionalFormatRule
	for _, block := range blocks {
		for _, rule := range block.Rules {
			if conditionalFormatRuleMatches(rule, selector) {
				matches = append(matches, rule)
			}
		}
	}
	if len(matches) == 0 {
		return ConditionalFormatRule{}, fmt.Errorf("no conditional format rule found for %q", selector)
	}
	if len(matches) > 1 {
		return ConditionalFormatRule{}, fmt.Errorf("conditional format rule selector %q is ambiguous", selector)
	}
	return matches[0], nil
}

func conditionalFormatRuleMatches(rule ConditionalFormatRule, selector string) bool {
	selector = strings.TrimSpace(selector)
	if selector == "" {
		return false
	}
	if n, err := strconv.Atoi(selector); err == nil {
		return rule.Index == n
	}
	for _, candidate := range rule.Selectors {
		if candidate == selector {
			return true
		}
	}
	return false
}
