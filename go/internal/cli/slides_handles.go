package cli

import (
	"fmt"
	"strconv"

	pptxhandle "github.com/ooxml-cli/ooxml-cli/pkg/pptx/handle"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

func pptxSlidePrimarySelector(slideRef inspect.SlideRef) string {
	return strconv.Itoa(slideRef.SlideNumber)
}

// pptxSlideHandleString mints a stable slide handle (H:pptx/s:<sldId>) for a
// slide, or "" when the slide has no native sldId OR when the sldId is shared by
// more than one slide (a handle for a duplicated sldId would mis-resolve, so we
// never mint one). sldIdCounts maps each sldId to the number of slides carrying
// it; pass nil to skip the uniqueness check.
func pptxSlideHandleString(slideRef inspect.SlideRef, sldIDCounts map[uint32]int) string {
	if slideRef.SlideID == 0 {
		return ""
	}
	if sldIDCounts != nil && sldIDCounts[slideRef.SlideID] > 1 {
		return ""
	}
	return pptxhandle.FormatSlide(slideRef.SlideID)
}

// pptxSlideIDCounts tallies how many slides carry each native p:sldId@id, so
// surfacing can omit a handle for any non-unique sldId.
func pptxSlideIDCounts(graph *inspect.PresentationGraph) map[uint32]int {
	if graph == nil {
		return nil
	}
	counts := make(map[uint32]int, len(graph.Slides))
	for _, slideRef := range graph.Slides {
		if slideRef.SlideID != 0 {
			counts[slideRef.SlideID]++
		}
	}
	return counts
}

func pptxSlideSelectors(slideRef inspect.SlideRef) []string {
	selectors := []string{pptxSlidePrimarySelector(slideRef)}
	if slideRef.PartURI != "" {
		selectors = append(selectors, "part:"+slideRef.PartURI)
	}
	if slideRef.SlideID != 0 {
		selectors = append(selectors, fmt.Sprintf("slideId:%d", slideRef.SlideID))
	}
	if slideRef.RelationshipID != "" {
		selectors = append(selectors, "rId:"+slideRef.RelationshipID)
	}
	return selectors
}

func pptxSlideLayoutNumber(graph *inspect.PresentationGraph, layoutPartURI string) int {
	if graph == nil || layoutPartURI == "" {
		return 0
	}
	for i, layout := range graph.Layouts {
		if layout.PartURI == layoutPartURI {
			return i + 1
		}
	}
	return 0
}

func pptxSlideReadbackCommand(filePath string, slideNumber int) string {
	return fmt.Sprintf("ooxml --json pptx slides show %s --slide %d --include-text --include-bounds", pptxXLSXCommandArg(filePath), slideNumber)
}

func pptxSlidesListCommand(filePath string) string {
	return fmt.Sprintf("ooxml --json pptx slides list %s", pptxXLSXCommandArg(filePath))
}

func pptxSlideSelectorsCommand(filePath string, slideNumber int) string {
	return fmt.Sprintf("ooxml --json pptx slides selectors %s --slide %d", pptxXLSXCommandArg(filePath), slideNumber)
}

func pptxSlideShapesCommand(filePath string, slideNumber int) string {
	return fmt.Sprintf("ooxml --json pptx shapes show %s --slide %d --include-text --include-bounds", pptxXLSXCommandArg(filePath), slideNumber)
}

func pptxSlideTablesCommand(filePath string, slideNumber int) string {
	return fmt.Sprintf("ooxml --json pptx tables show %s --slide %d", pptxXLSXCommandArg(filePath), slideNumber)
}

func pptxSlideLayoutCommand(filePath string, layoutNumber int) string {
	return fmt.Sprintf("ooxml --json pptx layouts show %s --layout %d", pptxXLSXCommandArg(filePath), layoutNumber)
}
