package mutate

import (
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// TestInsertTableRow tests row insertion
func TestInsertTableRow_Basic(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/table-simple/presentation.pptx")
	require.NoError(t, err, "failed to open test presentation")
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	require.Greater(t, len(graph.Slides), 1, "should have at least 2 slides")

	slideRef := graph.Slides[1] // Table is on slide 2

	result, err := InsertTableRow(&InsertTableRowRequest{
		Package:          pkg,
		SlideRef:         &slideRef,
		TableID:          2,
		InsertAtRowIndex: 0,
	})

	require.NoError(t, err, "failed to insert row")
	require.NotNil(t, result)
	assert.Equal(t, 0, result.InsertedRowIndex)
	assert.Equal(t, 3, result.CellCount)
}

// TestDeleteTableRow tests row deletion
func TestDeleteTableRow_Basic(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/table-simple/presentation.pptx")
	require.NoError(t, err, "failed to open test presentation")
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)

	slideRef := graph.Slides[1]

	result, err := DeleteTableRow(&DeleteTableRowRequest{
		Package:  pkg,
		SlideRef: &slideRef,
		TableID:  2,
		RowIndex: 1,
	})

	require.NoError(t, err, "failed to delete row")
	require.NotNil(t, result)
	assert.Equal(t, 1, result.DeletedRowIndex)
	assert.Equal(t, 3, result.CellCount)
}

// TestDeleteTableRow_WithMerge tests error handling for merged cells
func TestDeleteTableRow_WithMerge(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/table-merged/presentation.pptx")
	require.NoError(t, err, "failed to open test presentation")
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)

	slideRef := graph.Slides[1]

	// Try to delete row 1, which contains merged cells
	_, err = DeleteTableRow(&DeleteTableRowRequest{
		Package:  pkg,
		SlideRef: &slideRef,
		TableID:  2,
		RowIndex: 1,
	})

	require.Error(t, err, "should error when row contains merged cells")
	assert.Contains(t, err.Error(), "merge")
}

// TestInsertTableColumn tests column insertion
func TestInsertTableColumn_Basic(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/table-simple/presentation.pptx")
	require.NoError(t, err, "failed to open test presentation")
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)

	slideRef := graph.Slides[1]

	result, err := InsertTableColumn(&InsertTableColumnRequest{
		Package:             pkg,
		SlideRef:            &slideRef,
		TableID:             2,
		InsertAtColumnIndex: 0,
	})

	require.NoError(t, err, "failed to insert column")
	require.NotNil(t, result)
	assert.Equal(t, 0, result.InsertedColumnIndex)
	assert.Equal(t, 3, result.RowCount)
	assert.Greater(t, result.Width, int64(0))
}

// TestDeleteTableColumn tests column deletion
func TestDeleteTableColumn_Basic(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/table-simple/presentation.pptx")
	require.NoError(t, err, "failed to open test presentation")
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)

	slideRef := graph.Slides[1]

	result, err := DeleteTableColumn(&DeleteTableColumnRequest{
		Package:     pkg,
		SlideRef:    &slideRef,
		TableID:     2,
		ColumnIndex: 1,
	})

	require.NoError(t, err, "failed to delete column")
	require.NotNil(t, result)
	assert.Equal(t, 1, result.DeletedColumnIndex)
	assert.Equal(t, 3, result.RowCount)
}

// TestDeleteTableColumn_WithMerge tests error handling for merged cells
func TestDeleteTableColumn_WithMerge(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/table-merged/presentation.pptx")
	require.NoError(t, err, "failed to open test presentation")
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)

	slideRef := graph.Slides[1]

	// Try to delete column 0, which contains merged cells
	_, err = DeleteTableColumn(&DeleteTableColumnRequest{
		Package:     pkg,
		SlideRef:    &slideRef,
		TableID:     2,
		ColumnIndex: 0,
	})

	require.Error(t, err, "should error when column contains merged cells")
	assert.Contains(t, err.Error(), "merge")
}

func TestInsertTableRow_WithMerge(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/table-merged/presentation.pptx")
	require.NoError(t, err, "failed to open test presentation")
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)

	slideRef := graph.Slides[1]

	_, err = InsertTableRow(&InsertTableRowRequest{
		Package:          pkg,
		SlideRef:         &slideRef,
		TableID:          2,
		InsertAtRowIndex: 2,
	})

	require.Error(t, err, "should error when inserting into a vertical merge")
	assert.Contains(t, err.Error(), "merge")
}

func TestInsertTableColumn_WithMerge(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/table-merged/presentation.pptx")
	require.NoError(t, err, "failed to open test presentation")
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)

	slideRef := graph.Slides[1]

	_, err = InsertTableColumn(&InsertTableColumnRequest{
		Package:             pkg,
		SlideRef:            &slideRef,
		TableID:             2,
		InsertAtColumnIndex: 1,
	})

	require.Error(t, err, "should error when inserting into a horizontal merge")
	assert.Contains(t, err.Error(), "merge")
}

func TestDeleteTableRow_RejectsLastRow(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/table-simple/presentation.pptx")
	require.NoError(t, err, "failed to open test presentation")
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)

	slideRef := graph.Slides[1]

	for i := 0; i < 2; i++ {
		_, err = DeleteTableRow(&DeleteTableRowRequest{
			Package:  pkg,
			SlideRef: &slideRef,
			TableID:  2,
			RowIndex: 0,
		})
		require.NoError(t, err, "failed to delete setup row %d", i)
	}

	_, err = DeleteTableRow(&DeleteTableRowRequest{
		Package:  pkg,
		SlideRef: &slideRef,
		TableID:  2,
		RowIndex: 0,
	})
	require.Error(t, err, "should reject deleting the last row")
	assert.Contains(t, err.Error(), "last row")
}

func TestDeleteTableColumn_RejectsLastColumn(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/table-simple/presentation.pptx")
	require.NoError(t, err, "failed to open test presentation")
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)

	slideRef := graph.Slides[1]

	for i := 0; i < 2; i++ {
		_, err = DeleteTableColumn(&DeleteTableColumnRequest{
			Package:     pkg,
			SlideRef:    &slideRef,
			TableID:     2,
			ColumnIndex: 0,
		})
		require.NoError(t, err, "failed to delete setup column %d", i)
	}

	_, err = DeleteTableColumn(&DeleteTableColumnRequest{
		Package:     pkg,
		SlideRef:    &slideRef,
		TableID:     2,
		ColumnIndex: 0,
	})
	require.Error(t, err, "should reject deleting the last column")
	assert.Contains(t, err.Error(), "last column")
}
