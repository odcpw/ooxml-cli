package model

import (
	"encoding/json"
	"testing"
)

func TestPlaceholderInfoJSON(t *testing.T) {
	tests := []struct {
		name          string
		placeholder   *PlaceholderInfo
		expectedKey   string
		expectedRole  string
		expectedIndex int
		expectedName  string
	}{
		{
			name: "title placeholder with role",
			placeholder: &PlaceholderInfo{
				Key:       "title",
				Role:      "title",
				ShapeName: "Title 1",
			},
			expectedKey:  "title",
			expectedRole: "title",
			expectedName: "Title 1",
		},
		{
			name: "placeholder with index",
			placeholder: &PlaceholderInfo{
				Key:       "ph:11",
				Index:     11,
				ShapeName: "Content Placeholder 2",
			},
			expectedKey:   "ph:11",
			expectedIndex: 11,
			expectedName:  "Content Placeholder 2",
		},
		{
			name: "picture placeholder",
			placeholder: &PlaceholderInfo{
				Key:       "pic:12",
				Role:      "pic",
				Index:     12,
				ShapeName: "Picture 12",
			},
			expectedKey:   "pic:12",
			expectedRole:  "pic",
			expectedIndex: 12,
			expectedName:  "Picture 12",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			data, err := json.Marshal(tt.placeholder)
			if err != nil {
				t.Fatalf("Failed to marshal: %v", err)
			}

			var result map[string]interface{}
			if err := json.Unmarshal(data, &result); err != nil {
				t.Fatalf("Failed to unmarshal: %v", err)
			}

			// Check required fields
			if result["key"] != tt.expectedKey {
				t.Errorf("Key mismatch: got %v, expected %v", result["key"], tt.expectedKey)
			}
			if result["shapeName"] != tt.expectedName {
				t.Errorf("ShapeName mismatch: got %v, expected %v", result["shapeName"], tt.expectedName)
			}

			// Check optional fields
			if tt.expectedRole != "" && result["role"] != tt.expectedRole {
				t.Errorf("Role mismatch: got %v, expected %v", result["role"], tt.expectedRole)
			}
			if tt.expectedIndex > 0 && result["index"] != float64(tt.expectedIndex) {
				t.Errorf("Index mismatch: got %v, expected %v", result["index"], tt.expectedIndex)
			}
		})
	}
}

func TestShapeInfoJSON(t *testing.T) {
	shape := &ShapeInfo{
		ID:            2,
		Name:          "Title 1",
		Type:          ShapeTypeSP,
		IsPlaceholder: true,
		Bounds: &Bounds{
			X:  1000,
			Y:  2000,
			CX: 3000,
			CY: 4000,
		},
	}

	data, err := json.Marshal(shape)
	if err != nil {
		t.Fatalf("Failed to marshal: %v", err)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("Failed to unmarshal: %v", err)
	}

	// Verify field names match CLI surface spec
	if _, ok := result["id"]; !ok {
		t.Error("Missing 'id' field")
	}
	if _, ok := result["shapeName"]; !ok {
		t.Error("Missing 'shapeName' field")
	}
	if _, ok := result["type"]; !ok {
		t.Error("Missing 'type' field")
	}
	if _, ok := result["bounds"]; !ok {
		t.Error("Missing 'bounds' field")
	}
	if _, ok := result["isPlaceholder"]; !ok {
		t.Error("Missing 'isPlaceholder' field")
	}
}

func TestDeckSummaryJSON(t *testing.T) {
	deck := &DeckSummary{
		Slides:         11,
		Masters:        1,
		Layouts:        4,
		Themes:         3,
		NotesMasters:   1,
		HandoutMasters: 1,
		MediaAssets:    1,
		CustomXmlParts: 3,
		SlideSize: &SlideDimensions{
			CX:   5327650,
			CY:   7559675,
			Unit: "emu",
		},
	}

	data, err := json.Marshal(deck)
	if err != nil {
		t.Fatalf("Failed to marshal: %v", err)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("Failed to unmarshal: %v", err)
	}

	// Verify field names match CLI surface spec
	expectedFields := []string{"slides", "masters", "layouts", "themes", "notesMasters", "handoutMasters", "mediaAssets", "customXmlParts", "slideSize"}
	for _, field := range expectedFields {
		if _, ok := result[field]; !ok {
			t.Errorf("Missing '%s' field", field)
		}
	}
}

func TestLayoutReportJSON(t *testing.T) {
	layout := &LayoutReport{
		ID:        "layout-1",
		Name:      "Titelseite",
		PartURI:   "/ppt/slideLayouts/slideLayout1.xml",
		MasterID:  "master-1",
		Preserve:  true,
		UserDrawn: true,
		Placeholders: []*PlaceholderInfo{
			{
				Key:       "title",
				Role:      "title",
				ShapeName: "Titel 4",
			},
			{
				Key:       "ph:11",
				ShapeName: "Inhaltsplatzhalter 22",
				Index:     11,
			},
			{
				Key:       "pic:12",
				Role:      "pic",
				Index:     12,
				ShapeName: "Bildplatzhalter 12",
			},
		},
	}

	data, err := json.Marshal(layout)
	if err != nil {
		t.Fatalf("Failed to marshal: %v", err)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("Failed to unmarshal: %v", err)
	}

	// Verify field names match CLI surface spec
	if _, ok := result["id"]; !ok {
		t.Error("Missing 'id' field")
	}
	if _, ok := result["name"]; !ok {
		t.Error("Missing 'name' field")
	}
	if _, ok := result["partUri"]; !ok {
		t.Error("Missing 'partUri' field")
	}
	if _, ok := result["masterId"]; !ok {
		t.Error("Missing 'masterId' field")
	}
	if _, ok := result["preserve"]; !ok {
		t.Error("Missing 'preserve' field")
	}
	if _, ok := result["userDrawn"]; !ok {
		t.Error("Missing 'userDrawn' field")
	}
	if _, ok := result["placeholders"]; !ok {
		t.Error("Missing 'placeholders' field")
	}
}

func TestSlideReportJSON(t *testing.T) {
	slide := &SlideReport{
		ID:        "slide-1",
		Slide:     1,
		PartURI:   "/ppt/slides/slide1.xml",
		LayoutRef: "layout-1",
		Shapes: []*ShapeInfo{
			{
				ID:            2,
				Name:          "Title 1",
				Type:          ShapeTypeSP,
				IsPlaceholder: true,
			},
		},
	}

	data, err := json.Marshal(slide)
	if err != nil {
		t.Fatalf("Failed to marshal: %v", err)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("Failed to unmarshal: %v", err)
	}

	// Verify field names match CLI surface spec
	if _, ok := result["id"]; !ok {
		t.Error("Missing 'id' field")
	}
	if _, ok := result["slide"]; !ok {
		t.Error("Missing 'slide' field")
	}
	if _, ok := result["partUri"]; !ok {
		t.Error("Missing 'partUri' field")
	}
	if _, ok := result["layoutRef"]; !ok {
		t.Error("Missing 'layoutRef' field")
	}
	if _, ok := result["shapes"]; !ok {
		t.Error("Missing 'shapes' field")
	}
}

func TestImageRefJSON(t *testing.T) {
	image := &ImageRef{
		RelID:       "rId4",
		TargetURI:   "media/image1.png",
		ContentType: "image/png",
	}

	data, err := json.Marshal(image)
	if err != nil {
		t.Fatalf("Failed to marshal: %v", err)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("Failed to unmarshal: %v", err)
	}

	// Verify field names
	if _, ok := result["relId"]; !ok {
		t.Error("Missing 'relId' field")
	}
	if _, ok := result["targetUri"]; !ok {
		t.Error("Missing 'targetUri' field")
	}
	if _, ok := result["contentType"]; !ok {
		t.Error("Missing 'contentType' field")
	}
}

func TestTableInfoJSON(t *testing.T) {
	table := &TableInfo{
		Rows: 2,
		Cols: 3,
		Cells: [][]string{
			{"A1", "B1", "C1"},
			{"A2", "B2", "C2"},
		},
	}

	data, err := json.Marshal(table)
	if err != nil {
		t.Fatalf("Failed to marshal: %v", err)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("Failed to unmarshal: %v", err)
	}

	// Verify field names
	if _, ok := result["rows"]; !ok {
		t.Error("Missing 'rows' field")
	}
	if _, ok := result["cols"]; !ok {
		t.Error("Missing 'cols' field")
	}
	if _, ok := result["cells"]; !ok {
		t.Error("Missing 'cells' field")
	}
}

func TestTextBlockInfoJSON(t *testing.T) {
	text := &TextBlockInfo{
		Paragraphs: []Paragraph{
			{
				Runs: []interface{}{
					&TextRun{Type: "text", Text: "Hello "},
					&TextRun{Type: "text", Text: "World"},
				},
			},
		},
		PlainText: "Hello World",
	}

	data, err := json.Marshal(text)
	if err != nil {
		t.Fatalf("Failed to marshal: %v", err)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("Failed to unmarshal: %v", err)
	}

	// Verify field names
	if _, ok := result["paragraphs"]; !ok {
		t.Error("Missing 'paragraphs' field")
	}
	if _, ok := result["plainText"]; !ok {
		t.Error("Missing 'plainText' field")
	}
}

func TestMasterReportJSON(t *testing.T) {
	master := &MasterReport{
		ID:       "master-1",
		PartURI:  "/ppt/slideMasters/slideMaster1.xml",
		ThemeURI: "/ppt/theme/theme1.xml",
		LayoutURIs: []string{
			"/ppt/slideLayouts/slideLayout1.xml",
			"/ppt/slideLayouts/slideLayout2.xml",
		},
	}

	data, err := json.Marshal(master)
	if err != nil {
		t.Fatalf("Failed to marshal: %v", err)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("Failed to unmarshal: %v", err)
	}

	// Verify field names
	if _, ok := result["id"]; !ok {
		t.Error("Missing 'id' field")
	}
	if _, ok := result["partUri"]; !ok {
		t.Error("Missing 'partUri' field")
	}
	if _, ok := result["themeUri"]; !ok {
		t.Error("Missing 'themeUri' field")
	}
	if _, ok := result["layoutUris"]; !ok {
		t.Error("Missing 'layoutUris' field")
	}
}

func TestShapeTypeEnum(t *testing.T) {
	// Verify all shape type constants are defined
	types := []ShapeType{
		ShapeTypeSP,
		ShapeTypePic,
		ShapeTypeGraphicFrame,
		ShapeTypeGroup,
	}

	expectedValues := []string{"sp", "pic", "graphicFrame", "grpSp"}

	for i, st := range types {
		if string(st) != expectedValues[i] {
			t.Errorf("ShapeType mismatch at index %d: got %s, expected %s", i, string(st), expectedValues[i])
		}
	}
}
