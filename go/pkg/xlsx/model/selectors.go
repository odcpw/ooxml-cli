package model

import (
	"fmt"
	"strings"
)

// WithSheetSelectors returns a sheet reference with agent-friendly selectors populated.
func WithSheetSelectors(sheet SheetRef) SheetRef {
	primary := ""
	if strings.TrimSpace(sheet.SheetID) != "" {
		primary = "sheetId:" + sheet.SheetID
	} else if sheet.Number > 0 {
		primary = fmt.Sprintf("sheet:%d", sheet.Number)
	} else if strings.TrimSpace(sheet.Name) != "" {
		primary = "name:" + sheet.Name
	}

	builder := selectorBuilder{}
	builder.add(primary)
	if sheet.Number > 0 {
		builder.add(fmt.Sprintf("sheet:%d", sheet.Number))
		builder.add(fmt.Sprintf("#%d", sheet.Number))
	}
	if strings.TrimSpace(sheet.SheetID) != "" {
		builder.add("sheetId:" + sheet.SheetID)
	}
	if strings.TrimSpace(sheet.RelationshipID) != "" {
		builder.add("rId:" + sheet.RelationshipID)
		builder.add("rid:" + sheet.RelationshipID)
	}
	if strings.TrimSpace(sheet.PartURI) != "" {
		builder.add("part:" + sheet.PartURI)
	}
	if strings.TrimSpace(sheet.Name) != "" {
		builder.add("name:" + sheet.Name)
		builder.add("~" + sheet.Name)
		builder.add(sheet.Name)
	}

	sheet.PrimarySelector = primary
	sheet.Selectors = builder.values
	return sheet
}

// WithTableSelectors returns a table reference with agent-friendly selectors populated.
func WithTableSelectors(table TableRef) TableRef {
	primary := ""
	if table.ID > 0 {
		primary = fmt.Sprintf("tableId:%d", table.ID)
	} else if table.Number > 0 {
		primary = fmt.Sprintf("table:%d", table.Number)
	} else if strings.TrimSpace(table.DisplayName) != "" {
		primary = "table:" + table.DisplayName
	}

	builder := selectorBuilder{}
	builder.add(primary)
	if table.Number > 0 {
		builder.add(fmt.Sprintf("table:%d", table.Number))
		builder.add(fmt.Sprintf("#%d", table.Number))
	}
	if strings.TrimSpace(table.DisplayName) != "" {
		builder.add("table:" + table.DisplayName)
		builder.add("displayName:" + table.DisplayName)
		builder.add(table.DisplayName)
	}
	if strings.TrimSpace(table.Name) != "" {
		builder.add("name:" + table.Name)
		builder.add(table.Name)
	}
	if table.ID > 0 {
		builder.add(fmt.Sprintf("tableId:%d", table.ID))
		builder.add(fmt.Sprintf("id:%d", table.ID))
	}
	if strings.TrimSpace(table.RelationshipID) != "" {
		builder.add("rId:" + table.RelationshipID)
		builder.add("rid:" + table.RelationshipID)
	}
	if strings.TrimSpace(table.PartURI) != "" {
		builder.add("part:" + table.PartURI)
	}

	table.PrimarySelector = primary
	table.Selectors = builder.values
	return table
}

// WithChartSelectors returns a chart reference with agent-friendly selectors populated.
func WithChartSelectors(chart ChartRef) ChartRef {
	primary := ""
	if chart.Number > 0 {
		primary = fmt.Sprintf("chart:%d", chart.Number)
	} else if strings.TrimSpace(chart.Name) != "" {
		primary = "chart:" + chart.Name
	}

	builder := selectorBuilder{}
	builder.add(primary)
	if chart.Number > 0 {
		builder.add(fmt.Sprintf("chart:%d", chart.Number))
		builder.add(fmt.Sprintf("#%d", chart.Number))
	}
	if strings.TrimSpace(chart.Name) != "" {
		builder.add("chart:" + chart.Name)
		builder.add("name:" + chart.Name)
		builder.add("~" + chart.Name)
		builder.add(chart.Name)
	}
	if strings.TrimSpace(chart.RelationshipID) != "" {
		builder.add("rId:" + chart.RelationshipID)
		builder.add("rid:" + chart.RelationshipID)
	}
	if strings.TrimSpace(chart.DrawingRelationshipID) != "" {
		builder.add("drawingRid:" + chart.DrawingRelationshipID)
	}
	if strings.TrimSpace(chart.PartURI) != "" {
		builder.add("part:" + chart.PartURI)
	}

	chart.PrimarySelector = primary
	chart.Selectors = builder.values
	return chart
}

// WithDefinedNameSelectors returns a defined name with agent-friendly selectors populated.
func WithDefinedNameSelectors(name DefinedName) DefinedName {
	primary := ""
	if name.Scope == "workbook" && strings.TrimSpace(name.Name) != "" {
		primary = "name:" + name.Name
	} else if name.Scope == "sheet" && name.SheetNumber > 0 && strings.TrimSpace(name.Name) != "" {
		primary = fmt.Sprintf("sheet:%d/name:%s", name.SheetNumber, name.Name)
	} else if name.Number > 0 {
		primary = fmt.Sprintf("definedName:%d", name.Number)
	}

	builder := selectorBuilder{}
	builder.add(primary)
	if name.Number > 0 {
		builder.add(fmt.Sprintf("definedName:%d", name.Number))
		builder.add(fmt.Sprintf("#%d", name.Number))
	}
	if strings.TrimSpace(name.Name) != "" {
		builder.add("name:" + name.Name)
		builder.add("~" + name.Name)
		builder.add(name.Name)
	}
	if name.Scope == "workbook" && strings.TrimSpace(name.Name) != "" {
		builder.add("scope:workbook/name:" + name.Name)
		builder.add("workbook:" + name.Name)
	}
	if name.Scope == "sheet" && strings.TrimSpace(name.Name) != "" {
		if name.SheetNumber > 0 {
			builder.add(fmt.Sprintf("scope:sheet:%d/name:%s", name.SheetNumber, name.Name))
			builder.add(fmt.Sprintf("sheet:%d/name:%s", name.SheetNumber, name.Name))
		}
		if strings.TrimSpace(name.SheetName) != "" {
			builder.add("scope:sheet:" + name.SheetName + "/name:" + name.Name)
			builder.add("sheet:" + name.SheetName + "/name:" + name.Name)
		}
	}

	name.PrimarySelector = primary
	name.Selectors = builder.values
	return name
}

// WithPivotSelectors returns a pivot reference with agent-friendly selectors populated.
func WithPivotSelectors(pivot PivotRef) PivotRef {
	primary := ""
	if pivot.Number > 0 {
		primary = fmt.Sprintf("pivot:%d", pivot.Number)
	} else if strings.TrimSpace(pivot.Name) != "" {
		primary = "pivot:" + pivot.Name
	}

	builder := selectorBuilder{}
	builder.add(primary)
	if pivot.Number > 0 {
		builder.add(fmt.Sprintf("pivot:%d", pivot.Number))
		builder.add(fmt.Sprintf("#%d", pivot.Number))
	}
	if strings.TrimSpace(pivot.Name) != "" {
		builder.add("pivot:" + pivot.Name)
		builder.add("name:" + pivot.Name)
		builder.add("~" + pivot.Name)
		builder.add(pivot.Name)
	}
	if pivot.CacheID > 0 {
		builder.add(fmt.Sprintf("cacheId:%d", pivot.CacheID))
	}
	if strings.TrimSpace(pivot.RelationshipID) != "" {
		builder.add("rId:" + pivot.RelationshipID)
		builder.add("rid:" + pivot.RelationshipID)
	}
	if strings.TrimSpace(pivot.PartURI) != "" {
		builder.add("part:" + pivot.PartURI)
	}

	pivot.PrimarySelector = primary
	pivot.Selectors = builder.values
	return pivot
}

// SelectorMatches reports whether selector exactly matches one of the published selectors.
func SelectorMatches(selectors []string, selector string) bool {
	selector = strings.TrimSpace(selector)
	if selector == "" {
		return false
	}
	for _, candidate := range selectors {
		if candidate == selector {
			return true
		}
	}
	return false
}

type selectorBuilder struct {
	values []string
}

func (b *selectorBuilder) add(value string) {
	value = strings.TrimSpace(value)
	if value == "" {
		return
	}
	for _, existing := range b.values {
		if existing == value {
			return
		}
	}
	b.values = append(b.values, value)
}
