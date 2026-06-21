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

func TestConditionalFormatsAddColorScaleThreeColor(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData/>
  <dataValidations count="1"><dataValidation sqref="D1:D3" type="whole"><formula1>1</formula1></dataValidation></dataValidations>
</worksheet>`)
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	result, err := AddConditionalFormatColorScale(&AddConditionalFormatColorScaleRequest{
		Package:  pkg,
		SheetRef: sheet,
		Range:    "C1:C3",
		CFVO: []ConditionalFormatCFVO{
			{Type: "min"},
			{Type: "percentile", Value: "50"},
			{Type: "max"},
		},
		Colors: []ConditionalFormatColor{
			{RGB: "F8696B"},
			{RGB: "#FFEB84"},
			{RGB: "FF63BE7B"},
		},
		Priority:    9,
		HasPriority: true,
	})
	if err != nil {
		t.Fatalf("AddConditionalFormatColorScale failed: %v", err)
	}
	if result.Sqref != "C1:C3" || result.CellsAffected != 3 {
		t.Fatalf("unexpected mutation result: %+v", result)
	}
	if result.Rule.Type != "colorScale" || result.Rule.Priority != 9 || result.Rule.ColorScale == nil {
		t.Fatalf("unexpected added rule: %+v", result.Rule)
	}
	scale := result.Rule.ColorScale
	if len(scale.CFVO) != 3 || scale.CFVO[1].Type != "percentile" || scale.CFVO[1].Value != "50" {
		t.Fatalf("unexpected color-scale cfvo readback: %+v", scale.CFVO)
	}
	if len(scale.Colors) != 3 {
		t.Fatalf("unexpected color-scale colors readback: %+v", scale.Colors)
	}
	gotColors := []string{scale.Colors[0].RGB, scale.Colors[1].RGB, scale.Colors[2].RGB}
	wantColors := []string{"FFF8696B", "FFFFEB84", "FF63BE7B"}
	if !stringSlicesEqual(gotColors, wantColors) {
		t.Fatalf("color-scale colors = %+v, want %+v", gotColors, wantColors)
	}

	root := readTestWorksheetRoot(t, pkg, workbook)
	var order []string
	for _, child := range root.ChildElements() {
		if namespaces.IsElement(child, namespaces.NsSpreadsheetML, child.Tag) {
			order = append(order, child.Tag)
		}
	}
	if got := strings.Join(order, ","); got != "sheetData,conditionalFormatting,dataValidations" {
		t.Fatalf("worksheet child order = %s", got)
	}
}

func TestConditionalFormatsAddDataBar(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData/>
  <dataValidations count="1"><dataValidation sqref="D1:D3" type="whole"><formula1>1</formula1></dataValidation></dataValidations>
</worksheet>`)
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	result, err := AddConditionalFormatDataBar(&AddConditionalFormatDataBarRequest{
		Package:  pkg,
		SheetRef: sheet,
		Range:    "D1:D5",
		CFVO: []ConditionalFormatCFVO{
			{Type: "min"},
			{Type: "max"},
		},
		Colors: []ConditionalFormatColor{
			{RGB: "638EC6"},
		},
		Priority:    7,
		HasPriority: true,
	})
	if err != nil {
		t.Fatalf("AddConditionalFormatDataBar failed: %v", err)
	}
	if result.Sqref != "D1:D5" || result.CellsAffected != 5 {
		t.Fatalf("unexpected mutation result: %+v", result)
	}
	if result.Rule.Type != "dataBar" || result.Rule.Priority != 7 || result.Rule.DataBar == nil {
		t.Fatalf("unexpected added rule: %+v", result.Rule)
	}
	bar := result.Rule.DataBar
	if len(bar.CFVO) != 2 || bar.CFVO[0].Type != "min" || bar.CFVO[1].Type != "max" {
		t.Fatalf("unexpected data-bar cfvo readback: %+v", bar.CFVO)
	}
	if bar.Color.RGB != "FF638EC6" {
		t.Fatalf("data-bar color = %q, want FF638EC6", bar.Color.RGB)
	}

	root := readTestWorksheetRoot(t, pkg, workbook)
	var order []string
	for _, child := range root.ChildElements() {
		if namespaces.IsElement(child, namespaces.NsSpreadsheetML, child.Tag) {
			order = append(order, child.Tag)
		}
	}
	if got := strings.Join(order, ","); got != "sheetData,conditionalFormatting,dataValidations" {
		t.Fatalf("worksheet child order = %s", got)
	}
}

func TestConditionalFormatsListExistingDataBar(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData/>
  <conditionalFormatting sqref="D1:D5">
    <cfRule type="dataBar" priority="7">
      <dataBar>
        <cfvo type="min"/>
        <cfvo type="max"/>
        <color rgb="FF638EC6"/>
      </dataBar>
    </cfRule>
  </conditionalFormatting>
</worksheet>`)
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	blocks, err := ListConditionalFormats(pkg, sheet)
	if err != nil {
		t.Fatalf("ListConditionalFormats failed: %v", err)
	}
	if len(blocks) != 1 || len(blocks[0].Rules) != 1 {
		t.Fatalf("unexpected conditional-format list: %+v", blocks)
	}
	rule := blocks[0].Rules[0]
	if rule.Type != "dataBar" || rule.DataBar == nil {
		t.Fatalf("expected dataBar readback, got %+v", rule)
	}
	if len(rule.DataBar.CFVO) != 2 || rule.DataBar.CFVO[0].Type != "min" || rule.DataBar.CFVO[1].Type != "max" {
		t.Fatalf("unexpected dataBar cfvo readback: %+v", rule.DataBar.CFVO)
	}
	if rule.DataBar.Color.RGB != "FF638EC6" {
		t.Fatalf("unexpected dataBar color readback: %+v", rule.DataBar.Color)
	}
}

func TestConditionalFormatsColorScaleValidation(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData/>
</worksheet>`)
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	cases := []struct {
		name   string
		cfvo   []ConditionalFormatCFVO
		colors []ConditionalFormatColor
		want   string
	}{
		{
			name:   "one point",
			cfvo:   []ConditionalFormatCFVO{{Type: "min"}},
			colors: []ConditionalFormatColor{{RGB: "FF0000"}},
			want:   "exactly 2 or 3",
		},
		{
			name:   "mismatched colors",
			cfvo:   []ConditionalFormatCFVO{{Type: "min"}, {Type: "max"}},
			colors: []ConditionalFormatColor{{RGB: "FF0000"}},
			want:   "same number of --color and --cfvo",
		},
		{
			name:   "min has value",
			cfvo:   []ConditionalFormatCFVO{{Type: "min", Value: "0"}, {Type: "max"}},
			colors: []ConditionalFormatColor{{RGB: "FF0000"}, {RGB: "00FF00"}},
			want:   "must not include a value",
		},
		{
			name:   "percent out of range",
			cfvo:   []ConditionalFormatCFVO{{Type: "percent", Value: "101"}, {Type: "max"}},
			colors: []ConditionalFormatColor{{RGB: "FF0000"}, {RGB: "00FF00"}},
			want:   "between 0 and 100",
		},
		{
			name:   "invalid color",
			cfvo:   []ConditionalFormatCFVO{{Type: "min"}, {Type: "max"}},
			colors: []ConditionalFormatColor{{RGB: "nope"}, {RGB: "00FF00"}},
			want:   "invalid color",
		},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			_, err := AddConditionalFormatColorScale(&AddConditionalFormatColorScaleRequest{
				Package:  pkg,
				SheetRef: sheet,
				Range:    "A1:A3",
				CFVO:     tc.cfvo,
				Colors:   tc.colors,
			})
			if err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("expected %q error, got %v", tc.want, err)
			}
		})
	}
}

func TestConditionalFormatsDataBarValidation(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData/>
</worksheet>`)
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	cases := []struct {
		name   string
		cfvo   []ConditionalFormatCFVO
		colors []ConditionalFormatColor
		want   string
	}{
		{
			name:   "one cfvo",
			cfvo:   []ConditionalFormatCFVO{{Type: "min"}},
			colors: []ConditionalFormatColor{{RGB: "638EC6"}},
			want:   "exactly 2 --cfvo",
		},
		{
			name:   "two colors",
			cfvo:   []ConditionalFormatCFVO{{Type: "min"}, {Type: "max"}},
			colors: []ConditionalFormatColor{{RGB: "638EC6"}, {RGB: "FF0000"}},
			want:   "exactly 1 --color",
		},
		{
			name:   "min has value",
			cfvo:   []ConditionalFormatCFVO{{Type: "min", Value: "0"}, {Type: "max"}},
			colors: []ConditionalFormatColor{{RGB: "638EC6"}},
			want:   "must not include a value",
		},
		{
			name:   "invalid color",
			cfvo:   []ConditionalFormatCFVO{{Type: "min"}, {Type: "max"}},
			colors: []ConditionalFormatColor{{RGB: "nope"}},
			want:   "invalid color",
		},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			_, err := AddConditionalFormatDataBar(&AddConditionalFormatDataBarRequest{
				Package:  pkg,
				SheetRef: sheet,
				Range:    "D1:D5",
				CFVO:     tc.cfvo,
				Colors:   tc.colors,
			})
			if err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("expected %q error, got %v", tc.want, err)
			}
		})
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
