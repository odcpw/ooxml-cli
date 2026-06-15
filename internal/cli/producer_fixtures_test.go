package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

// TestProducerFixtures tests layouts and slides commands against all producer fixtures
func TestProducerFixtures(t *testing.T) {
	producers := []string{"powerpoint", "google-slides", "libreoffice", "python-pptx"}

	for _, producer := range producers {
		t.Run("layouts_list_"+producer, func(t *testing.T) {
			testLayoutsList(t, producer)
		})
		t.Run("slides_list_"+producer, func(t *testing.T) {
			testSlidesList(t, producer)
		})
		t.Run("layouts_show_"+producer, func(t *testing.T) {
			testLayoutsShow(t, producer)
		})
		t.Run("slides_show_"+producer, func(t *testing.T) {
			testSlidesShow(t, producer)
		})
	}
}

func testLayoutsList(t *testing.T, producer string) {
	filePath := getProducerFixturePath(producer)

	// Check if file exists
	if _, err := os.Stat(filePath); err != nil {
		t.Skip("Producer fixture not found: " + filePath)
	}

	output := runProducerFixtureJSON(t, "pptx", "layouts", "list", filePath)
	compareWithGolden(t, output, producerGoldenPath(producer, "layouts-list.json"))

	// Validate JSON structure
	var result LayoutListOutput
	err := json.Unmarshal([]byte(output), &result)
	if err != nil {
		t.Fatalf("failed to parse JSON output: %v", err)
	}

	// At least one layout should be present
	if len(result.Layouts) == 0 {
		t.Errorf("expected layouts for producer %s, got none", producer)
	}

	// Verify layout structure
	for _, layout := range result.Layouts {
		if layout.ID == "" {
			t.Errorf("layout missing ID")
		}
		if layout.Name == "" {
			t.Errorf("layout missing name for ID %s", layout.ID)
		}
		if layout.PartURI == "" {
			t.Errorf("layout missing PartURI for %s", layout.Name)
		}
	}
}

func testSlidesList(t *testing.T, producer string) {
	filePath := getProducerFixturePath(producer)

	// Check if file exists
	if _, err := os.Stat(filePath); err != nil {
		t.Skip("Producer fixture not found: " + filePath)
	}

	output := runProducerFixtureJSON(t, "pptx", "slides", "list", filePath)
	compareWithGolden(t, output, producerGoldenPath(producer, "slides-list.json"))

	// Validate JSON structure
	var result struct {
		File   string           `json:"file"`
		Slides []SlidesListItem `json:"slides"`
	}
	err := json.Unmarshal([]byte(output), &result)
	if err != nil {
		t.Fatalf("failed to parse JSON output: %v", err)
	}

	// At least one slide should be present
	if len(result.Slides) == 0 {
		t.Errorf("expected slides for producer %s, got none", producer)
	}
}

func testLayoutsShow(t *testing.T, producer string) {
	filePath := getProducerFixturePath(producer)

	// Check if file exists
	if _, err := os.Stat(filePath); err != nil {
		t.Skip("Producer fixture not found: " + filePath)
	}

	output := runProducerFixtureJSON(t, "pptx", "layouts", "show", filePath, "--layout", "1")
	compareWithGolden(t, output, producerGoldenPath(producer, "layouts-show.json"))

	// Should have some output
	if output == "" {
		t.Errorf("expected output for layouts show for producer %s", producer)
	}
}

func testSlidesShow(t *testing.T, producer string) {
	filePath := getProducerFixturePath(producer)

	// Check if file exists
	if _, err := os.Stat(filePath); err != nil {
		t.Skip("Producer fixture not found: " + filePath)
	}

	output := runProducerFixtureJSON(t, "pptx", "slides", "show", filePath, "--slide", "1")
	compareWithGolden(t, output, producerGoldenPath(producer, "slides-show.json"))

	// Should have some output
	if output == "" {
		t.Errorf("expected output for slides show for producer %s", producer)
	}
}

func runProducerFixtureJSON(t *testing.T, args ...string) string {
	t.Helper()
	out, err := executeRootForXLSXTest(t, append([]string{"--format", "json"}, args...)...)
	if err != nil {
		t.Fatalf("producer fixture command failed: %v\nargs=%v\n%s", err, args, out)
	}
	return out
}

func producerGoldenPath(producer, name string) string {
	return filepath.Join("testdata", "pptx", "producers", producer, name)
}
