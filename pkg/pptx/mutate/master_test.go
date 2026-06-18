package mutate

import (
	"fmt"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// TestAddTextPlaceholderToMaster tests adding a text placeholder to a master
func TestAddTextPlaceholderToMaster(t *testing.T) {
	// Open the multi-layout test fixture
	pkg, err := opc.Open("../../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	masterURI := "/ppt/slideMasters/slideMaster1.xml"

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
		{
			name:      "invalid: negative width",
			phType:    PlaceholderTypeBody,
			cx:        -1,
			cy:        4572000,
			expectErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := &AddTextPlaceholderToMasterRequest{
				Package:         pkg,
				MasterPartURI:   masterURI,
				PlaceholderType: tt.phType,
				X:               914400,
				Y:               1371600,
				CX:              tt.cx,
				CY:              tt.cy,
			}

			result, err := AddTextPlaceholderToMaster(req)

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

			// Verify shape was added to master
			masterDoc, err := pkg.ReadXMLPart(masterURI)
			require.NoError(t, err)

			spTree := masterDoc.FindElement(".//spTree")
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
			assert.True(t, foundShape, "added shape should be found in master")
		})
	}
}

// TestAddPicturePlaceholderToMaster tests adding a picture placeholder to a master
func TestAddPicturePlaceholderToMaster(t *testing.T) {
	// Open the multi-layout test fixture
	pkg, err := opc.Open("../../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	masterURI := "/ppt/slideMasters/slideMaster1.xml"

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
		{
			name:      "invalid: zero height",
			cx:        8229600,
			cy:        0,
			expectErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := &AddPicturePlaceholderToMasterRequest{
				Package:       pkg,
				MasterPartURI: masterURI,
				X:             914400,
				Y:             1371600,
				CX:            tt.cx,
				CY:            tt.cy,
			}

			result, err := AddPicturePlaceholderToMaster(req)

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

			// Verify shape was added to master
			masterDoc, err := pkg.ReadXMLPart(masterURI)
			require.NoError(t, err)

			spTree := masterDoc.FindElement(".//spTree")
			require.NotNil(t, spTree, "shape tree should exist")

			// Find the added shape
			var foundShape bool
			for _, sp := range spTree.FindElements("sp") {
				nvSpPr := sp.FindElement("nvSpPr")
				if nvSpPr == nil {
					continue
				}
				cNvPr := nvSpPr.FindElement("cNvPr")
				if cNvPr != nil && cNvPr.SelectAttrValue("name", "") == result.ShapeName {
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
			assert.True(t, foundShape, "added shape should be found in master")
		})
	}
}

func TestAddPicturePlaceholderToMaster_ExplicitZeroIndex(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/title-content/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	masterURI := "/ppt/slideMasters/slideMaster1.xml"
	result, err := AddPicturePlaceholderToMaster(&AddPicturePlaceholderToMasterRequest{
		Package:       pkg,
		MasterPartURI: masterURI,
		X:             1000,
		Y:             2000,
		CX:            3000,
		CY:            4000,
		Idx:           0,
		ExplicitIdx:   true,
	})
	require.NoError(t, err)
	assert.Equal(t, 0, result.Idx)

	masterDoc, err := pkg.ReadXMLPart(masterURI)
	require.NoError(t, err)
	spTree := masterDoc.FindElement(".//spTree")
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
	t.Fatal("explicit idx=0 master placeholder not found")
}

// TestMasterPlaceholderGeometry tests that geometry is correctly set on master placeholders
func TestMasterPlaceholderGeometry(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	masterURI := "/ppt/slideMasters/slideMaster1.xml"

	x, y, cx, cy := int64(914400), int64(1371600), int64(8229600), int64(4572000)

	result, err := AddTextPlaceholderToMaster(&AddTextPlaceholderToMasterRequest{
		Package:         pkg,
		MasterPartURI:   masterURI,
		PlaceholderType: PlaceholderTypeBody,
		X:               x,
		Y:               y,
		CX:              cx,
		CY:              cy,
	})
	require.NoError(t, err)

	// Verify geometry in the master
	masterDoc, err := pkg.ReadXMLPart(masterURI)
	require.NoError(t, err)

	spTree := masterDoc.FindElement(".//spTree")
	require.NotNil(t, spTree)

	// Find the added shape by the specific ID returned from the result
	for _, sp := range spTree.FindElements("sp") {
		nvSpPr := sp.FindElement("nvSpPr")
		if nvSpPr == nil {
			continue
		}
		cNvPr := nvSpPr.FindElement("cNvPr")
		if cNvPr == nil {
			continue
		}
		shapeID := cNvPr.SelectAttrValue("id", "")
		if shapeID != fmt.Sprintf("%d", result.ShapeID) {
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
		assert.Equal(t, fmt.Sprintf("%d", x), offX, "x coordinate should match")
		assert.Equal(t, fmt.Sprintf("%d", y), offY, "y coordinate should match")

		ext := xfrm.FindElement("ext")
		require.NotNil(t, ext)
		extCX := ext.SelectAttrValue("cx", "")
		extCY := ext.SelectAttrValue("cy", "")
		assert.Equal(t, fmt.Sprintf("%d", cx), extCX, "width should match")
		assert.Equal(t, fmt.Sprintf("%d", cy), extCY, "height should match")

		break
	}
}

// TestMasterPlaceholderTextBody tests that text placeholder has proper text body
func TestMasterPlaceholderTextBody(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	masterURI := "/ppt/slideMasters/slideMaster1.xml"

	_, err = AddTextPlaceholderToMaster(&AddTextPlaceholderToMasterRequest{
		Package:         pkg,
		MasterPartURI:   masterURI,
		PlaceholderType: PlaceholderTypeBody,
		X:               914400,
		Y:               1371600,
		CX:              8229600,
		CY:              4572000,
	})
	require.NoError(t, err)

	// Verify text body in the master
	masterDoc, err := pkg.ReadXMLPart(masterURI)
	require.NoError(t, err)

	spTree := masterDoc.FindElement(".//spTree")
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

// TestUpdateMasterDefaultTextStyle tests updating default text styles on a master
func TestUpdateMasterDefaultTextStyle(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	masterURI := "/ppt/slideMasters/slideMaster1.xml"

	tests := []struct {
		name      string
		styleType string
		style     *DefaultTextStyleInfo
		expectErr bool
	}{
		{
			name:      "update title style",
			styleType: "title",
			style: &DefaultTextStyleInfo{
				FontSize:  4400,
				FontName:  "+mj-lt",
				Alignment: "ctr",
				Color:     "tx1",
			},
			expectErr: false,
		},
		{
			name:      "update body style",
			styleType: "body",
			style: &DefaultTextStyleInfo{
				FontSize:    3200,
				FontName:    "+mn-lt",
				Alignment:   "l",
				Color:       "tx1",
				SpaceBefore: 182880,
				SpaceAfter:  0,
			},
			expectErr: false,
		},
		{
			name:      "invalid: empty style type",
			styleType: "",
			style: &DefaultTextStyleInfo{
				FontSize: 3200,
			},
			expectErr: true,
		},
		{
			name:      "invalid: unsupported style type",
			styleType: "unknown",
			style: &DefaultTextStyleInfo{
				FontSize: 3200,
			},
			expectErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := &UpdateMasterDefaultTextStyleRequest{
				Package:       pkg,
				MasterPartURI: masterURI,
				StyleType:     tt.styleType,
				Style:         tt.style,
			}

			err := UpdateMasterDefaultTextStyle(req)

			if tt.expectErr {
				assert.Error(t, err, "expected error but got none")
				return
			}

			require.NoError(t, err, "unexpected error: %v", err)

			// Verify the style was updated
			masterDoc, err := pkg.ReadXMLPart(masterURI)
			require.NoError(t, err)

			txStyles := masterDoc.FindElement(".//txStyles")
			require.NotNil(t, txStyles, "txStyles should exist")

			// Find the style element
			var styleElem *etree.Element
			if tt.styleType == "title" {
				styleElem = txStyles.FindElement("titleStyle")
			} else if tt.styleType == "body" {
				styleElem = txStyles.FindElement("bodyStyle")
			} else if tt.styleType == "other" {
				styleElem = txStyles.FindElement("otherStyle")
			}

			require.NotNil(t, styleElem, "style element should exist: %s", tt.styleType)

			// Verify level paragraph properties exist
			lvl1pPr := styleElem.FindElement("a:lvl1pPr")
			require.NotNil(t, lvl1pPr, "level 1 paragraph properties should exist")
		})
	}
}

func TestUpdateMasterDefaultTextStyleCreatesTxStylesBeforeExtLst(t *testing.T) {
	const masterURI = "/ppt/slideMasters/slideMaster1.xml"
	session := newMockPackageSession()
	session.xmlParts[masterURI] = []byte(`<?xml version="1.0" encoding="UTF-8"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld name="Office Theme">
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr/>
    </p:spTree>
  </p:cSld>
  <p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
  <p:extLst>
    <p:ext uri="{11111111-1111-1111-1111-111111111111}"/>
  </p:extLst>
</p:sldMaster>`)
	session.contentTypes[masterURI] = "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"

	err := UpdateMasterDefaultTextStyle(&UpdateMasterDefaultTextStyleRequest{
		Package:       session,
		MasterPartURI: masterURI,
		StyleType:     "body",
		Style: &DefaultTextStyleInfo{
			FontSize:  3200,
			FontName:  "+mn-lt",
			Alignment: "l",
			Color:     "tx1",
		},
	})
	require.NoError(t, err)

	masterDoc, err := session.ReadXMLPart(masterURI)
	require.NoError(t, err)
	root := masterDoc.Root()
	require.NotNil(t, root)

	txStylesIdx := -1
	extLstIdx := -1
	for i, child := range root.ChildElements() {
		switch localTag(child.Tag) {
		case "txStyles":
			txStylesIdx = i
		case "extLst":
			extLstIdx = i
		}
	}
	require.NotEqual(t, -1, txStylesIdx, "txStyles should be created")
	require.NotEqual(t, -1, extLstIdx, "extLst should remain present")
	assert.Less(t, txStylesIdx, extLstIdx, "txStyles must precede extLst in slide master child order")
}

func TestUpdateMasterDefaultTextStyleUpdatesAllScriptFonts(t *testing.T) {
	const masterURI = "/ppt/slideMasters/slideMaster1.xml"
	session := newMockPackageSession()
	session.xmlParts[masterURI] = []byte(`<?xml version="1.0" encoding="UTF-8"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld name="Office Theme">
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr/>
    </p:spTree>
  </p:cSld>
  <p:txStyles>
    <p:bodyStyle>
      <a:lvl1pPr>
        <a:defRPr sz="1800">
          <a:latin typeface="+mj-lt"/>
          <a:ea typeface="+mj-ea"/>
          <a:cs typeface="+mj-cs"/>
        </a:defRPr>
      </a:lvl1pPr>
    </p:bodyStyle>
  </p:txStyles>
</p:sldMaster>`)
	session.contentTypes[masterURI] = "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"

	err := UpdateMasterDefaultTextStyle(&UpdateMasterDefaultTextStyleRequest{
		Package:       session,
		MasterPartURI: masterURI,
		StyleType:     "body",
		Style: &DefaultTextStyleInfo{
			FontName: "+mn-lt",
		},
	})
	require.NoError(t, err)

	masterDoc, err := session.ReadXMLPart(masterURI)
	require.NoError(t, err)
	defRPr := findDescendantByLocal(masterDoc.Root(), "defRPr")
	require.NotNil(t, defRPr)
	assert.Equal(t, "+mn-lt", findDirectChildByLocal(defRPr, "latin").SelectAttrValue("typeface", ""))
	assert.Equal(t, "+mn-ea", findDirectChildByLocal(defRPr, "ea").SelectAttrValue("typeface", ""))
	assert.Equal(t, "+mn-cs", findDirectChildByLocal(defRPr, "cs").SelectAttrValue("typeface", ""))
}

func findDescendantByLocal(elem *etree.Element, local string) *etree.Element {
	if elem == nil {
		return nil
	}
	if localTag(elem.Tag) == local {
		return elem
	}
	for _, child := range elem.ChildElements() {
		if found := findDescendantByLocal(child, local); found != nil {
			return found
		}
	}
	return nil
}

// TestMasterPlaceholderInheritance tests that placeholders on masters are inherited by layouts
func TestMasterPlaceholderInheritance(t *testing.T) {
	// This test verifies that when a placeholder is added to a master,
	// it can be referenced by slides created from layouts that use that master
	pkg, err := opc.Open("../../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	masterURI := "/ppt/slideMasters/slideMaster1.xml"

	// Add a new placeholder to the master
	result, err := AddTextPlaceholderToMaster(&AddTextPlaceholderToMasterRequest{
		Package:         pkg,
		MasterPartURI:   masterURI,
		PlaceholderType: PlaceholderTypeBody,
		X:               914400,
		Y:               1371600,
		CX:              8229600,
		CY:              4572000,
		Idx:             10, // Explicit idx to ensure it's available for inheritance
		ExplicitIdx:     true,
	})
	require.NoError(t, err, "failed to add placeholder to master")

	// Verify the placeholder was added
	masterDoc, err := pkg.ReadXMLPart(masterURI)
	require.NoError(t, err)

	spTree := masterDoc.FindElement(".//spTree")
	require.NotNil(t, spTree)

	// Find the added placeholder
	var foundPlaceholder bool
	for _, sp := range spTree.FindElements("sp") {
		nvSpPr := sp.FindElement("nvSpPr")
		if nvSpPr == nil {
			continue
		}
		cNvPr := nvSpPr.FindElement("cNvPr")
		if cNvPr == nil {
			continue
		}
		if cNvPr.SelectAttrValue("name", "") == result.ShapeName {
			foundPlaceholder = true
			// Verify the placeholder has the correct idx for inheritance
			nvPr := nvSpPr.FindElement("nvPr")
			ph := nvPr.FindElement("ph")
			idxStr := ph.SelectAttrValue("idx", "")
			assert.Equal(t, "10", idxStr, "placeholder idx should be available for inheritance")
			break
		}
	}
	assert.True(t, foundPlaceholder, "placeholder should be found in master")
}

// TestMultiplePlaceholderIndexAllocation tests that placeholder indices are allocated correctly
func TestMultiplePlaceholderIndexAllocation(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	masterURI := "/ppt/slideMasters/slideMaster1.xml"

	// Add multiple placeholders and verify indices don't conflict
	result1, err := AddTextPlaceholderToMaster(&AddTextPlaceholderToMasterRequest{
		Package:         pkg,
		MasterPartURI:   masterURI,
		PlaceholderType: PlaceholderTypeBody,
		X:               914400,
		Y:               1371600,
		CX:              8229600,
		CY:              4572000,
	})
	require.NoError(t, err)

	result2, err := AddTextPlaceholderToMaster(&AddTextPlaceholderToMasterRequest{
		Package:         pkg,
		MasterPartURI:   masterURI,
		PlaceholderType: PlaceholderTypeSubtitle,
		X:               914400,
		Y:               6000000,
		CX:              8229600,
		CY:              914400,
	})
	require.NoError(t, err)

	// Verify indices are different
	assert.NotEqual(t, result1.Idx, result2.Idx, "placeholder indices should be unique")
	assert.Greater(t, result2.Idx, result1.Idx, "second placeholder should have higher index")
}
