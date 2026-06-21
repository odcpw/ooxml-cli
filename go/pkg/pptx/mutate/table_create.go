package mutate

import (
	"fmt"
	"strconv"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

// CreateTableFromDataRequest holds parameters for creating a table from CSV/JSON data
type CreateTableFromDataRequest struct {
	// Table data: rows of strings
	Data [][]string

	// Table dimensions (in EMUs)
	X      int64 // Top-left X coordinate
	Y      int64 // Top-left Y coordinate
	Width  int64 // Total table width
	Height int64 // Total table height (auto-calculated if 0)

	// Table options
	HasHeader       bool   // If true, first row is header (banded)
	HasBandedRows   bool   // If true, alternate row fills are applied
	HeaderFillColor string // Header cell fill color (hex, e.g., "4472C4")
	BandFill1Color  string // Band row 1 fill color (hex, e.g., "D9E1F2")
	BandFill2Color  string // Band row 2 fill color (hex, empty means no band)

	// Cell formatting
	DefaultFontSize int    // Font size in points (default: 18)
	BorderColor     string // Border color (hex, default: "000000")
	BorderWidth     int64  // Border width in EMUs (default: 19050 = 0.5pt)

	// Column widths (if empty, divide table width equally)
	ColumnWidths []int64
}

// CreateTableFromDataResult holds the result of table creation
type CreateTableFromDataResult struct {
	// XML for the table to be inserted
	TableXML *etree.Document

	// Shape properties for the graphic frame
	ShapeWidth  int64
	ShapeHeight int64
	ShapeX      int64
	ShapeY      int64
}

// CreateTableFromData creates a table XML structure from CSV/JSON data
func CreateTableFromData(req *CreateTableFromDataRequest) (*CreateTableFromDataResult, error) {
	if req == nil {
		return nil, fmt.Errorf("request cannot be nil")
	}
	if len(req.Data) == 0 {
		return nil, fmt.Errorf("table data cannot be empty")
	}
	if len(req.Data[0]) == 0 {
		return nil, fmt.Errorf("first row must contain at least one column")
	}
	if req.Width <= 0 {
		return nil, fmt.Errorf("table width must be positive")
	}

	// Validate data consistency
	numCols := len(req.Data[0])
	for i, row := range req.Data {
		if len(row) != numCols {
			return nil, fmt.Errorf("row %d has %d columns, expected %d", i, len(row), numCols)
		}
	}

	// Set defaults
	if req.DefaultFontSize <= 0 {
		req.DefaultFontSize = 18
	}
	if req.BorderColor == "" {
		req.BorderColor = "000000"
	}
	if req.BorderWidth <= 0 {
		req.BorderWidth = 19050 // 0.5pt
	}

	// Calculate column widths
	colWidths := req.ColumnWidths
	if len(colWidths) == 0 {
		colWidth := req.Width / int64(numCols)
		colWidths = make([]int64, numCols)
		for i := 0; i < numCols; i++ {
			colWidths[i] = colWidth
		}
	} else if len(colWidths) != numCols {
		return nil, fmt.Errorf("column widths length %d doesn't match number of columns %d", len(colWidths), numCols)
	}

	// Calculate row heights (auto)
	numRows := len(req.Data)
	var rowHeight int64 = 457200 // Default: 0.5 inches in EMUs
	if req.Height > 0 {
		rowHeight = req.Height / int64(numRows)
	}

	// Create table XML
	tblDoc := etree.NewDocument()

	// Create the main table element with proper namespace handling
	tbl := etree.NewElement("tbl")
	tbl.Space = "a" // Set the namespace prefix
	tbl.CreateAttr("xmlns:a", "http://schemas.openxmlformats.org/drawingml/2006/main")

	// Add table grid (column widths)
	tblGrid := etree.NewElement("tblGrid")
	tblGrid.Space = "a"
	for _, width := range colWidths {
		gridCol := etree.NewElement("gridCol")
		gridCol.Space = "a"
		gridCol.CreateAttr("w", strconv.FormatInt(width, 10))
		tblGrid.AddChild(gridCol)
	}
	tbl.AddChild(tblGrid)

	// Add table properties
	tblPr := etree.NewElement("tblPr")
	tblPr.Space = "a"
	tbl.AddChild(tblPr)

	// Add rows
	for rowIdx, row := range req.Data {
		tr := etree.NewElement("tr")
		tr.Space = "a"
		tr.CreateAttr("h", strconv.FormatInt(rowHeight, 10))

		// Determine if this is a header row
		isHeaderRow := req.HasHeader && rowIdx == 0

		// Alternate band fill
		isBandRow := req.HasBandedRows && !isHeaderRow
		bandIdx := 0
		if isBandRow && rowIdx > 0 {
			bandIdx = (rowIdx - 1) % 2
		}

		for _, cellContent := range row {
			tc := etree.NewElement("tc")
			tc.Space = "a"

			// Add cell properties
			tcPr := etree.NewElement("tcPr")
			tcPr.Space = "a"
			tcPr.CreateAttr("lnL", "1")
			tcPr.CreateAttr("lnR", "1")
			tcPr.CreateAttr("lnT", "1")
			tcPr.CreateAttr("lnB", "1")

			// Add fill based on row type
			if isHeaderRow && req.HeaderFillColor != "" {
				solidFill := etree.NewElement("solidFill")
				solidFill.Space = "a"
				srgbClr := etree.NewElement("srgbClr")
				srgbClr.Space = "a"
				srgbClr.CreateAttr("val", req.HeaderFillColor)
				solidFill.AddChild(srgbClr)
				tcPr.AddChild(solidFill)
			} else if isBandRow && bandIdx == 0 && req.BandFill1Color != "" {
				solidFill := etree.NewElement("solidFill")
				solidFill.Space = "a"
				srgbClr := etree.NewElement("srgbClr")
				srgbClr.Space = "a"
				srgbClr.CreateAttr("val", req.BandFill1Color)
				solidFill.AddChild(srgbClr)
				tcPr.AddChild(solidFill)
			} else if isBandRow && bandIdx == 1 && req.BandFill2Color != "" {
				solidFill := etree.NewElement("solidFill")
				solidFill.Space = "a"
				srgbClr := etree.NewElement("srgbClr")
				srgbClr.Space = "a"
				srgbClr.CreateAttr("val", req.BandFill2Color)
				solidFill.AddChild(srgbClr)
				tcPr.AddChild(solidFill)
			}

			// Add borders to all cells
			for _, borderSide := range []string{"lnL", "lnR", "lnT", "lnB"} {
				ln := etree.NewElement(borderSide)
				ln.Space = "a"
				ln.CreateAttr("w", strconv.FormatInt(req.BorderWidth, 10))
				solidFill := etree.NewElement("solidFill")
				solidFill.Space = "a"
				srgbClr := etree.NewElement("srgbClr")
				srgbClr.Space = "a"
				srgbClr.CreateAttr("val", req.BorderColor)
				solidFill.AddChild(srgbClr)
				ln.AddChild(solidFill)
				tcPr.AddChild(ln)
			}

			tc.AddChild(tcPr)

			// Add text body
			txBody := etree.NewElement("txBody")
			txBody.Space = "a"
			bodyPr := etree.NewElement("bodyPr")
			bodyPr.Space = "a"
			bodyPr.CreateAttr("wrap", "none")
			bodyPr.CreateAttr("rtlCol", "0")
			txBody.AddChild(bodyPr)

			lstStyle := etree.NewElement("lstStyle")
			lstStyle.Space = "a"
			txBody.AddChild(lstStyle)

			// Add paragraph with text
			p := etree.NewElement("p")
			p.Space = "a"
			pPr := etree.NewElement("pPr")
			pPr.Space = "a"
			pPr.CreateAttr("algn", "ctr")
			p.AddChild(pPr)

			r := etree.NewElement("r")
			r.Space = "a"
			p.AddChild(r)

			rPr := etree.NewElement("rPr")
			rPr.Space = "a"
			rPr.CreateAttr("lang", "en-US")
			rPr.CreateAttr("sz", strconv.Itoa(req.DefaultFontSize*100))
			if isHeaderRow {
				rPr.CreateAttr("b", "1") // Bold header
			}
			r.AddChild(rPr)

			t := etree.NewElement("t")
			t.Space = "a"
			t.SetText(cellContent)
			r.AddChild(t)

			endParaRPr := etree.NewElement("endParaRPr")
			endParaRPr.Space = "a"
			endParaRPr.CreateAttr("lang", "en-US")
			endParaRPr.CreateAttr("sz", strconv.Itoa(req.DefaultFontSize*100))
			p.AddChild(endParaRPr)

			txBody.AddChild(p)

			tc.AddChild(txBody)
			tr.AddChild(tc)
		}

		tbl.AddChild(tr)
	}

	tblDoc.SetRoot(tbl)

	return &CreateTableFromDataResult{
		TableXML:    tblDoc,
		ShapeWidth:  req.Width,
		ShapeHeight: rowHeight * int64(numRows),
		ShapeX:      req.X,
		ShapeY:      req.Y,
	}, nil
}

// InsertTableRequest holds parameters for inserting a table on a slide
type InsertTableRequest struct {
	// OPC package session
	Package opc.PackageSession

	// Slide reference
	SlideRef *inspect.SlideRef

	// Table data (rows of strings)
	Data [][]string

	// Position and size in EMUs (English Metric Units)
	X      int64 // Left position
	Y      int64 // Top position
	Width  int64 // Table width
	Height int64 // Table height (auto-calculated if 0)

	// Table options
	HasHeader       bool   // If true, first row is header (banded)
	HasBandedRows   bool   // If true, alternate row fills are applied
	HeaderFillColor string // Header cell fill color (hex, e.g., "4472C4")
	BandFill1Color  string // Band row 1 fill color (hex, e.g., "D9E1F2")
	BandFill2Color  string // Band row 2 fill color (hex, empty means no band)

	// Cell formatting
	DefaultFontSize int    // Font size in points (default: 18)
	BorderColor     string // Border color (hex, default: "000000")
	BorderWidth     int64  // Border width in EMUs (default: 19050 = 0.5pt)

	// Column widths (if empty, divide table width equally)
	ColumnWidths []int64

	// Optional: shape name (if not provided, uses "Table_N")
	ShapeName string
}

// InsertTableResult holds the result of a successful table insertion
type InsertTableResult struct {
	// Unique shape ID assigned to the table
	ShapeID int

	// Shape name (either provided or auto-generated)
	ShapeName string

	// Actual dimensions used
	Width  int64
	Height int64
}

// InsertTable creates a new table on a slide at specific EMU coordinates.
func InsertTable(req *InsertTableRequest) (*InsertTableResult, error) {
	if req == nil {
		return nil, fmt.Errorf("request cannot be nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session cannot be nil")
	}
	if req.SlideRef == nil {
		return nil, fmt.Errorf("slide reference cannot be nil")
	}
	if len(req.Data) == 0 {
		return nil, fmt.Errorf("table data cannot be empty")
	}
	if req.Width <= 0 {
		return nil, fmt.Errorf("table width must be positive")
	}

	// Read the slide XML
	slideDoc, err := req.Package.ReadXMLPart(req.SlideRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read slide: %w", err)
	}

	// Get the shape tree
	spTree := slideDoc.FindElement(".//p:spTree")
	if spTree == nil {
		spTree = slideDoc.FindElement(".//spTree")
		if spTree == nil {
			return nil, fmt.Errorf("shape tree not found in slide")
		}
	}

	// Determine the new shape ID across all non-visual drawing properties on the slide.
	newShapeID := nextTableShapeID(spTree)

	// Determine shape name
	shapeName := req.ShapeName
	if shapeName == "" {
		shapeName = fmt.Sprintf("Table %d", newShapeID)
	}

	// Create table data from CSV/JSON
	tableCreateReq := &CreateTableFromDataRequest{
		Data:            req.Data,
		X:               req.X,
		Y:               req.Y,
		Width:           req.Width,
		Height:          req.Height,
		HasHeader:       req.HasHeader,
		HasBandedRows:   req.HasBandedRows,
		HeaderFillColor: req.HeaderFillColor,
		BandFill1Color:  req.BandFill1Color,
		BandFill2Color:  req.BandFill2Color,
		DefaultFontSize: req.DefaultFontSize,
		BorderColor:     req.BorderColor,
		BorderWidth:     req.BorderWidth,
		ColumnWidths:    req.ColumnWidths,
	}

	tableRes, err := CreateTableFromData(tableCreateReq)
	if err != nil {
		return nil, fmt.Errorf("failed to create table data: %w", err)
	}

	// Create the graphic frame element with the table
	gfElem := createTableGraphicFrame(
		newShapeID,
		shapeName,
		tableRes.ShapeX, tableRes.ShapeY,
		tableRes.ShapeWidth, tableRes.ShapeHeight,
		tableRes.TableXML,
	)

	// Insert into shape tree
	appendSpTreeChild(spTree, gfElem)

	// Write the slide back
	if err := req.Package.ReplaceXMLPart(req.SlideRef.PartURI, slideDoc); err != nil {
		return nil, fmt.Errorf("failed to write slide: %w", err)
	}

	return &InsertTableResult{
		ShapeID:   newShapeID,
		ShapeName: shapeName,
		Width:     tableRes.ShapeWidth,
		Height:    tableRes.ShapeHeight,
	}, nil
}

func nextTableShapeID(spTree *etree.Element) int {
	return nextSpTreeShapeID(spTree)
}

// createTableGraphicFrame creates a p:graphicFrame element containing a table
func createTableGraphicFrame(
	shapeID int,
	shapeName string,
	x, y, cx, cy int64,
	tableXML *etree.Document,
) *etree.Element {
	// Create graphic frame element
	gf := etree.NewElement("graphicFrame")
	gf.Space = "p"

	// Create non-visual properties
	nvGraphicFramePr := etree.NewElement("nvGraphicFramePr")
	nvGraphicFramePr.Space = "p"

	cNvPr := etree.NewElement("cNvPr")
	cNvPr.Space = "p"
	cNvPr.CreateAttr("id", strconv.Itoa(shapeID))
	cNvPr.CreateAttr("name", shapeName)
	nvGraphicFramePr.AddChild(cNvPr)

	cNvGraphicFramePr := etree.NewElement("cNvGraphicFramePr")
	cNvGraphicFramePr.Space = "p"
	nvGraphicFramePr.AddChild(cNvGraphicFramePr)

	nvPr := etree.NewElement("nvPr")
	nvPr.Space = "p"
	nvGraphicFramePr.AddChild(nvPr)

	gf.AddChild(nvGraphicFramePr)

	// Create transform (position and size)
	xfrm := etree.NewElement("xfrm")
	xfrm.Space = "p"

	off := etree.NewElement("off")
	off.Space = "a"
	off.CreateAttr("x", strconv.FormatInt(x, 10))
	off.CreateAttr("y", strconv.FormatInt(y, 10))
	xfrm.AddChild(off)

	ext := etree.NewElement("ext")
	ext.Space = "a"
	ext.CreateAttr("cx", strconv.FormatInt(cx, 10))
	ext.CreateAttr("cy", strconv.FormatInt(cy, 10))
	xfrm.AddChild(ext)

	gf.AddChild(xfrm)

	// Create graphic element
	graphic := etree.NewElement("graphic")
	graphic.Space = "a"

	graphicData := etree.NewElement("graphicData")
	graphicData.Space = "a"
	graphicData.CreateAttr("uri", "http://schemas.openxmlformats.org/drawingml/2006/table")

	// Add the table element from tableXML
	if tableXML != nil && tableXML.Root() != nil {
		// Clone the table root element
		tblClone := tableXML.Root().Copy()
		graphicData.AddChild(tblClone)
	}

	graphic.AddChild(graphicData)
	gf.AddChild(graphic)

	return gf
}
