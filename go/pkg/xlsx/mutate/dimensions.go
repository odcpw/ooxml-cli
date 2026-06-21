package mutate

import (
	"fmt"
	"sort"
	"strconv"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

// Excel's practical limits for column width (characters) and row height (points).
const (
	maxColumnWidth   = 255.0
	maxRowHeight     = 409.0
	defaultColWidth  = 8.43
	defaultRowHeight = 15.0
)

// ColumnWidthInfo describes the resolved width of a single column.
type ColumnWidthInfo struct {
	Width    float64 `json:"width"`
	Explicit bool    `json:"explicit"`
	Custom   bool    `json:"custom"`
	Hidden   bool    `json:"hidden"`
}

// RowHeightInfo describes the resolved height of a single row.
type RowHeightInfo struct {
	Height   float64 `json:"height"`
	Explicit bool    `json:"explicit"`
	Custom   bool    `json:"custom"`
	Hidden   bool    `json:"hidden"`
}

// ColumnWidthRequest sets a uniform width across a span of columns.
type ColumnWidthRequest struct {
	Package   opc.PackageSession
	SheetRef  model.SheetRef
	MinColumn int
	MaxColumn int
	Width     float64
}

// ColumnWidthResult reports the outcome of a column-width mutation.
type ColumnWidthResult struct {
	MinColumn int     `json:"minColumn"`
	MaxColumn int     `json:"maxColumn"`
	Columns   int     `json:"columns"`
	Width     float64 `json:"width"`
}

// RowHeightRequest sets a uniform height across a span of rows.
type RowHeightRequest struct {
	Package  opc.PackageSession
	SheetRef model.SheetRef
	MinRow   int
	MaxRow   int
	Height   float64
}

// RowHeightResult reports the outcome of a row-height mutation.
type RowHeightResult struct {
	MinRow  int     `json:"minRow"`
	MaxRow  int     `json:"maxRow"`
	Rows    int     `json:"rows"`
	Height  float64 `json:"height"`
	Created int     `json:"created"`
}

func readWorksheetRoot(session opc.PackageSession, sheet model.SheetRef) (*etree.Document, *etree.Element, error) {
	if session == nil {
		return nil, nil, fmt.Errorf("package session is nil")
	}
	if sheet.PartURI == "" {
		return nil, nil, fmt.Errorf("sheet %q has no worksheet part URI", sheet.Name)
	}
	doc, err := session.ReadXMLPart(sheet.PartURI)
	if err != nil {
		return nil, nil, fmt.Errorf("failed to read worksheet %s: %w", sheet.PartURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "worksheet") {
		return nil, nil, fmt.Errorf("worksheet part %s root element not found", sheet.PartURI)
	}
	return doc, root, nil
}

// DefaultColumnWidth returns the worksheet's default column width.
func DefaultColumnWidth(root *etree.Element) float64 {
	if fmtPr := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheetFormatPr"); fmtPr != nil {
		if v := fmtPr.SelectAttrValue("defaultColWidth", ""); v != "" {
			if width, err := strconv.ParseFloat(v, 64); err == nil {
				return width
			}
		}
	}
	return defaultColWidth
}

// DefaultRowHeight returns the worksheet's default row height.
func DefaultRowHeight(root *etree.Element) float64 {
	if fmtPr := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheetFormatPr"); fmtPr != nil {
		if v := fmtPr.SelectAttrValue("defaultRowHeight", ""); v != "" {
			if height, err := strconv.ParseFloat(v, 64); err == nil {
				return height
			}
		}
	}
	return defaultRowHeight
}

// ReadColumnWidths returns resolved widths for columns in [minCol, maxCol].
func ReadColumnWidths(session opc.PackageSession, sheet model.SheetRef, minCol, maxCol int) (map[int]ColumnWidthInfo, float64, error) {
	_, root, err := readWorksheetRoot(session, sheet)
	if err != nil {
		return nil, 0, err
	}
	fallback := DefaultColumnWidth(root)
	out := map[int]ColumnWidthInfo{}
	for col := minCol; col <= maxCol; col++ {
		out[col] = ColumnWidthInfo{Width: fallback}
	}
	cols := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "cols")
	if cols == nil {
		return out, fallback, nil
	}
	for _, col := range namespaces.FindChildren(cols, namespaces.NsSpreadsheetML, "col") {
		spanMin, spanMax, ok := colSpanBounds(col)
		if !ok {
			continue
		}
		widthText := col.SelectAttrValue("width", "")
		custom := col.SelectAttrValue("customWidth", "") == "1"
		hidden := col.SelectAttrValue("hidden", "") == "1"
		for c := spanMin; c <= spanMax; c++ {
			if c < minCol || c > maxCol {
				continue
			}
			info := ColumnWidthInfo{Width: fallback, Custom: custom, Hidden: hidden}
			if widthText != "" {
				if width, err := strconv.ParseFloat(widthText, 64); err == nil {
					info.Width = width
					info.Explicit = true
				}
			}
			out[c] = info
		}
	}
	return out, fallback, nil
}

// ReadRowHeights returns resolved heights for rows in [minRow, maxRow].
func ReadRowHeights(session opc.PackageSession, sheet model.SheetRef, minRow, maxRow int) (map[int]RowHeightInfo, float64, error) {
	_, root, err := readWorksheetRoot(session, sheet)
	if err != nil {
		return nil, 0, err
	}
	fallback := DefaultRowHeight(root)
	out := map[int]RowHeightInfo{}
	for r := minRow; r <= maxRow; r++ {
		out[r] = RowHeightInfo{Height: fallback}
	}
	sheetData := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheetData")
	if sheetData == nil {
		return out, fallback, nil
	}
	for _, row := range namespaces.FindChildren(sheetData, namespaces.NsSpreadsheetML, "row") {
		num, ok := rowNumber(row)
		if !ok || num < minRow || num > maxRow {
			continue
		}
		info := RowHeightInfo{
			Height: fallback,
			Custom: row.SelectAttrValue("customHeight", "") == "1",
			Hidden: row.SelectAttrValue("hidden", "") == "1",
		}
		if htText := row.SelectAttrValue("ht", ""); htText != "" {
			if height, err := strconv.ParseFloat(htText, 64); err == nil {
				info.Height = height
				info.Explicit = true
			}
		}
		out[num] = info
	}
	return out, fallback, nil
}

// SetColumnWidths applies a uniform width to columns [MinColumn, MaxColumn].
func SetColumnWidths(req *ColumnWidthRequest) (*ColumnWidthResult, error) {
	if req == nil {
		return nil, fmt.Errorf("column width request is nil")
	}
	if req.MinColumn < 1 || req.MaxColumn < req.MinColumn {
		return nil, fmt.Errorf("invalid column span %d:%d", req.MinColumn, req.MaxColumn)
	}
	if req.Width < 0 || req.Width > maxColumnWidth {
		return nil, fmt.Errorf("width %.4g out of range 0-%g", req.Width, maxColumnWidth)
	}
	doc, root, err := readWorksheetRoot(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	prefix := root.Space
	cols := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "cols")
	if cols == nil {
		cols = newElement(prefix, "cols")
		insertWorksheetChild(root, cols, "cols")
	}

	// Collect existing spans, then rebuild with the target span carved out.
	// Portions outside the target span keep all their attributes; portions
	// inside it keep the covering span's attributes (hidden, style, outlineLevel,
	// collapsed, bestFit, ...) but adopt the new width.
	existing := namespaces.FindChildren(cols, namespaces.NsSpreadsheetML, "col")
	var rebuilt []*etree.Element
	var malformed []*etree.Element
	for _, col := range existing {
		spanMin, spanMax, ok := colSpanBounds(col)
		if !ok {
			malformed = append(malformed, col.Copy())
			continue
		}
		// Keep portions that fall outside the target span verbatim.
		if spanMin < req.MinColumn {
			left := col.Copy()
			left.CreateAttr("min", strconv.Itoa(spanMin))
			left.CreateAttr("max", strconv.Itoa(min(spanMax, req.MinColumn-1)))
			rebuilt = append(rebuilt, left)
		}
		if spanMax > req.MaxColumn {
			right := col.Copy()
			right.CreateAttr("min", strconv.Itoa(max(spanMin, req.MaxColumn+1)))
			right.CreateAttr("max", strconv.Itoa(spanMax))
			rebuilt = append(rebuilt, right)
		}
	}

	// Build the target region, preserving the attributes of whichever existing
	// span covered each part. Gaps not covered by any span get a fresh element.
	for _, seg := range targetWidthSegments(existing, req.MinColumn, req.MaxColumn) {
		var el *etree.Element
		if seg.base != nil {
			el = seg.base.Copy()
		} else {
			el = newElement(prefix, "col")
		}
		el.CreateAttr("min", strconv.Itoa(seg.min))
		el.CreateAttr("max", strconv.Itoa(seg.max))
		el.CreateAttr("width", formatDimension(req.Width))
		el.CreateAttr("customWidth", "1")
		rebuilt = append(rebuilt, el)
	}

	sort.SliceStable(rebuilt, func(i, j int) bool {
		iMin, _, iok := colSpanBounds(rebuilt[i])
		jMin, _, jok := colSpanBounds(rebuilt[j])
		if !iok || !jok {
			// Keep stable order when bounds are unparseable.
			return false
		}
		return iMin < jMin
	})
	// Append any malformed entries last, preserving their original order.
	rebuilt = append(rebuilt, malformed...)

	for _, child := range cols.ChildElements() {
		cols.RemoveChild(child)
	}
	for _, col := range rebuilt {
		cols.AddChild(col)
	}
	if len(cols.ChildElements()) == 0 {
		root.RemoveChild(cols)
	}

	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return &ColumnWidthResult{
		MinColumn: req.MinColumn,
		MaxColumn: req.MaxColumn,
		Columns:   req.MaxColumn - req.MinColumn + 1,
		Width:     req.Width,
	}, nil
}

// SetRowHeights applies a uniform height to rows [MinRow, MaxRow].
func SetRowHeights(req *RowHeightRequest) (*RowHeightResult, error) {
	if req == nil {
		return nil, fmt.Errorf("row height request is nil")
	}
	if req.MinRow < 1 || req.MaxRow < req.MinRow {
		return nil, fmt.Errorf("invalid row span %d:%d", req.MinRow, req.MaxRow)
	}
	if req.Height < 0 || req.Height > maxRowHeight {
		return nil, fmt.Errorf("height %.4g out of range 0-%g", req.Height, maxRowHeight)
	}
	doc, root, err := readWorksheetRoot(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	prefix := root.Space
	sheetData := ensureSheetData(root, prefix)
	result := &RowHeightResult{
		MinRow: req.MinRow,
		MaxRow: req.MaxRow,
		Rows:   req.MaxRow - req.MinRow + 1,
		Height: req.Height,
	}
	for rowNum := req.MinRow; rowNum <= req.MaxRow; rowNum++ {
		if findRow(sheetData, rowNum) == nil {
			result.Created++
		}
		row := ensureRow(sheetData, prefix, rowNum)
		row.CreateAttr("ht", formatDimension(req.Height))
		row.CreateAttr("customHeight", "1")
	}
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return result, nil
}

// widthSegment is a contiguous part of the target span. base is the existing
// <col> whose attributes should be preserved, or nil for an uncovered gap.
type widthSegment struct {
	min  int
	max  int
	base *etree.Element
}

// targetWidthSegments partitions [tmin, tmax] by which existing <col> covers
// each column, so attributes (hidden, style, outlineLevel, ...) survive the
// width change. Gaps not covered by any span become base-less segments.
func targetWidthSegments(existing []*etree.Element, tmin, tmax int) []widthSegment {
	type span struct {
		min, max int
		el       *etree.Element
	}
	var spans []span
	for _, col := range existing {
		sMin, sMax, ok := colSpanBounds(col)
		if !ok {
			continue
		}
		lo, hi := max(sMin, tmin), min(sMax, tmax)
		if lo > hi {
			continue
		}
		spans = append(spans, span{min: lo, max: hi, el: col})
	}
	sort.SliceStable(spans, func(i, j int) bool { return spans[i].min < spans[j].min })

	var segs []widthSegment
	cursor := tmin
	for _, s := range spans {
		lo := s.min
		if lo < cursor {
			lo = cursor // earlier span already covered this; first-wins on overlap
		}
		if lo > s.max {
			continue
		}
		if lo > cursor {
			segs = append(segs, widthSegment{min: cursor, max: lo - 1}) // uncovered gap
		}
		segs = append(segs, widthSegment{min: lo, max: s.max, base: s.el})
		cursor = s.max + 1
	}
	if cursor <= tmax {
		segs = append(segs, widthSegment{min: cursor, max: tmax})
	}
	return segs
}

func colSpanBounds(col *etree.Element) (int, int, bool) {
	minText := col.SelectAttrValue("min", "")
	maxText := col.SelectAttrValue("max", "")
	if minText == "" || maxText == "" {
		return 0, 0, false
	}
	minCol, err := strconv.Atoi(minText)
	if err != nil {
		return 0, 0, false
	}
	maxCol, err := strconv.Atoi(maxText)
	if err != nil || maxCol < minCol {
		return 0, 0, false
	}
	return minCol, maxCol, true
}

// formatDimension renders a width/height without trailing zeros.
func formatDimension(value float64) string {
	return strconv.FormatFloat(value, 'f', -1, 64)
}
