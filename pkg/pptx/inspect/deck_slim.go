package inspect

import (
	"fmt"
	"regexp"
	"strconv"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// DeckSummary is a minimal summary of a presentation
type DeckSummary struct {
	Type               string
	FilePath           string
	SlideCount         int
	MasterCount        int
	LayoutCount        int
	ThemeCount         int
	MediaCount         int
	NotesMasterCount   int
	HandoutMasterCount int
	CustomXMLCount     int
	SlideSizeEMU       *SlideDimensions
}

// SlideDimensions represents slide dimensions in EMU (English Metric Units)
type SlideDimensions struct {
	CX int64 // Width in EMU
	CY int64 // Height in EMU
}

// SummarizeDeck reads a presentation and returns a summary with minimal parsing
func SummarizeDeck(pkg *opc.Package) (*DeckSummary, error) {
	summary := &DeckSummary{
		Type:     string(opc.DetectType(pkg)),
		FilePath: "",
	}

	// Read presentation.xml
	presentationXML, err := pkg.ReadRawPart("/ppt/presentation.xml")
	if err != nil {
		return nil, fmt.Errorf("failed to read presentation.xml: %w", err)
	}

	// Parse slide size from sldSz element
	if dims := parseSlideSize(presentationXML); dims != nil {
		summary.SlideSizeEMU = dims
	}

	// Count slides, masters, layouts from sldIdLst
	if err := countPresentationParts(pkg, summary); err != nil {
		return nil, err
	}

	return summary, nil
}

// parseSlideSize extracts the slide dimensions from presentation.xml
func parseSlideSize(xmlData []byte) *SlideDimensions {
	// Look for <p:sldSz cx="..." cy="..."/>
	xmlStr := string(xmlData)

	// Simple regex-based parsing to find sldSz element
	// Pattern: <p:sldSz cx="5327650" cy="7559675"
	re := regexp.MustCompile(`<p:sldSz\s+cx="(\d+)"\s+cy="(\d+)"`)
	matches := re.FindStringSubmatch(xmlStr)

	if len(matches) >= 3 {
		cx, _ := strconv.ParseInt(matches[1], 10, 64)
		cy, _ := strconv.ParseInt(matches[2], 10, 64)
		return &SlideDimensions{CX: cx, CY: cy}
	}

	return nil
}

// countPresentationParts counts the various part types by examining the package structure
func countPresentationParts(pkg *opc.Package, summary *DeckSummary) error {
	parts := pkg.ListParts()

	for _, part := range parts {
		uri := part.URI

		// Count slides
		if strings.HasPrefix(uri, "/ppt/slides/") && strings.HasSuffix(uri, ".xml") && !strings.Contains(uri, ".rels") {
			summary.SlideCount++
		}

		// Count slide masters
		if strings.HasPrefix(uri, "/ppt/slideMasters/") && strings.HasSuffix(uri, ".xml") && !strings.Contains(uri, ".rels") {
			summary.MasterCount++
		}

		// Count slide layouts
		if strings.HasPrefix(uri, "/ppt/slideLayouts/") && strings.HasSuffix(uri, ".xml") && !strings.Contains(uri, ".rels") {
			summary.LayoutCount++
		}

		// Count themes
		if strings.HasPrefix(uri, "/ppt/theme/") && strings.HasSuffix(uri, ".xml") {
			summary.ThemeCount++
		}

		// Count media assets
		if strings.HasPrefix(uri, "/ppt/media/") {
			summary.MediaCount++
		}

		// Count notes masters
		if strings.HasPrefix(uri, "/ppt/notesMasters/") && strings.HasSuffix(uri, ".xml") && !strings.Contains(uri, ".rels") {
			summary.NotesMasterCount++
		}

		// Count handout masters
		if strings.HasPrefix(uri, "/ppt/handoutMasters/") && strings.HasSuffix(uri, ".xml") && !strings.Contains(uri, ".rels") {
			summary.HandoutMasterCount++
		}

		// Count custom XML parts
		if strings.HasPrefix(uri, "/customXml/") && strings.HasSuffix(uri, ".xml") && !strings.Contains(uri, ".rels") {
			summary.CustomXMLCount++
		}
	}

	return nil
}
