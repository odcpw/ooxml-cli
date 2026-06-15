package normalize

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// TestIntegrationMultiLayout tests placeholder normalization on the multi-layout fixture.
func TestIntegrationMultiLayout(t *testing.T) {
	testFixture(t, "multi-layout", []int{1, 2})
}

// TestIntegrationMinimalTitle tests placeholder normalization on the minimal-title fixture.
func TestIntegrationMinimalTitle(t *testing.T) {
	testFixture(t, "minimal-title", []int{1})
}

// TestIntegrationTitleContent tests placeholder normalization on the title-content fixture.
func TestIntegrationTitleContent(t *testing.T) {
	testFixture(t, "title-content", []int{1, 2})
}

// TestIntegrationPictureplaceholder tests placeholder normalization on the picture-placeholder fixture.
func TestIntegrationPicturePlaceholder(t *testing.T) {
	testFixture(t, "picture-placeholder", []int{1})
}

// TestIntegrationTableSlide tests placeholder normalization on the table-slide fixture.
func TestIntegrationTableSlide(t *testing.T) {
	testFixture(t, "table-slide", []int{1})
}

// TestIntegrationNotesSlide tests placeholder normalization on the notes-slide fixture.
func TestIntegrationNotesSlide(t *testing.T) {
	testFixture(t, "notes-slide", []int{1})
}

// TestIntegrationNotesHandout tests placeholder normalization on the notes-handout fixture.
func TestIntegrationNotesHandout(t *testing.T) {
	testFixture(t, "notes-handout", []int{1})
}

// TestIntegrationProducersLibreOffice tests placeholder normalization from LibreOffice producer.
func TestIntegrationProducersLibreOffice(t *testing.T) {
	testFixture(t, "producers/libreoffice", []int{1, 2})
}

// TestIntegrationProducersPowerPoint tests placeholder normalization from PowerPoint producer.
func TestIntegrationProducersPowerPoint(t *testing.T) {
	testFixture(t, "producers/powerpoint", []int{1, 2})
}

// TestIntegrationProducersGoogleSlides tests placeholder normalization from Google Slides producer.
func TestIntegrationProducersGoogleSlides(t *testing.T) {
	testFixture(t, "producers/google-slides", []int{1, 2})
}

// testFixture is a helper that loads a fixture PPTX and normalizes placeholders for each slide.
func testFixture(t *testing.T, fixtureName string, slideNumbers []int) {
	// Load the PPTX file using relative path (from normalize package)
	fixturePath := filepath.Join("../../../testdata/pptx", fixtureName, "presentation.pptx")

	// Skip if file doesn't exist
	if _, err := os.Stat(fixturePath); err != nil {
		t.Skipf("fixture file not found at %s", fixturePath)
		return
	}

	pkg, err := opc.Open(fixturePath)
	if err != nil {
		t.Fatalf("failed to open package: %v", err)
	}
	defer pkg.Close()

	// Parse the presentation graph
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	// For each slide, extract and normalize placeholders
	for _, slideNum := range slideNumbers {
		if slideNum < 1 || slideNum > len(graph.Slides) {
			t.Logf("skipping slide %d (out of range)", slideNum)
			continue
		}

		slide := graph.Slides[slideNum-1]

		// Load slide XML
		slideDoc, err := pkg.ReadXMLPart(slide.PartURI)
		if err != nil {
			t.Fatalf("failed to read slide %d: %v", slideNum, err)
		}

		// Get slide shapes
		slideShapes := extractShapesFromDoc(t, slideDoc)

		// Find the layout
		var layout *inspect.LayoutRef
		for _, l := range graph.Layouts {
			if l.PartURI == slide.LayoutPartURI {
				layout = &l
				break
			}
		}

		if layout == nil {
			t.Logf("slide %d has no layout reference, skipping", slideNum)
			continue
		}

		// Load layout XML
		layoutDoc, err := pkg.ReadXMLPart(layout.PartURI)
		if err != nil {
			t.Fatalf("failed to read layout: %v", err)
		}

		layoutShapes := extractShapesFromDoc(t, layoutDoc)

		// Find the master
		var master *inspect.MasterRef
		for _, m := range graph.Masters {
			if m.PartURI == layout.MasterPartURI {
				master = &m
				break
			}
		}

		masterShapes := []*etree.Element{}
		if master != nil {
			masterDoc, err := pkg.ReadXMLPart(master.PartURI)
			if err != nil {
				t.Logf("warning: failed to read master: %v", err)
			} else {
				masterShapes = extractShapesFromDoc(t, masterDoc)
			}
		}

		// Build layout context for key generation
		layoutPhs := parseShapesForPlaceholders(layoutShapes)
		layoutRoles := make(map[string]int)
		for _, ph := range layoutPhs {
			role := CanonicalRole(ph.Type)
			if role != "" {
				layoutRoles[role]++
			}
		}
		layoutCtx := NewSimpleLayoutContext(layoutRoles)

		// Normalize placeholders
		req := &NormalizePlaceholdersRequest{
			SlideShapes:   slideShapes,
			LayoutShapes:  layoutShapes,
			MasterShapes:  masterShapes,
			LayoutContext: layoutCtx,
		}

		results := NormalizePlaceholders(req)

		// Generate golden JSON
		golden := struct {
			Fixture      string                  `json:"fixture"`
			Slide        int                     `json:"slide"`
			Placeholders []model.PlaceholderInfo `json:"placeholders"`
		}{
			Fixture:      fixtureName,
			Slide:        slideNum,
			Placeholders: results,
		}

		// Marshal to JSON
		jsonData, err := json.MarshalIndent(golden, "", "  ")
		if err != nil {
			t.Fatalf("failed to marshal JSON: %v", err)
		}

		// Log the results
		t.Logf("✓ normalized %s slide %d: %d placeholders", fixtureName, slideNum, len(results))
		for _, p := range results {
			t.Logf("  - %s: role=%s idx=%d type=%s", p.ShapeName, p.Role, p.Index, p.ResolvedType)
		}

		// Store golden data in testdata for comparison
		// Resolve path relative to project root (not test file location)
		pkgDir := "../../../"
		goldenDir := filepath.Join(pkgDir, "testdata", "normalize", filepath.Dir(fixtureName))
		goldenFile := filepath.Join(goldenDir, fmt.Sprintf("%s-slide%d.json", filepath.Base(fixtureName), slideNum))

		assertNormalizeGolden(t, goldenFile, jsonData)
	}
}

func assertNormalizeGolden(t *testing.T, goldenFile string, actual []byte) {
	t.Helper()
	if os.Getenv("UPDATE_GOLDENS") == "1" {
		if err := os.MkdirAll(filepath.Dir(goldenFile), 0o755); err != nil {
			t.Fatalf("failed to create golden directory: %v", err)
		}
		if err := os.WriteFile(goldenFile, actual, 0o644); err != nil {
			t.Fatalf("failed to update golden file %s: %v", goldenFile, err)
		}
	}

	expected, err := os.ReadFile(goldenFile)
	if err != nil {
		t.Fatalf("failed to read golden file %s: %v", goldenFile, err)
	}
	if !bytes.Equal(bytes.TrimSpace(normalizeGoldenLineEndings(expected)), bytes.TrimSpace(normalizeGoldenLineEndings(actual))) {
		t.Fatalf("golden mismatch for %s\nexpected:\n%s\nactual:\n%s", goldenFile, expected, actual)
	}
	t.Logf("✓ golden file matches: %s", goldenFile)
}

func normalizeGoldenLineEndings(data []byte) []byte {
	return bytes.ReplaceAll(data, []byte("\r\n"), []byte("\n"))
}

// extractShapesFromDoc extracts shape elements from a presentation XML document.
func extractShapesFromDoc(t *testing.T, doc *etree.Document) []*etree.Element {
	if doc == nil {
		return []*etree.Element{}
	}

	// Find the shape tree (p:cSld/p:spTree)
	root := doc.Root()
	if root == nil {
		return []*etree.Element{}
	}

	// Find p:cSld
	cSld := root.FindElement("{http://schemas.openxmlformats.org/presentationml/2006/main}cSld")
	if cSld == nil {
		// Try with just the tag name
		cSld = root.FindElement("cSld")
		if cSld == nil {
			return []*etree.Element{}
		}
	}

	// Find p:spTree
	spTree := cSld.FindElement("{http://schemas.openxmlformats.org/presentationml/2006/main}spTree")
	if spTree == nil {
		// Try with just the tag name
		spTree = cSld.FindElement("spTree")
		if spTree == nil {
			return []*etree.Element{}
		}
	}

	// Extract shape elements: p:sp, p:pic, p:graphicFrame, p:grpSp
	var shapes []*etree.Element

	for _, child := range spTree.ChildElements() {
		// Extract just the local tag name (remove namespace if present)
		tag := child.Tag
		if i := strings.LastIndex(tag, "}"); i >= 0 {
			tag = tag[i+1:]
		}

		switch tag {
		case "sp":
			shapes = append(shapes, child)
		case "pic":
			shapes = append(shapes, child)
		case "graphicFrame":
			shapes = append(shapes, child)
		case "grpSp":
			shapes = append(shapes, child)
		}
	}

	return shapes
}
