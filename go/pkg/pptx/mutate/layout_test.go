package mutate

import (
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// TestAddTextPlaceholder tests adding a text placeholder to a layout
func TestAddTextPlaceholder(t *testing.T) {
	// Open the multi-layout test fixture
	pkg, err := opc.Open("../../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	layoutURI := "/ppt/slideLayouts/slideLayout1.xml"

	tests := []struct {
		name      string
		phType    PlaceholderType
		cx, cy    int64
		expectErr bool
	}{
		{
			name:      "body placeholder with auto idx",
			phType:    PlaceholderTypeBody,
			cx:        8229600,
			cy:        4572000,
			expectErr: false,
		},
		{
			name:      "subtitle placeholder",
			phType:    PlaceholderTypeSubtitle,
			cx:        9144000,
			cy:        1371600,
			expectErr: false,
		},
		{
			name:      "invalid: zero dimensions",
			phType:    PlaceholderTypeBody,
			cx:        0,
			cy:        4572000,
			expectErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := &AddTextPlaceholderRequest{
				Package:         pkg,
				LayoutPartURI:   layoutURI,
				PlaceholderType: tt.phType,
				X:               914400,
				Y:               1371600,
				CX:              tt.cx,
				CY:              tt.cy,
			}

			result, err := AddTextPlaceholder(req)

			if tt.expectErr {
				assert.Error(t, err, "expected error but got none")
				return
			}

			require.NoError(t, err, "unexpected error: %v", err)
			require.NotNil(t, result, "result should not be nil")

			// Validate result
			assert.True(t, result.ShapeID > 0, "shape ID should be positive")
			assert.NotEmpty(t, result.ShapeName, "shape name should not be empty")
			assert.True(t, result.Idx >= 0, "placeholder index should be non-negative")

			// Verify shape was added to layout
			layoutDoc, err := pkg.ReadXMLPart(layoutURI)
			require.NoError(t, err)

			spTree := layoutDoc.FindElement(".//spTree")
			require.NotNil(t, spTree, "shape tree should exist")

			// Find the added shape by shape name
			var foundShape bool
			for _, sp := range spTree.FindElements("sp") {
				nvSpPr := sp.FindElement("nvSpPr")
				if nvSpPr == nil {
					continue
				}
				cNvPr := nvSpPr.FindElement("cNvPr")
				if cNvPr == nil {
					continue
				}
				shapeName := cNvPr.SelectAttrValue("name", "")
				if shapeName == result.ShapeName {
					foundShape = true
					// Verify placeholder attributes
					nvPr := nvSpPr.FindElement("nvPr")
					require.NotNil(t, nvPr, "nvPr should exist")
					phElem := nvPr.FindElement("ph")
					require.NotNil(t, phElem, "placeholder element should exist")

					phType := phElem.SelectAttrValue("type", "")
					assert.Equal(t, string(tt.phType), phType, "placeholder type should match")

					phIdx := phElem.SelectAttrValue("idx", "")
					assert.NotEmpty(t, phIdx, "placeholder idx should be set")

					break
				}
			}
			assert.True(t, foundShape, "added shape should be found in layout")
		})
	}
}

// TestAddPicturePlaceholder tests adding a picture placeholder to a layout
func TestAddPicturePlaceholder(t *testing.T) {
	// Open the multi-layout test fixture
	pkg, err := opc.Open("../../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	layoutURI := "/ppt/slideLayouts/slideLayout1.xml"

	tests := []struct {
		name      string
		cx, cy    int64
		expectErr bool
	}{
		{
			name:      "picture placeholder with auto idx",
			cx:        8229600,
			cy:        4572000,
			expectErr: false,
		},
		{
			name:      "invalid: zero width",
			cx:        0,
			cy:        4572000,
			expectErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := &AddPicturePlaceholderRequest{
				Package:       pkg,
				LayoutPartURI: layoutURI,
				X:             914400,
				Y:             1371600,
				CX:            tt.cx,
				CY:            tt.cy,
			}

			result, err := AddPicturePlaceholder(req)

			if tt.expectErr {
				assert.Error(t, err, "expected error but got none")
				return
			}

			require.NoError(t, err, "unexpected error: %v", err)
			require.NotNil(t, result, "result should not be nil")

			// Validate result
			assert.True(t, result.ShapeID > 0, "shape ID should be positive")
			assert.NotEmpty(t, result.ShapeName, "shape name should not be empty")
			assert.True(t, result.Idx >= 0, "placeholder index should be non-negative")

			// Verify shape was added to layout
			layoutDoc, err := pkg.ReadXMLPart(layoutURI)
			require.NoError(t, err)

			spTree := layoutDoc.FindElement(".//spTree")
			require.NotNil(t, spTree, "shape tree should exist")

			// Find the added shape by name
			var foundShape bool
			for _, sp := range spTree.FindElements("sp") {
				nvSpPr := sp.FindElement("nvSpPr")
				if nvSpPr == nil {
					continue
				}
				cNvPr := nvSpPr.FindElement("cNvPr")
				if cNvPr == nil {
					continue
				}
				shapeName := cNvPr.SelectAttrValue("name", "")
				if shapeName == result.ShapeName {
					foundShape = true
					// Verify placeholder attributes
					nvPr := nvSpPr.FindElement("nvPr")
					require.NotNil(t, nvPr, "nvPr should exist")
					phElem := nvPr.FindElement("ph")
					require.NotNil(t, phElem, "placeholder element should exist")

					phType := phElem.SelectAttrValue("type", "")
					assert.Equal(t, "pic", phType, "placeholder type should be 'pic'")

					phIdx := phElem.SelectAttrValue("idx", "")
					assert.NotEmpty(t, phIdx, "placeholder idx should be set")

					break
				}
			}
			assert.True(t, foundShape, "added shape should be found in layout")
		})
	}
}

// TestPlaceholderNameGeneration tests placeholder name generation
func TestAddPicturePlaceholder_ExplicitZeroIndex(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/title-content/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	layoutURI := "/ppt/slideLayouts/slideLayout2.xml"
	result, err := AddPicturePlaceholder(&AddPicturePlaceholderRequest{
		Package:       pkg,
		LayoutPartURI: layoutURI,
		X:             1000,
		Y:             2000,
		CX:            3000,
		CY:            4000,
		Idx:           0,
		ExplicitIdx:   true,
	})
	require.NoError(t, err)
	assert.Equal(t, 0, result.Idx)

	layoutDoc, err := pkg.ReadXMLPart(layoutURI)
	require.NoError(t, err)
	spTree := layoutDoc.FindElement(".//spTree")
	require.NotNil(t, spTree)
	for _, sp := range spTree.FindElements("sp") {
		nvSpPr := sp.FindElement("nvSpPr")
		if nvSpPr == nil {
			continue
		}
		cNvPr := nvSpPr.FindElement("cNvPr")
		if cNvPr == nil || cNvPr.SelectAttrValue("name", "") != result.ShapeName {
			continue
		}
		nvPr := nvSpPr.FindElement("nvPr")
		require.NotNil(t, nvPr)
		phElem := nvPr.FindElement("ph")
		require.NotNil(t, phElem)
		assert.Equal(t, "0", phElem.SelectAttrValue("idx", ""))
		return
	}
	t.Fatal("explicit idx=0 placeholder not found")
}

func TestPlaceholderNameGeneration(t *testing.T) {
	tests := []struct {
		phType   PlaceholderType
		idx      int
		expected string
	}{
		{PlaceholderTypeTitle, 0, "Title 1"},
		{PlaceholderTypeSubtitle, 1, "Subtitle 1"},
		{PlaceholderTypeBody, 1, "Content Placeholder 1"},
		{PlaceholderTypePicture, 2, "Picture Placeholder 2"},
	}

	for _, tt := range tests {
		t.Run(string(tt.phType), func(t *testing.T) {
			name := generatePlaceholderName(tt.phType, tt.idx)
			assert.Equal(t, tt.expected, name)
		})
	}
}

// TestPlaceholderGeometry tests that geometry is correctly set
func TestPlaceholderGeometry(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	layoutURI := "/ppt/slideLayouts/slideLayout1.xml"

	x, y, cx, cy := int64(914400), int64(1371600), int64(8229600), int64(4572000)

	result, err := AddTextPlaceholder(&AddTextPlaceholderRequest{
		Package:         pkg,
		LayoutPartURI:   layoutURI,
		PlaceholderType: PlaceholderTypeBody,
		X:               x,
		Y:               y,
		CX:              cx,
		CY:              cy,
	})
	require.NoError(t, err)

	// Verify geometry in the layout
	layoutDoc, err := pkg.ReadXMLPart(layoutURI)
	require.NoError(t, err)

	spTree := layoutDoc.FindElement(".//spTree")
	require.NotNil(t, spTree)

	// Find the added shape by shape name
	for _, sp := range spTree.FindElements("sp") {
		nvSpPr := sp.FindElement("nvSpPr")
		if nvSpPr == nil {
			continue
		}
		cNvPr := nvSpPr.FindElement("cNvPr")
		if cNvPr == nil {
			continue
		}
		if cNvPr.SelectAttrValue("name", "") != result.ShapeName {
			continue
		}

		spPr := sp.FindElement("spPr")
		require.NotNil(t, spPr)

		xfrm := spPr.FindElement("xfrm")
		require.NotNil(t, xfrm)

		off := xfrm.FindElement("off")
		require.NotNil(t, off)
		offX := off.SelectAttrValue("x", "")
		offY := off.SelectAttrValue("y", "")
		assert.Equal(t, "914400", offX, "x coordinate should match")
		assert.Equal(t, "1371600", offY, "y coordinate should match")

		ext := xfrm.FindElement("ext")
		require.NotNil(t, ext)
		extCX := ext.SelectAttrValue("cx", "")
		extCY := ext.SelectAttrValue("cy", "")
		assert.Equal(t, "8229600", extCX, "width should match")
		assert.Equal(t, "4572000", extCY, "height should match")

		return
	}
	t.Fatal("added shape not found in layout")
}

// TestPlaceholderTextBody tests that text placeholder has proper text body
func TestPlaceholderTextBody(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	layoutURI := "/ppt/slideLayouts/slideLayout1.xml"

	_, err = AddTextPlaceholder(&AddTextPlaceholderRequest{
		Package:         pkg,
		LayoutPartURI:   layoutURI,
		PlaceholderType: PlaceholderTypeBody,
		X:               914400,
		Y:               1371600,
		CX:              8229600,
		CY:              4572000,
	})
	require.NoError(t, err)

	// Verify text body in the layout
	layoutDoc, err := pkg.ReadXMLPart(layoutURI)
	require.NoError(t, err)

	spTree := layoutDoc.FindElement(".//spTree")
	require.NotNil(t, spTree)

	// Find the added shape
	for _, sp := range spTree.FindElements("sp") {
		txBody := sp.FindElement("txBody")
		require.NotNil(t, txBody, "text body should exist")

		bodyPr := txBody.FindElement("bodyPr")
		require.NotNil(t, bodyPr, "body properties should exist")

		lstStyle := txBody.FindElement("lstStyle")
		require.NotNil(t, lstStyle, "list style should exist")

		paragraphs := txBody.FindElements("a:p")
		assert.True(t, len(paragraphs) > 0, "should have at least one paragraph")

		break
	}
}
