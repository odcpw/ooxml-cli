package mutate

import (
	"testing"

	"github.com/beevik/etree"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// TestCreateTableFromData_Simple tests simple table creation
func TestCreateTableFromData_Simple(t *testing.T) {
	req := &CreateTableFromDataRequest{
		Data: [][]string{
			{"Name", "Age", "City"},
			{"Alice", "30", "New York"},
			{"Bob", "25", "Los Angeles"},
		},
		X:               0,
		Y:               0,
		Width:           3000000,
		HasHeader:       true,
		HasBandedRows:   true,
		HeaderFillColor: "4472C4",
		BandFill1Color:  "D9E1F2",
		BandFill2Color:  "E7E6E6",
		DefaultFontSize: 18,
		BorderColor:     "000000",
		BorderWidth:     19050,
	}

	result, err := CreateTableFromData(req)
	require.NoError(t, err)
	require.NotNil(t, result)

	assert.Equal(t, int64(3000000), result.ShapeWidth)
	assert.Greater(t, result.ShapeHeight, int64(0))
	assert.NotNil(t, result.TableXML)
	assert.NotNil(t, result.TableXML.Root())

	// Check that the table has the correct structure
	tblRoot := result.TableXML.Root()
	assert.Equal(t, "tbl", tblRoot.Tag)
	assert.Equal(t, "a", tblRoot.Space)

	// Check grid
	tblGrid := tblRoot.FindElement(".//a:tblGrid")
	if tblGrid == nil {
		tblGrid = tblRoot.FindElement(".//tblGrid")
	}
	require.NotNil(t, tblGrid)

	gridCols := tblGrid.FindElements(".//a:gridCol")
	if len(gridCols) == 0 {
		gridCols = tblGrid.FindElements(".//gridCol")
	}
	assert.Len(t, gridCols, 3, "should have 3 columns")

	// Check rows
	rows := tblRoot.FindElements(".//a:tr")
	if len(rows) == 0 {
		rows = tblRoot.FindElements(".//tr")
	}
	assert.Len(t, rows, 3, "should have 3 rows (header + 2 data)")
}

// TestCreateTableFromData_EmptyData tests error handling for empty data
func TestCreateTableFromData_EmptyData(t *testing.T) {
	req := &CreateTableFromDataRequest{
		Data:  [][]string{},
		Width: 1000000,
	}

	result, err := CreateTableFromData(req)
	assert.Error(t, err)
	assert.Nil(t, result)
}

// TestCreateTableFromData_InvalidWidth tests error handling for invalid width
func TestCreateTableFromData_InvalidWidth(t *testing.T) {
	req := &CreateTableFromDataRequest{
		Data: [][]string{
			{"A", "B"},
		},
		Width: 0,
	}

	result, err := CreateTableFromData(req)
	assert.Error(t, err)
	assert.Nil(t, result)
}

// TestCreateTableFromData_InconsistentColumns tests error handling for inconsistent column count
func TestCreateTableFromData_InconsistentColumns(t *testing.T) {
	req := &CreateTableFromDataRequest{
		Data: [][]string{
			{"A", "B"},
			{"C"},
		},
		Width: 1000000,
	}

	result, err := CreateTableFromData(req)
	assert.Error(t, err)
	assert.Nil(t, result)
}

// TestCreateTableFromData_WithoutHeader tests table without header
func TestCreateTableFromData_WithoutHeader(t *testing.T) {
	req := &CreateTableFromDataRequest{
		Data: [][]string{
			{"Val1", "Val2"},
			{"Val3", "Val4"},
		},
		Width: 2000000,
	}

	result, err := CreateTableFromData(req)
	require.NoError(t, err)
	require.NotNil(t, result)

	assert.Equal(t, int64(2000000), result.ShapeWidth)
	assert.NotNil(t, result.TableXML)
}

// TestCreateTableFromData_CustomColumnWidths tests custom column widths
func TestCreateTableFromData_CustomColumnWidths(t *testing.T) {
	req := &CreateTableFromDataRequest{
		Data: [][]string{
			{"A", "B"},
			{"C", "D"},
		},
		Width:        1000000,
		ColumnWidths: []int64{600000, 400000},
	}

	result, err := CreateTableFromData(req)
	require.NoError(t, err)
	require.NotNil(t, result)

	tblRoot := result.TableXML.Root()
	tblGrid := tblRoot.FindElement(".//a:tblGrid")
	if tblGrid == nil {
		tblGrid = tblRoot.FindElement(".//tblGrid")
	}
	require.NotNil(t, tblGrid)

	gridCols := tblGrid.FindElements(".//a:gridCol")
	if len(gridCols) == 0 {
		gridCols = tblGrid.FindElements(".//gridCol")
	}
	assert.Len(t, gridCols, 2)

	// Check first column width
	width1 := gridCols[0].SelectAttrValue("w", "")
	assert.Equal(t, "600000", width1)

	// Check second column width
	width2 := gridCols[1].SelectAttrValue("w", "")
	assert.Equal(t, "400000", width2)
}

func TestNextTableShapeIDScansAllShapeTypes(t *testing.T) {
	spTree := mustParseElement(t, `<p:spTree xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
		<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
		<p:grpSpPr/>
		<p:sp>
			<p:nvSpPr><p:cNvPr id="19" name="Shape"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
			<p:spPr/>
		</p:sp>
		<p:pic>
			<p:nvPicPr><p:cNvPr id="7" name="Picture"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>
			<p:blipFill/>
			<p:spPr/>
		</p:pic>
		<p:graphicFrame>
			<p:nvGraphicFramePr><p:cNvPr id="22" name="Table"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>
			<p:xfrm/><a:graphic/>
		</p:graphicFrame>
		<p:grpSp>
			<p:nvGrpSpPr><p:cNvPr id="41" name="Group"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
			<p:grpSpPr/>
			<p:cxnSp>
				<p:nvCxnSpPr><p:cNvPr id="39" name="Connector"/><p:cNvCxnSpPr/><p:nvPr/></p:nvCxnSpPr>
				<p:spPr/>
			</p:cxnSp>
		</p:grpSp>
	</p:spTree>`)

	assert.Equal(t, 42, nextTableShapeID(spTree))
	assert.Equal(t, 42, nextSpTreeShapeID(spTree))
}

func TestNextSpTreeShapeIDReturnsOneForEmptyTree(t *testing.T) {
	assert.Equal(t, 1, nextSpTreeShapeID(etree.NewElement("spTree")))
}
