// Package mutate applies safe, semantic mutations to XLSX worksheet parts.
package mutate

import (
	"fmt"
	"math"
	"sort"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

type CellValueType string

const (
	CellValueString  CellValueType = "string"
	CellValueNumber  CellValueType = "number"
	CellValueBool    CellValueType = "bool"
	CellValueFormula CellValueType = "formula"
	CellValueAuto    CellValueType = "auto"
)

type SetCellRequest struct {
	Package     opc.PackageSession
	WorkbookURI string
	SheetRef    model.SheetRef
	Cell        string
	Value       string
	Type        CellValueType
}

type SetCellResult struct {
	Ref           string        `json:"ref"`
	Type          CellValueType `json:"type"`
	Value         string        `json:"value"`
	PreviousType  string        `json:"previousType,omitempty"`
	PreviousValue string        `json:"previousValue,omitempty"`
	Created       bool          `json:"created"`
}

type CellAssignment struct {
	Ref   string        `json:"ref"`
	Type  CellValueType `json:"type,omitempty"`
	Value string        `json:"value,omitempty"`
}

type SetCellsRequest struct {
	Package     opc.PackageSession
	WorkbookURI string
	SheetRef    model.SheetRef
	Cells       []CellAssignment
}

type SetCellsResult struct {
	Updated      int             `json:"updated"`
	Created      int             `json:"created"`
	FormulaCount int             `json:"formulaCount"`
	Range        string          `json:"range,omitempty"`
	Cells        []SetCellResult `json:"cells,omitempty"`
}

type ClearCellsRequest struct {
	Package     opc.PackageSession
	WorkbookURI string
	SheetRef    model.SheetRef
	Range       address.RangeRef
}

type ClearCellsResult struct {
	Cleared int      `json:"cleared"`
	Refs    []string `json:"refs"`
}

func SetCell(req *SetCellRequest) (*SetCellResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set cell request is nil")
	}
	result, err := SetCells(&SetCellsRequest{
		Package:     req.Package,
		WorkbookURI: req.WorkbookURI,
		SheetRef:    req.SheetRef,
		Cells: []CellAssignment{{
			Ref:   req.Cell,
			Type:  req.Type,
			Value: req.Value,
		}},
	})
	if err != nil {
		return nil, err
	}
	if len(result.Cells) != 1 {
		return nil, fmt.Errorf("set cell produced %d results, want 1", len(result.Cells))
	}
	return &result.Cells[0], nil
}

func SetCells(req *SetCellsRequest) (*SetCellsResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set cells request is nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if req.SheetRef.PartURI == "" {
		return nil, fmt.Errorf("sheet %q has no worksheet part URI", req.SheetRef.Name)
	}

	writes, err := normalizeAssignments(req.Cells)
	if err != nil {
		return nil, err
	}

	doc, err := req.Package.ReadXMLPart(req.SheetRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "worksheet") {
		return nil, fmt.Errorf("worksheet part %s root element not found", req.SheetRef.PartURI)
	}

	prefix := root.Space
	sheetData := ensureSheetData(root, prefix)
	result := &SetCellsResult{Cells: make([]SetCellResult, 0, len(writes))}
	formulaSeen := false
	formulaInvalidated := false
	for _, write := range writes {
		cellResult := applyCellWrite(sheetData, prefix, write)
		result.Updated++
		if cellResult.Created {
			result.Created++
		}
		if cellResult.PreviousType == "formula" {
			formulaInvalidated = true
		}
		if write.Type == CellValueFormula {
			result.FormulaCount++
			formulaSeen = true
		}
		result.Cells = append(result.Cells, cellResult)
	}
	updateDimension(root, prefix)
	result.Range = writtenRange(result.Cells)

	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	if formulaInvalidated {
		if err := invalidateCalcChainForFullRecalc(req.Package, req.WorkbookURI); err != nil {
			return nil, err
		}
	} else if formulaSeen {
		if req.WorkbookURI == "" {
			return nil, fmt.Errorf("workbook URI is required when setting formulas")
		}
		if err := EnsureFullCalcOnLoad(req.Package, req.WorkbookURI); err != nil {
			return nil, err
		}
	}

	return result, nil
}

type normalizedCellWrite struct {
	Ref     address.CellRef
	RefText string
	Type    CellValueType
	Value   string
}

func normalizeAssignments(cells []CellAssignment) ([]normalizedCellWrite, error) {
	if len(cells) == 0 {
		return nil, fmt.Errorf("cells batch cannot be empty")
	}

	byRef := make(map[string]normalizedCellWrite, len(cells))
	for _, cell := range cells {
		ref, err := address.ParseCell(cell.Ref)
		if err != nil {
			return nil, fmt.Errorf("invalid cell reference %q: %w", cell.Ref, err)
		}
		refText := cellRef(ref.Column, ref.Row)
		typ, value, err := normalizeCellValue(cell.Type, cell.Value)
		if err != nil {
			return nil, fmt.Errorf("invalid value for cell %s: %w", refText, err)
		}
		byRef[refText] = normalizedCellWrite{
			Ref:     ref,
			RefText: refText,
			Type:    typ,
			Value:   value,
		}
	}

	writes := make([]normalizedCellWrite, 0, len(byRef))
	for _, write := range byRef {
		writes = append(writes, write)
	}
	sort.Slice(writes, func(i, j int) bool {
		if writes[i].Ref.Row != writes[j].Ref.Row {
			return writes[i].Ref.Row < writes[j].Ref.Row
		}
		return writes[i].Ref.Column < writes[j].Ref.Column
	})
	return writes, nil
}

func applyCellWrite(sheetData *etree.Element, prefix string, write normalizedCellWrite) SetCellResult {
	row := ensureRow(sheetData, prefix, write.Ref.Row)
	cell, created := ensureCell(row, prefix, write.RefText, write.Ref.Column)
	prevType, prevValue := summarizeCell(cell)

	cell.CreateAttr("r", write.RefText)
	clearCellContent(cell)
	writeCellValue(cell, prefix, write.Type, write.Value)
	row.RemoveAttr("spans")

	return SetCellResult{
		Ref:           write.RefText,
		Type:          write.Type,
		Value:         write.Value,
		PreviousType:  prevType,
		PreviousValue: prevValue,
		Created:       created,
	}
}

func writtenRange(cells []SetCellResult) string {
	if len(cells) == 0 {
		return ""
	}
	firstRef, err := address.ParseCell(cells[0].Ref)
	if err != nil {
		return ""
	}
	minCol, minRow, maxCol, maxRow := firstRef.Column, firstRef.Row, firstRef.Column, firstRef.Row
	for _, cell := range cells[1:] {
		ref, err := address.ParseCell(cell.Ref)
		if err != nil {
			continue
		}
		if ref.Column < minCol {
			minCol = ref.Column
		}
		if ref.Column > maxCol {
			maxCol = ref.Column
		}
		if ref.Row < minRow {
			minRow = ref.Row
		}
		if ref.Row > maxRow {
			maxRow = ref.Row
		}
	}
	return rangeRef(minCol, minRow, maxCol, maxRow)
}

func ClearCells(req *ClearCellsRequest) (*ClearCellsResult, error) {
	if req == nil {
		return nil, fmt.Errorf("clear cells request is nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if req.SheetRef.PartURI == "" {
		return nil, fmt.Errorf("sheet %q has no worksheet part URI", req.SheetRef.Name)
	}

	doc, err := req.Package.ReadXMLPart(req.SheetRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "worksheet") {
		return nil, fmt.Errorf("worksheet part %s root element not found", req.SheetRef.PartURI)
	}

	minCol, minRow, maxCol, maxRow := req.Range.Bounds()
	sheetData := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheetData")
	result := &ClearCellsResult{}
	formulaInvalidated := false
	if sheetData != nil {
		for _, row := range namespaces.FindChildren(sheetData, namespaces.NsSpreadsheetML, "row") {
			rowNum, ok := rowNumber(row)
			if !ok || rowNum < minRow || rowNum > maxRow {
				continue
			}
			for _, cell := range namespaces.FindChildren(row, namespaces.NsSpreadsheetML, "c") {
				ref, ok := cellReference(cell)
				if !ok || ref.Column < minCol || ref.Column > maxCol {
					continue
				}
				result.Cleared++
				result.Refs = append(result.Refs, cellRef(ref.Column, ref.Row))
				if cellHasFormula(cell) {
					formulaInvalidated = true
				}
				clearCellContent(cell)
				if cell.SelectAttrValue("s", "") == "" {
					row.RemoveChild(cell)
				}
			}
			row.RemoveAttr("spans")
		}
	}
	updateDimension(root, root.Space)

	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	if formulaInvalidated {
		if err := invalidateCalcChainForFullRecalc(req.Package, req.WorkbookURI); err != nil {
			return nil, err
		}
	}
	return result, nil
}

func invalidateCalcChainForFullRecalc(session opc.PackageSession, workbookURI string) error {
	resolvedWorkbookURI, err := resolveWorkbookURIForCalcInvalidation(session, workbookURI)
	if err != nil {
		return err
	}

	relsDoc, err := readOrCreateWorkbookRels(session, resolvedWorkbookURI)
	if err != nil {
		return err
	}
	removedRelCount := 0
	calcChainParts := removeWorkbookRelationships(relsDoc.Root(), resolvedWorkbookURI, func(rel *etree.Element) bool {
		if rel.SelectAttrValue("Type", "") != namespaces.RelCalcChain {
			return false
		}
		removedRelCount++
		return true
	})
	for _, part := range session.ListParts() {
		uri := opc.NormalizeURI(part.URI)
		if uri == "/xl/calcChain.xml" || part.ContentType == namespaces.ContentTypeCalcChain {
			calcChainParts = append(calcChainParts, uri)
		}
	}
	if removedRelCount > 0 {
		if err := session.ReplaceXMLPart(workbookRelsURI(resolvedWorkbookURI), relsDoc); err != nil {
			return fmt.Errorf("failed to replace workbook relationships: %w", err)
		}
	}
	removed := make(map[string]bool, len(calcChainParts))
	for _, calcPart := range calcChainParts {
		calcPart = opc.NormalizeURI(calcPart)
		if calcPart == "" || removed[calcPart] || !partExists(session, calcPart) {
			continue
		}
		if err := session.RemovePart(calcPart); err != nil {
			return fmt.Errorf("failed to remove calcChain %s: %w", calcPart, err)
		}
		removed[calcPart] = true
	}

	return EnsureFullCalcOnLoad(session, resolvedWorkbookURI)
}

func resolveWorkbookURIForCalcInvalidation(session opc.PackageSession, workbookURI string) (string, error) {
	if workbookURI != "" {
		return workbookURI, nil
	}
	for _, part := range session.ListParts() {
		switch part.ContentType {
		case namespaces.ContentTypeWorkbook, namespaces.ContentTypeWorkbookMacro, namespaces.ContentTypeWorkbookAddin, namespaces.ContentTypeWorkbookTemplate:
			return opc.NormalizeURI(part.URI), nil
		}
	}
	return "", fmt.Errorf("workbook URI is required when changing formula cells")
}

func normalizeCellValue(typ CellValueType, value string) (CellValueType, string, error) {
	if typ == "" {
		typ = CellValueString
	}
	if typ == CellValueAuto {
		typ = inferCellValueType(value)
	}
	switch typ {
	case CellValueString:
		return typ, value, nil
	case CellValueNumber:
		literal := strings.TrimSpace(value)
		if literal == "" {
			return "", "", fmt.Errorf("number value cannot be empty")
		}
		parsed, err := strconv.ParseFloat(literal, 64)
		if err != nil || math.IsInf(parsed, 0) || math.IsNaN(parsed) {
			return "", "", fmt.Errorf("invalid number value %q", value)
		}
		return typ, literal, nil
	case CellValueBool:
		switch strings.ToLower(strings.TrimSpace(value)) {
		case "true", "1":
			return typ, "1", nil
		case "false", "0":
			return typ, "0", nil
		default:
			return "", "", fmt.Errorf("invalid bool value %q", value)
		}
	case CellValueFormula:
		formula := strings.TrimSpace(value)
		formula = strings.TrimPrefix(formula, "=")
		if formula == "" {
			return "", "", fmt.Errorf("formula value cannot be empty")
		}
		return typ, formula, nil
	default:
		return "", "", fmt.Errorf("invalid cell value type %q", typ)
	}
}

func inferCellValueType(value string) CellValueType {
	trimmed := strings.TrimSpace(value)
	if strings.HasPrefix(trimmed, "=") {
		return CellValueFormula
	}
	lower := strings.ToLower(trimmed)
	if lower == "true" || lower == "false" {
		return CellValueBool
	}
	if parsed, err := strconv.ParseFloat(trimmed, 64); err == nil && !math.IsInf(parsed, 0) && !math.IsNaN(parsed) {
		return CellValueNumber
	}
	return CellValueString
}

func writeCellValue(cell *etree.Element, prefix string, typ CellValueType, value string) {
	switch typ {
	case CellValueString:
		cell.CreateAttr("t", "inlineStr")
		inline := newElement(prefix, "is")
		text := newElement(prefix, "t")
		if needsSpacePreserve(value) {
			text.CreateAttr("xml:space", "preserve")
		}
		text.SetText(value)
		inline.AddChild(text)
		addCellChild(cell, inline)
	case CellValueNumber:
		valueElem := newElement(prefix, "v")
		valueElem.SetText(value)
		addCellChild(cell, valueElem)
	case CellValueBool:
		cell.CreateAttr("t", "b")
		valueElem := newElement(prefix, "v")
		valueElem.SetText(value)
		addCellChild(cell, valueElem)
	case CellValueFormula:
		formula := newElement(prefix, "f")
		formula.SetText(value)
		addCellChild(cell, formula)
	}
}

func needsSpacePreserve(value string) bool {
	return value != strings.Trim(value, " \t\r\n")
}
