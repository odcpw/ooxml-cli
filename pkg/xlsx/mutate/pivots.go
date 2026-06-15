package mutate

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/rangeio"
)

// pivot data-field aggregation -> ST_DataConsolidateFunction subtotal value.
var pivotAggregations = map[string]string{
	"sum": "sum", "count": "count", "average": "average", "avg": "average",
	"max": "max", "min": "min", "product": "product", "countnums": "countNums",
	"stddev": "stdDev", "var": "var",
}

// PivotValueSpec is a value (data) field with its aggregation.
type PivotValueSpec struct {
	Name        string
	Aggregation string // sum, count, average, max, min, product, ...
}

// CreatePivotRequest authors a new PivotTable from a worksheet range/table.
type CreatePivotRequest struct {
	Package      opc.PackageSession
	WorkbookURI  string
	SourceSheet  string
	SourceRange  address.RangeRef
	SourceCells  [][]rangeio.Cell // first row is the header
	TargetSheet  model.SheetRef
	TargetAnchor address.CellRef
	Name         string
	RowFields    []string
	ColFields    []string
	PageFields   []string
	ValueFields  []PivotValueSpec
}

// CreatePivotResult reports the authored pivot.
type CreatePivotResult struct {
	Name           string   `json:"name"`
	CacheDefURI    string   `json:"cacheDefinitionUri"`
	CacheRecordURI string   `json:"cacheRecordsUri"`
	PivotTableURI  string   `json:"pivotTableUri"`
	CacheID        int      `json:"cacheId"`
	Location       string   `json:"location"`
	RowFields      []string `json:"rowFields,omitempty"`
	ColFields      []string `json:"colFields,omitempty"`
	PageFields     []string `json:"pageFields,omitempty"`
	ValueFields    []string `json:"valueFields,omitempty"`
	Warnings       []string `json:"warnings,omitempty"`
}

type pivotFieldModel struct {
	name      string
	numeric   bool
	hasItems  bool
	items     []string
	itemIsNum []bool
	itemIndex map[string]int
	minV      float64
	maxV      float64
}

// CreatePivot builds the pivot cache definition, cache records, and pivot table
// parts, wires relationships and content types, and registers the cache on the
// workbook. refreshOnLoad is set so Excel/LibreOffice rebuild the layout.
func CreatePivot(req *CreatePivotRequest) (*CreatePivotResult, error) {
	if req == nil {
		return nil, fmt.Errorf("create pivot request is nil")
	}
	if len(req.SourceCells) < 2 {
		return nil, fmt.Errorf("pivot source needs a header row and at least one data row")
	}
	if req.TargetSheet.PartURI == "" {
		return nil, fmt.Errorf("target sheet %q has no worksheet part URI", req.TargetSheet.Name)
	}
	headers, nameIndex := pivotHeaders(req.SourceCells)
	if len(req.RowFields) == 0 && len(req.ColFields) == 0 {
		return nil, fmt.Errorf("specify at least one --rows or --cols field")
	}
	if len(req.ValueFields) == 0 {
		return nil, fmt.Errorf("specify at least one --values field")
	}
	axis := map[int]bool{}
	resolve := func(role string, names []string) ([]int, error) {
		var idxs []int
		for _, n := range names {
			i, ok := nameIndex[n]
			if !ok {
				return nil, fmt.Errorf("%s field %q not found in source header (%s)", role, n, strings.Join(headers, ", "))
			}
			idxs = append(idxs, i)
			axis[i] = true
		}
		return idxs, nil
	}
	rowIdx, err := resolve("rows", req.RowFields)
	if err != nil {
		return nil, err
	}
	colIdx, err := resolve("cols", req.ColFields)
	if err != nil {
		return nil, err
	}
	pageIdx, err := resolve("filters", req.PageFields)
	if err != nil {
		return nil, err
	}
	type valueField struct {
		idx     int
		agg     string
		caption string
	}
	var values []valueField
	dataFieldIdx := map[int]bool{}
	var warnings []string
	for _, v := range req.ValueFields {
		i, ok := nameIndex[v.Name]
		if !ok {
			return nil, fmt.Errorf("value field %q not found in source header (%s)", v.Name, strings.Join(headers, ", "))
		}
		agg := strings.ToLower(strings.TrimSpace(v.Aggregation))
		if agg == "" {
			agg = "sum"
		}
		sub, ok := pivotAggregations[agg]
		if !ok {
			return nil, fmt.Errorf("invalid aggregation %q for %q", v.Aggregation, v.Name)
		}
		values = append(values, valueField{idx: i, agg: sub, caption: pivotDataCaption(sub, v.Name)})
		dataFieldIdx[i] = true
	}

	fields := buildPivotFieldModels(req.SourceCells, headers, axis)

	// Allocate parts.
	cacheDefURI := allocateNumberedPart(req.Package, "/xl/pivotCache/pivotCacheDefinition", ".xml")
	cacheRecURI := allocateNumberedPart(req.Package, "/xl/pivotCache/pivotCacheRecords", ".xml")
	pivotURI := allocateNumberedPart(req.Package, "/xl/pivotTables/pivotTable", ".xml")
	cacheID := nextPivotCacheID(req.Package, req.WorkbookURI)

	recordCount := len(req.SourceCells) - 1
	// cacheDefinition -> records relationship
	cacheRecRID := opc.AllocateRelationshipID(nil)
	cacheDefXML := buildCacheDefinitionXML(req, fields, recordCount, cacheRecRID)
	if err := req.Package.AddPart(cacheDefURI, cacheDefXML, namespaces.ContentTypePivotCache, nil); err != nil {
		return nil, fmt.Errorf("failed to add pivot cache definition: %w", err)
	}
	cacheRecXML := buildCacheRecordsXML(req.SourceCells, fields)
	if err := req.Package.AddPart(cacheRecURI, cacheRecXML, namespaces.ContentTypePivotRecords, nil); err != nil {
		return nil, fmt.Errorf("failed to add pivot cache records: %w", err)
	}
	if err := opc.WriteRelationships(req.Package, cacheDefURI, []opc.RelationshipInfo{{
		SourceURI: cacheDefURI, ID: cacheRecRID, Type: namespaces.RelPivotRecords,
		Target: opc.RelationshipTarget(cacheDefURI, cacheRecURI),
	}}); err != nil {
		return nil, fmt.Errorf("failed to write cache definition relationships: %w", err)
	}

	// pivot table part
	location := pivotLocation(req.TargetAnchor, len(rowIdx), len(values))
	pivotName := strings.TrimSpace(req.Name)
	if pivotName == "" {
		pivotName = fmt.Sprintf("PivotTable%d", cacheID)
	}
	dataFields := make([]pivotDataField, len(values))
	for i, v := range values {
		dataFields[i] = pivotDataField{fld: v.idx, subtotal: v.agg, name: v.caption}
	}
	pivotXML := buildPivotTableXML(pivotName, cacheID, location, fields, rowIdx, colIdx, pageIdx, dataFields)
	if err := req.Package.AddPart(pivotURI, pivotXML, namespaces.ContentTypePivotTable, nil); err != nil {
		return nil, fmt.Errorf("failed to add pivot table: %w", err)
	}
	pivotCacheRID := opc.AllocateRelationshipID(nil)
	if err := opc.WriteRelationships(req.Package, pivotURI, []opc.RelationshipInfo{{
		SourceURI: pivotURI, ID: pivotCacheRID, Type: namespaces.RelPivotCache,
		Target: opc.RelationshipTarget(pivotURI, cacheDefURI),
	}}); err != nil {
		return nil, fmt.Errorf("failed to write pivot table relationships: %w", err)
	}
	// target worksheet -> pivot table relationship
	wsRels := req.Package.ListRelationships(req.TargetSheet.PartURI)
	pivotRID := opc.AllocateRelationshipID(wsRels)
	wsRels = append(wsRels, opc.RelationshipInfo{
		SourceURI: req.TargetSheet.PartURI, ID: pivotRID, Type: namespaces.RelPivotTable,
		Target: opc.RelationshipTarget(req.TargetSheet.PartURI, pivotURI),
	})
	if err := opc.WriteRelationships(req.Package, req.TargetSheet.PartURI, wsRels); err != nil {
		return nil, fmt.Errorf("failed to write worksheet relationships: %w", err)
	}

	// workbook pivotCaches + workbook -> cache definition relationship
	if err := registerWorkbookPivotCache(req.Package, req.WorkbookURI, cacheID, cacheDefURI); err != nil {
		return nil, err
	}

	for _, v := range req.ValueFields {
		if i, ok := nameIndex[v.Name]; ok && !fields[i].numeric {
			warnings = append(warnings, fmt.Sprintf("value field %q is not fully numeric; aggregation may be approximate", v.Name))
		}
	}

	return &CreatePivotResult{
		Name: pivotName, CacheDefURI: cacheDefURI, CacheRecordURI: cacheRecURI,
		PivotTableURI: pivotURI, CacheID: cacheID, Location: location,
		RowFields: req.RowFields, ColFields: req.ColFields, PageFields: req.PageFields,
		ValueFields: valueFieldNames(req.ValueFields), Warnings: warnings,
	}, nil
}

type pivotDataField struct {
	fld      int
	subtotal string
	name     string
}

func valueFieldNames(v []PivotValueSpec) []string {
	out := make([]string, len(v))
	for i := range v {
		out[i] = v[i].Name
	}
	return out
}

func pivotDataCaption(subtotal, field string) string {
	label := map[string]string{
		"sum": "Sum", "count": "Count", "average": "Average", "max": "Max",
		"min": "Min", "product": "Product", "countNums": "Count", "stdDev": "StdDev", "var": "Var",
	}[subtotal]
	if label == "" {
		label = "Sum"
	}
	return label + " of " + field
}

func pivotHeaders(cells [][]rangeio.Cell) ([]string, map[string]int) {
	header := cells[0]
	names := make([]string, len(header))
	index := map[string]int{}
	for i, c := range header {
		n := strings.TrimSpace(c.Value)
		if n == "" {
			n = "Field" + strconv.Itoa(i+1)
		}
		names[i] = n
		index[n] = i
	}
	return names, index
}

func buildPivotFieldModels(cells [][]rangeio.Cell, headers []string, axis map[int]bool) []pivotFieldModel {
	fields := make([]pivotFieldModel, len(headers))
	for c := range headers {
		f := pivotFieldModel{name: headers[c], numeric: true, itemIndex: map[string]int{}}
		hasData := false
		firstNum := true
		for r := 1; r < len(cells); r++ {
			if c >= len(cells[r]) {
				continue
			}
			cell := cells[r][c]
			if cell.Null || cell.Value == "" {
				continue
			}
			hasData = true
			if num, err := strconv.ParseFloat(cell.Value, 64); err == nil {
				if firstNum || num < f.minV {
					f.minV = num
				}
				if firstNum || num > f.maxV {
					f.maxV = num
				}
				firstNum = false
			} else {
				f.numeric = false
			}
		}
		if !hasData {
			f.numeric = false
		}
		// Axis fields always carry a shared-item list; non-numeric fields too.
		f.hasItems = axis[c] || !f.numeric
		if f.hasItems {
			for r := 1; r < len(cells); r++ {
				v := ""
				if c < len(cells[r]) && !cells[r][c].Null {
					v = cells[r][c].Value
				}
				if _, ok := f.itemIndex[v]; ok {
					continue
				}
				f.itemIndex[v] = len(f.items)
				f.items = append(f.items, v)
				_, numErr := strconv.ParseFloat(v, 64)
				f.itemIsNum = append(f.itemIsNum, numErr == nil && v != "")
			}
		}
		fields[c] = f
	}
	return fields
}

// pel builds an unprefixed SpreadsheetML element for pivot parts (these parts
// declare the default xmlns, so children carry no prefix).
func pel(local string) *etree.Element { return etree.NewElement(local) }

func buildCacheDefinitionXML(req *CreatePivotRequest, fields []pivotFieldModel, recordCount int, recRID string) []byte {
	doc := etree.NewDocument()
	root := pel("pivotCacheDefinition")
	root.CreateAttr("xmlns", namespaces.NsSpreadsheetML)
	root.CreateAttr("xmlns:r", namespaces.NsR)
	root.CreateAttr("r:id", recRID)
	root.CreateAttr("refreshOnLoad", "1")
	root.CreateAttr("refreshedBy", "ooxml-cli")
	root.CreateAttr("createdVersion", "6")
	root.CreateAttr("refreshedVersion", "6")
	root.CreateAttr("minRefreshableVersion", "3")
	root.CreateAttr("recordCount", strconv.Itoa(recordCount))
	doc.SetRoot(root)

	src := pel("cacheSource")
	src.CreateAttr("type", "worksheet")
	ws := pel("worksheetSource")
	ws.CreateAttr("ref", req.SourceRange.String())
	ws.CreateAttr("sheet", req.SourceSheet)
	src.AddChild(ws)
	root.AddChild(src)

	cf := pel("cacheFields")
	cf.CreateAttr("count", strconv.Itoa(len(fields)))
	for _, f := range fields {
		field := pel("cacheField")
		field.CreateAttr("name", f.name)
		field.CreateAttr("numFmtId", "0")
		shared := pel("sharedItems")
		if f.hasItems {
			hasString, hasNumber, hasBlank := false, false, false
			var children []*etree.Element
			for i, it := range f.items {
				if it == "" {
					children = append(children, pel("m"))
					hasBlank = true
					continue
				}
				if f.itemIsNum[i] {
					n := pel("n")
					n.CreateAttr("v", it)
					children = append(children, n)
					hasNumber = true
				} else {
					s := pel("s")
					s.CreateAttr("v", it)
					children = append(children, s)
					hasString = true
				}
			}
			// containsString/containsNumber must reflect the actual item types.
			// Excel writes containsString explicitly (default true) so emit both.
			boolAttr := func(v bool) string {
				if v {
					return "1"
				}
				return "0"
			}
			shared.CreateAttr("containsSemiMixedTypes", boolAttr(hasString || hasBlank))
			if hasBlank {
				shared.CreateAttr("containsBlank", "1")
			}
			shared.CreateAttr("containsString", boolAttr(hasString))
			if hasNumber {
				shared.CreateAttr("containsNumber", "1")
			}
			shared.CreateAttr("count", strconv.Itoa(len(f.items)))
			for _, child := range children {
				shared.AddChild(child)
			}
		} else {
			// Numeric value field: summary only, records carry the numbers.
			shared.CreateAttr("containsString", "0")
			shared.CreateAttr("containsNumber", "1")
			shared.CreateAttr("minValue", trimFloat(f.minV))
			shared.CreateAttr("maxValue", trimFloat(f.maxV))
		}
		field.AddChild(shared)
		cf.AddChild(field)
	}
	root.AddChild(cf)
	doc.IndentTabs()
	data, _ := doc.WriteToBytes()
	return data
}

func buildCacheRecordsXML(cells [][]rangeio.Cell, fields []pivotFieldModel) []byte {
	doc := etree.NewDocument()
	root := pel("pivotCacheRecords")
	root.CreateAttr("xmlns", namespaces.NsSpreadsheetML)
	root.CreateAttr("xmlns:r", namespaces.NsR)
	root.CreateAttr("count", strconv.Itoa(len(cells)-1))
	doc.SetRoot(root)
	for r := 1; r < len(cells); r++ {
		rec := pel("r")
		for c, f := range fields {
			v := ""
			null := true
			if c < len(cells[r]) {
				cell := cells[r][c]
				null = cell.Null || cell.Value == ""
				v = cell.Value
			}
			if f.hasItems {
				x := pel("x")
				idx := 0
				if i, ok := f.itemIndex[v]; ok {
					idx = i
				}
				x.CreateAttr("v", strconv.Itoa(idx))
				rec.AddChild(x)
			} else if null {
				rec.AddChild(pel("m"))
			} else {
				n := pel("n")
				n.CreateAttr("v", v)
				rec.AddChild(n)
			}
		}
		root.AddChild(rec)
	}
	doc.IndentTabs()
	data, _ := doc.WriteToBytes()
	return data
}

func buildPivotTableXML(name string, cacheID int, location string, fields []pivotFieldModel, rowIdx, colIdx, pageIdx []int, values []pivotDataField) []byte {
	doc := etree.NewDocument()
	root := pel("pivotTableDefinition")
	root.CreateAttr("xmlns", namespaces.NsSpreadsheetML)
	root.CreateAttr("name", name)
	root.CreateAttr("cacheId", strconv.Itoa(cacheID))
	root.CreateAttr("applyNumberFormats", "0")
	root.CreateAttr("applyBorderFormats", "0")
	root.CreateAttr("applyFontFormats", "0")
	root.CreateAttr("applyPatternFormats", "0")
	root.CreateAttr("applyAlignmentFormats", "0")
	root.CreateAttr("applyWidthHeightFormats", "1")
	root.CreateAttr("dataCaption", "Values")
	root.CreateAttr("updatedVersion", "6")
	root.CreateAttr("minRefreshableVersion", "3")
	root.CreateAttr("useAutoFormatting", "1")
	root.CreateAttr("itemPrintTitles", "1")
	root.CreateAttr("createdVersion", "6")
	root.CreateAttr("indent", "0")
	root.CreateAttr("outline", "1")
	root.CreateAttr("outlineData", "1")
	root.CreateAttr("multipleFieldFilters", "0")
	doc.SetRoot(root)

	loc := pel("location")
	loc.CreateAttr("ref", location)
	loc.CreateAttr("firstHeaderRow", "1")
	loc.CreateAttr("firstDataRow", "2")
	loc.CreateAttr("firstDataCol", "1")
	if len(pageIdx) > 0 {
		loc.CreateAttr("rowPageCount", "1")
		loc.CreateAttr("colPageCount", strconv.Itoa(len(pageIdx)))
	}
	root.AddChild(loc)

	role := map[int]string{}
	for _, i := range rowIdx {
		role[i] = "axisRow"
	}
	for _, i := range colIdx {
		role[i] = "axisCol"
	}
	for _, i := range pageIdx {
		role[i] = "axisPage"
	}
	dataField := map[int]bool{}
	for _, v := range values {
		dataField[v.fld] = true
	}

	pf := pel("pivotFields")
	pf.CreateAttr("count", strconv.Itoa(len(fields)))
	for i, f := range fields {
		field := pel("pivotField")
		if ax, ok := role[i]; ok {
			field.CreateAttr("axis", ax)
			field.CreateAttr("showAll", "0")
			items := pel("items")
			items.CreateAttr("count", strconv.Itoa(len(f.items)+1))
			for j := range f.items {
				it := pel("item")
				it.CreateAttr("x", strconv.Itoa(j))
				items.AddChild(it)
			}
			def := pel("item")
			def.CreateAttr("t", "default")
			items.AddChild(def)
			field.AddChild(items)
		} else if dataField[i] {
			field.CreateAttr("dataField", "1")
			field.CreateAttr("showAll", "0")
		} else {
			field.CreateAttr("showAll", "0")
		}
		pf.AddChild(field)
	}
	root.AddChild(pf)

	if len(rowIdx) > 0 {
		rf := pel("rowFields")
		rf.CreateAttr("count", strconv.Itoa(len(rowIdx)))
		for _, i := range rowIdx {
			fe := pel("field")
			fe.CreateAttr("x", strconv.Itoa(i))
			rf.AddChild(fe)
		}
		root.AddChild(rf)
	}
	if len(colIdx) > 0 {
		cf := pel("colFields")
		cf.CreateAttr("count", strconv.Itoa(len(colIdx)))
		for _, i := range colIdx {
			fe := pel("field")
			fe.CreateAttr("x", strconv.Itoa(i))
			cf.AddChild(fe)
		}
		root.AddChild(cf)
	}
	if len(pageIdx) > 0 {
		pgs := pel("pageFields")
		pgs.CreateAttr("count", strconv.Itoa(len(pageIdx)))
		for _, i := range pageIdx {
			pg := pel("pageField")
			pg.CreateAttr("fld", strconv.Itoa(i))
			pg.CreateAttr("hier", "-1")
			pgs.AddChild(pg)
		}
		root.AddChild(pgs)
	}

	df := pel("dataFields")
	df.CreateAttr("count", strconv.Itoa(len(values)))
	for _, v := range values {
		d := pel("dataField")
		d.CreateAttr("name", v.name)
		d.CreateAttr("fld", strconv.Itoa(v.fld))
		if v.subtotal != "sum" {
			d.CreateAttr("subtotal", v.subtotal)
		}
		d.CreateAttr("baseField", "0")
		d.CreateAttr("baseItem", "0")
		df.AddChild(d)
	}
	root.AddChild(df)

	style := pel("pivotTableStyleInfo")
	style.CreateAttr("name", "PivotStyleLight16")
	style.CreateAttr("showRowHeaders", "1")
	style.CreateAttr("showColHeaders", "1")
	style.CreateAttr("showRowStripes", "0")
	style.CreateAttr("showColStripes", "0")
	style.CreateAttr("showLastColumn", "1")
	root.AddChild(style)

	doc.IndentTabs()
	data, _ := doc.WriteToBytes()
	return data
}

func pivotLocation(anchor address.CellRef, rowFieldCount, valueCount int) string {
	cols := rowFieldCount
	if cols < 1 {
		cols = 1
	}
	cols += valueCount
	if cols < 1 {
		cols = 1
	}
	endCol := anchor.Column + cols - 1
	endRow := anchor.Row + 4
	return rangeRef(anchor.Column, anchor.Row, endCol, endRow)
}

func nextPivotCacheID(session opc.PackageSession, workbookURI string) int {
	doc, err := session.ReadXMLPart(workbookURI)
	if err != nil {
		return 1
	}
	root := doc.Root()
	if root == nil {
		return 1
	}
	caches := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "pivotCaches")
	if caches == nil {
		return 1
	}
	maxID := 0
	for _, pc := range namespaces.FindChildren(caches, namespaces.NsSpreadsheetML, "pivotCache") {
		if id, err := strconv.Atoi(pc.SelectAttrValue("cacheId", "")); err == nil && id > maxID {
			maxID = id
		}
	}
	return maxID + 1
}

func registerWorkbookPivotCache(session opc.PackageSession, workbookURI string, cacheID int, cacheDefURI string) error {
	doc, err := session.ReadXMLPart(workbookURI)
	if err != nil {
		return fmt.Errorf("failed to read workbook: %w", err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "workbook") {
		return fmt.Errorf("workbook root element not found")
	}
	rels := session.ListRelationships(workbookURI)
	rid := opc.AllocateRelationshipID(rels)
	rels = append(rels, opc.RelationshipInfo{
		SourceURI: workbookURI, ID: rid, Type: namespaces.RelPivotCache,
		Target: opc.RelationshipTarget(workbookURI, cacheDefURI),
	})
	if err := opc.WriteRelationships(session, workbookURI, rels); err != nil {
		return fmt.Errorf("failed to write workbook relationships: %w", err)
	}
	prefix := root.Space
	caches := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "pivotCaches")
	if caches == nil {
		caches = newElement(prefix, "pivotCaches")
		insertWorkbookChild(root, caches, "pivotCaches")
	}
	pc := newElement(prefix, "pivotCache")
	pc.CreateAttr("cacheId", strconv.Itoa(cacheID))
	pc.CreateAttr("r:id", rid)
	caches.AddChild(pc)
	ensurePivotRelationshipsNamespace(root)
	if err := session.ReplaceXMLPart(workbookURI, doc); err != nil {
		return fmt.Errorf("failed to replace workbook: %w", err)
	}
	return nil
}

func ensurePivotRelationshipsNamespace(root *etree.Element) {
	if root.SelectAttr("xmlns:r") == nil {
		root.CreateAttr("xmlns:r", namespaces.NsR)
	}
}

func trimFloat(v float64) string {
	return strconv.FormatFloat(v, 'f', -1, 64)
}
