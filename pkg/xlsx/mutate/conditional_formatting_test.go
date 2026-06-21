package mutate

import (
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

func TestConditionalFormatsAddExpressionOrdersBeforeDataValidations(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData/>
  <dataValidations count="1"><dataValidation sqref="C1:C3" type="whole"><formula1>1</formula1></dataValidation></dataValidations>
</worksheet>`)
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	result, err := AddConditionalFormatExpression(&AddConditionalFormatExpressionRequest{
		Package:       pkg,
		SheetRef:      sheet,
		Range:         "A1:A5",
		Formula:       "A1>0",
		Priority:      3,
		HasPriority:   true,
		StopIfTrue:    true,
		HasStopIfTrue: true,
		DxfID:         0,
		HasDxfID:      true,
	})
	if err != nil {
		t.Fatalf("AddConditionalFormatExpression failed: %v", err)
	}
	if result.Sqref != "A1:A5" || result.CellsAffected != 5 {
		t.Fatalf("unexpected mutation result: %+v", result)
	}
	if result.Rule.Type != "expression" || result.Rule.Priority != 3 || !result.Rule.StopIfTrue || !result.Rule.HasDxfID || result.Rule.DxfID != 0 {
		t.Fatalf("unexpected added rule: %+v", result.Rule)
	}

	root := readTestWorksheetRoot(t, pkg, workbook)
	var order []string
	for _, child := range root.ChildElements() {
		if namespaces.IsElement(child, namespaces.NsSpreadsheetML, child.Tag) {
			order = append(order, child.Tag)
		}
	}
	got := strings.Join(order, ",")
	if got != "sheetData,conditionalFormatting,dataValidations" {
		t.Fatalf("worksheet child order = %s", got)
	}
	blocks, err := ListConditionalFormats(pkg, sheet)
	if err != nil {
		t.Fatalf("ListConditionalFormats failed: %v", err)
	}
	if len(blocks) != 1 || len(blocks[0].Rules) != 1 || blocks[0].Rules[0].PrimarySelector != "cfRule:1" {
		t.Fatalf("unexpected conditional-format list: %+v", blocks)
	}
}

func TestConditionalFormatsAddCellIsBetween(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData/>
</worksheet>`)
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	result, err := AddConditionalFormatCellIs(&AddConditionalFormatCellIsRequest{
		Package:     pkg,
		SheetRef:    sheet,
		Range:       "B1:B3",
		Operator:    "between",
		Formula:     "1",
		Formula2:    "10",
		HasFormula2: true,
	})
	if err != nil {
		t.Fatalf("AddConditionalFormatCellIs failed: %v", err)
	}
	if result.Sqref != "B1:B3" || result.CellsAffected != 3 {
		t.Fatalf("unexpected mutation result: %+v", result)
	}
	if result.Rule.Type != "cellIs" || result.Rule.Operator != "between" || result.Rule.Priority != 1 {
		t.Fatalf("unexpected added rule: %+v", result.Rule)
	}
	if len(result.Rule.Formulas) != 2 || result.Rule.Formulas[0] != "1" || result.Rule.Formulas[1] != "10" {
		t.Fatalf("unexpected formulas: %+v", result.Rule.Formulas)
	}

	blocks, err := ListConditionalFormats(pkg, sheet)
	if err != nil {
		t.Fatalf("ListConditionalFormats failed: %v", err)
	}
	if len(blocks) != 1 || len(blocks[0].Rules) != 1 {
		t.Fatalf("unexpected conditional-format list: %+v", blocks)
	}
	rule := blocks[0].Rules[0]
	if rule.Type != "cellIs" || rule.Operator != "between" || len(rule.Formulas) != 2 {
		t.Fatalf("unexpected rule readback: %+v", rule)
	}
}

func TestConditionalFormatsCellIsFormula2Validation(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData/>
</worksheet>`)
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	if _, err := AddConditionalFormatCellIs(&AddConditionalFormatCellIsRequest{
		Package:  pkg,
		SheetRef: sheet,
		Range:    "A1:A3",
		Operator: "between",
		Formula:  "1",
	}); err == nil {
		t.Fatalf("expected between without formula2 to fail")
	}
	if _, err := AddConditionalFormatCellIs(&AddConditionalFormatCellIsRequest{
		Package:     pkg,
		SheetRef:    sheet,
		Range:       "A1:A3",
		Operator:    "greaterThan",
		Formula:     "1",
		Formula2:    "10",
		HasFormula2: true,
	}); err == nil {
		t.Fatalf("expected formula2 with non-between operator to fail")
	}
	if _, err := AddConditionalFormatCellIs(&AddConditionalFormatCellIsRequest{
		Package:  pkg,
		SheetRef: sheet,
		Range:    "A1:A3",
		Operator: "containsText",
		Formula:  "1",
	}); err == nil {
		t.Fatalf("expected invalid operator to fail")
	}
}

func TestConditionalFormatsPreserveUnsupportedAndDeleteOneRule(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData/>
  <conditionalFormatting sqref="B1:B5">
    <cfRule type="colorScale" priority="1">
      <colorScale>
        <cfvo type="min"/>
        <cfvo type="max"/>
        <color rgb="FFFF0000"/>
        <color rgb="FF00FF00"/>
      </colorScale>
    </cfRule>
  </conditionalFormatting>
</worksheet>`)
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	if _, err := AddConditionalFormatExpression(&AddConditionalFormatExpressionRequest{
		Package:  pkg,
		SheetRef: sheet,
		Range:    "C1:C5",
		Formula:  "C1<>0",
	}); err != nil {
		t.Fatalf("AddConditionalFormatExpression failed: %v", err)
	}
	blocks, err := ListConditionalFormats(pkg, sheet)
	if err != nil {
		t.Fatalf("ListConditionalFormats failed: %v", err)
	}
	if len(blocks) != 2 || blocks[0].Rules[0].Type != "colorScale" || blocks[1].Rules[0].Type != "expression" {
		t.Fatalf("unsupported rule was not preserved/read back: %+v", blocks)
	}
	if _, err := DeleteConditionalFormatRule(&DeleteConditionalFormatRuleRequest{
		Package:      pkg,
		SheetRef:     sheet,
		RuleSelector: blocks[1].Rules[0].PrimarySelector,
	}); err != nil {
		t.Fatalf("DeleteConditionalFormatRule failed: %v", err)
	}
	blocks, err = ListConditionalFormats(pkg, sheet)
	if err != nil {
		t.Fatalf("ListConditionalFormats after delete failed: %v", err)
	}
	if len(blocks) != 1 || len(blocks[0].Rules) != 1 || blocks[0].Rules[0].Type != "colorScale" {
		t.Fatalf("delete removed the wrong rule/block: %+v", blocks)
	}
}
