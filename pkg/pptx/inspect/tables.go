package inspect

import (
	"strconv"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// ParseTable parses a:tbl element with full enrichment
// Returns a TableInfo with rows, cells, columns, fills, borders, and style info
func ParseTable(tbl *etree.Element) *model.TableInfo {
	if tbl == nil {
		return nil
	}

	info := &model.TableInfo{
		Cells:      [][]string{},
		RowDefs:    []*model.TableRow{},
		ColumnDefs: []*model.TableCol{},
		CellDefs:   [][]model.CellInfo{},
	}

	// Parse table grid for column widths
	tblGrid := tbl.FindElement("tblGrid")
	if tblGrid != nil {
		for _, gridCol := range tblGrid.FindElements("gridCol") {
			width := int64(0)
			if wStr := gridCol.SelectAttrValue("w", ""); wStr != "" {
				if w, err := strconv.ParseInt(wStr, 10, 64); err == nil {
					width = w
				}
			}
			info.ColumnDefs = append(info.ColumnDefs, &model.TableCol{
				Width: width,
			})
		}
	}

	// If no column defs from grid, count from first row
	if len(info.ColumnDefs) == 0 {
		firstRow := tbl.FindElement("tr")
		if firstRow != nil {
			for range firstRow.FindElements("tc") {
				info.ColumnDefs = append(info.ColumnDefs, &model.TableCol{})
			}
		}
	}
	info.Cols = len(info.ColumnDefs)

	// Parse table rows
	for _, tr := range tbl.FindElements("tr") {
		info.Rows++

		rowDef := &model.TableRow{
			Cells: []*model.CellInfo{},
		}

		// Try to get row height
		if hStr := tr.SelectAttrValue("h", ""); hStr != "" {
			if h, err := strconv.ParseInt(hStr, 10, 64); err == nil {
				rowDef.Height = h
			}
		}

		var row []string
		var cellRow []model.CellInfo

		for _, tc := range tr.FindElements("tc") {
			cellInfo := parseCellInfo(tc)
			row = append(row, cellInfo.Text)
			cellRow = append(cellRow, *cellInfo)
			rowDef.Cells = append(rowDef.Cells, cellInfo)
		}

		info.Cells = append(info.Cells, row)
		info.CellDefs = append(info.CellDefs, cellRow)
		info.RowDefs = append(info.RowDefs, rowDef)
	}

	return info
}

// parseCellInfo extracts all details from a:tc element
func parseCellInfo(tc *etree.Element) *model.CellInfo {
	cellInfo := &model.CellInfo{}

	// Parse grid and row spans from a:tc attributes (horizontal and vertical merges)
	// gridSpan is on the primary cell, hMerge/vMerge are on continuation cells
	if gridSpanStr := tc.SelectAttrValue("gridSpan", ""); gridSpanStr != "" {
		if gridSpan, err := strconv.Atoi(gridSpanStr); err == nil && gridSpan > 0 {
			cellInfo.GridSpan = gridSpan
		}
	} else {
		cellInfo.GridSpan = 1
	}

	if rowSpanStr := tc.SelectAttrValue("rowSpan", ""); rowSpanStr != "" {
		if rowSpan, err := strconv.Atoi(rowSpanStr); err == nil && rowSpan > 0 {
			cellInfo.RowSpan = rowSpan
		}
	} else {
		cellInfo.RowSpan = 1
	}

	// Parse cell properties (a:tcPr)
	tcPr := tc.FindElement("tcPr")
	if tcPr != nil {

		// Cell fill
		cellInfo.Fill = parseCellFill(tcPr)

		// Cell borders
		cellInfo.Border = parseCellBorder(tcPr)

		// Text alignment
		if anchorStr := tcPr.SelectAttrValue("anchor", ""); anchorStr != "" {
			cellInfo.VertAlign = mapVertAlign(anchorStr)
		}
	}

	// Parse text body content and formatting
	txBody := tc.FindElement("txBody")
	if txBody != nil {
		// Extract all text
		cellInfo.Text = extractCellText(txBody)

		// Get first paragraph for alignment and style (apply to first element if exists)
		if firstP := txBody.FindElement("p"); firstP != nil {
			// Paragraph properties
			if pPr := firstP.FindElement("pPr"); pPr != nil {
				if algn := pPr.SelectAttrValue("algn", ""); algn != "" {
					cellInfo.TextAlign = mapHorizAlign(algn)
				}
			}

			// Get first run for formatting
			if firstR := firstP.FindElement("r"); firstR != nil {
				if rPr := firstR.FindElement("rPr"); rPr != nil {
					parseCellFormatting(rPr, cellInfo)
				}
			}
		}
	}

	return cellInfo
}

// parseCellFill extracts fill information from tcPr
func parseCellFill(tcPr *etree.Element) *model.CellFill {
	// a:solidFill or a:noFill
	if solidFill := tcPr.FindElement("solidFill"); solidFill != nil {
		return &model.CellFill{
			Type:  "solid",
			Color: extractColor(solidFill),
		}
	}

	if noFill := tcPr.FindElement("noFill"); noFill != nil {
		return &model.CellFill{
			Type: "none",
		}
	}

	return nil
}

// parseCellBorder extracts border information from tcPr
func parseCellBorder(tcPr *etree.Element) *model.CellBorder {
	// Look for a:lnL, a:lnR, a:lnT, a:lnB
	border := &model.CellBorder{}

	hasAny := false
	if lnL := tcPr.FindElement("lnL"); lnL != nil {
		border.Left = parseBorderLine(lnL)
		hasAny = true
	}

	if lnR := tcPr.FindElement("lnR"); lnR != nil {
		border.Right = parseBorderLine(lnR)
		hasAny = true
	}

	if lnT := tcPr.FindElement("lnT"); lnT != nil {
		border.Top = parseBorderLine(lnT)
		hasAny = true
	}

	if lnB := tcPr.FindElement("lnB"); lnB != nil {
		border.Bottom = parseBorderLine(lnB)
		hasAny = true
	}

	if !hasAny {
		return nil
	}

	return border
}

// parseBorderLine extracts details from a:ln* element
func parseBorderLine(ln *etree.Element) *model.BorderLine {
	if ln == nil {
		return nil
	}

	line := &model.BorderLine{}

	// Get width from w attribute
	if wStr := ln.SelectAttrValue("w", ""); wStr != "" {
		if w, err := strconv.ParseInt(wStr, 10, 64); err == nil {
			line.Width = w
		}
	}

	// Get line style from prstDash or other style elements
	// For now, default to solid
	line.Style = "solid"

	// Get color
	line.Color = extractColor(ln)

	return line
}

// extractColor extracts color from various color elements (solidFill, schemeClr, srgbClr, etc.)
func extractColor(parent *etree.Element) string {
	// Try srgbClr first
	if srgbClr := parent.FindElement("srgbClr"); srgbClr != nil {
		return srgbClr.SelectAttrValue("val", "")
	}

	// Try schemeClr (theme color)
	if schemeClr := parent.FindElement("schemeClr"); schemeClr != nil {
		return schemeClr.SelectAttrValue("val", "")
	}

	// Try sysClr (system color)
	if sysClr := parent.FindElement("sysClr"); sysClr != nil {
		return sysClr.SelectAttrValue("lastClr", "")
	}

	return ""
}

// extractCellText extracts all text from a txBody
func extractCellText(txBody *etree.Element) string {
	var text string

	for _, p := range txBody.FindElements("p") {
		for _, r := range p.FindElements("r") {
			t := r.FindElement("t")
			if t != nil {
				text += t.Text()
			}

			// Handle breaks
			br := r.FindElement("br")
			if br != nil {
				text += "\n"
			}
		}

		// Add paragraph break (except after last paragraph)
		if p != txBody.FindElements("p")[len(txBody.FindElements("p"))-1] {
			text += "\n"
		}
	}

	return text
}

// parseCellFormatting extracts text formatting from rPr
func parseCellFormatting(rPr *etree.Element, cellInfo *model.CellInfo) {
	// Bold
	if bStr := rPr.SelectAttrValue("b", ""); bStr == "1" || bStr == "true" {
		cellInfo.Bold = true
	}

	// Italic
	if iStr := rPr.SelectAttrValue("i", ""); iStr == "1" || iStr == "true" {
		cellInfo.Italic = true
	}

	// Font size (in 100ths of a point)
	if szStr := rPr.SelectAttrValue("sz", ""); szStr != "" {
		if sz, err := strconv.Atoi(szStr); err == nil {
			cellInfo.FontSize = sz / 100 // Convert to points
		}
	}

	// Font color
	cellInfo.FontColor = extractColor(rPr)
}

// mapHorizAlign converts alignment string to short form
func mapHorizAlign(algn string) string {
	switch algn {
	case "l":
		return "l"
	case "r":
		return "r"
	case "ctr":
		return "ctr"
	case "just":
		return "j"
	}
	return algn
}

// mapVertAlign converts vertical anchor to short form
func mapVertAlign(anchor string) string {
	switch anchor {
	case "t":
		return "t"
	case "b":
		return "b"
	case "ctr":
		return "ctr"
	}
	return anchor
}
