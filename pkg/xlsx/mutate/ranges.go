package mutate

import (
	"errors"
	"fmt"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

var (
	ErrRangeOverwritesFormula    = errors.New("range write would overwrite existing formula")
	ErrRangeIntersectsMergedCell = errors.New("range write intersects merged cells")
)

type RangeNullPolicy string

const (
	RangeNullSkip        RangeNullPolicy = "skip"
	RangeNullClear       RangeNullPolicy = "clear"
	RangeNullEmptyString RangeNullPolicy = "empty-string"
)

type RangeCell struct {
	Type  CellValueType
	Value string
	Null  bool
}

type SetRangeRequest struct {
	Package           opc.PackageSession
	WorkbookURI       string
	SheetRef          model.SheetRef
	Range             address.RangeRef
	Rows              [][]RangeCell
	NullPolicy        RangeNullPolicy
	OverwriteFormulas bool
}

type SetRangeResult struct {
	Updated      int             `json:"updated"`
	Created      int             `json:"created"`
	Cleared      int             `json:"cleared"`
	Skipped      int             `json:"skipped"`
	FormulaCount int             `json:"formulaCount"`
	Range        string          `json:"range"`
	Cells        []SetCellResult `json:"cells,omitempty"`
}

func NormalizeRangeNullPolicy(value string) (RangeNullPolicy, error) {
	switch RangeNullPolicy(strings.ToLower(strings.TrimSpace(value))) {
	case "", RangeNullSkip:
		return RangeNullSkip, nil
	case RangeNullClear:
		return RangeNullClear, nil
	case RangeNullEmptyString:
		return RangeNullEmptyString, nil
	default:
		return "", fmt.Errorf("invalid null policy %q (must be skip, clear, or empty-string)", value)
	}
}

func SetRange(req *SetRangeRequest) (*SetRangeResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set range request is nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if req.SheetRef.PartURI == "" {
		return nil, fmt.Errorf("sheet %q has no worksheet part URI", req.SheetRef.Name)
	}
	nullPolicy, err := NormalizeRangeNullPolicy(string(req.NullPolicy))
	if err != nil {
		return nil, err
	}
	minCol, minRow, maxCol, maxRow := req.Range.Bounds()
	rows := maxRow - minRow + 1
	cols := maxCol - minCol + 1
	if len(req.Rows) != rows {
		return nil, fmt.Errorf("range %s expects %d rows, got %d", req.Range.String(), rows, len(req.Rows))
	}
	for idx, row := range req.Rows {
		if len(row) != cols {
			return nil, fmt.Errorf("range %s expects %d columns in row %d, got %d", req.Range.String(), cols, idx+1, len(row))
		}
	}

	doc, err := req.Package.ReadXMLPart(req.SheetRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "worksheet") {
		return nil, fmt.Errorf("worksheet part %s root element not found", req.SheetRef.PartURI)
	}
	if err := rejectMergedCellIntersection(root, req.Range); err != nil {
		return nil, err
	}

	prefix := root.Space
	sheetData := ensureSheetData(root, prefix)
	if err := preflightRangeWrites(sheetData, req, minCol, minRow, nullPolicy); err != nil {
		return nil, err
	}

	result := &SetRangeResult{
		Range: req.Range.String(),
		Cells: make([]SetCellResult, 0, rows*cols),
	}
	formulaSeen := false
	formulaInvalidated := false
	for rowIdx, row := range req.Rows {
		for colIdx, cell := range row {
			ref := address.CellRef{Column: minCol + colIdx, Row: minRow + rowIdx}
			refText := cellRef(ref.Column, ref.Row)
			if cell.Null {
				switch nullPolicy {
				case RangeNullSkip:
					result.Skipped++
					continue
				case RangeNullClear:
					if cleared, clearedFormula := clearRangeCell(sheetData, ref.Row, refText); cleared {
						result.Cleared++
						if clearedFormula {
							formulaInvalidated = true
						}
					}
					continue
				case RangeNullEmptyString:
					cell.Type = CellValueString
					cell.Value = ""
				}
			}

			typ, value, err := normalizeCellValue(cell.Type, cell.Value)
			if err != nil {
				return nil, fmt.Errorf("invalid value for cell %s: %w", refText, err)
			}
			cellResult := applyCellWrite(sheetData, prefix, normalizedCellWrite{
				Ref:     ref,
				RefText: refText,
				Type:    typ,
				Value:   value,
			})
			result.Updated++
			if cellResult.Created {
				result.Created++
			}
			if cellResult.PreviousType == "formula" {
				formulaInvalidated = true
			}
			if typ == CellValueFormula {
				result.FormulaCount++
				formulaSeen = true
			}
			result.Cells = append(result.Cells, cellResult)
		}
	}
	updateDimension(root, prefix)

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

func rejectMergedCellIntersection(root *etree.Element, target address.RangeRef) error {
	mergeCells := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "mergeCells")
	if mergeCells == nil {
		return nil
	}
	for _, mergeCell := range namespaces.FindChildren(mergeCells, namespaces.NsSpreadsheetML, "mergeCell") {
		refText := mergeCell.SelectAttrValue("ref", "")
		if refText == "" {
			continue
		}
		mergedRef, err := address.ParseRange(refText)
		if err != nil {
			return fmt.Errorf("invalid merged-cell reference %q: %w", refText, err)
		}
		if rangesIntersect(target, mergedRef) {
			return fmt.Errorf("%w: %s intersects %s", ErrRangeIntersectsMergedCell, target.String(), mergedRef.String())
		}
	}
	return nil
}

func preflightRangeWrites(sheetData *etree.Element, req *SetRangeRequest, minCol, minRow int, nullPolicy RangeNullPolicy) error {
	if req.OverwriteFormulas {
		return nil
	}
	for rowIdx, row := range req.Rows {
		for colIdx, cell := range row {
			if cell.Null && nullPolicy == RangeNullSkip {
				continue
			}
			ref := address.CellRef{Column: minCol + colIdx, Row: minRow + rowIdx}
			refText := cellRef(ref.Column, ref.Row)
			rowElem := findRow(sheetData, ref.Row)
			if rowElem == nil {
				continue
			}
			cellElem := findCell(rowElem, refText)
			if cellElem != nil && cellHasFormula(cellElem) {
				return fmt.Errorf("%w: %s", ErrRangeOverwritesFormula, refText)
			}
		}
	}
	return nil
}

func clearRangeCell(sheetData *etree.Element, rowNumber int, refText string) (bool, bool) {
	row := findRow(sheetData, rowNumber)
	if row == nil {
		return false, false
	}
	cell := findCell(row, refText)
	if cell == nil {
		return false, false
	}
	hadFormula := cellHasFormula(cell)
	clearCellContent(cell)
	if cell.SelectAttrValue("s", "") == "" {
		row.RemoveChild(cell)
	}
	row.RemoveAttr("spans")
	return true, hadFormula
}

func cellHasFormula(cell *etree.Element) bool {
	return namespaces.FindChild(cell, namespaces.NsSpreadsheetML, "f") != nil
}

func rangesIntersect(a, b address.RangeRef) bool {
	aMinCol, aMinRow, aMaxCol, aMaxRow := a.Bounds()
	bMinCol, bMinRow, bMaxCol, bMaxRow := b.Bounds()
	return aMinCol <= bMaxCol && aMaxCol >= bMinCol &&
		aMinRow <= bMaxRow && aMaxRow >= bMinRow
}
