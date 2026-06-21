package cli

import (
	"fmt"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
	"github.com/spf13/cobra"
)

// pptxMediaCmd is the `pptx media` command group: embed, replace, and inspect
// local audio/video clips on slides.
var pptxMediaCmd = &cobra.Command{
	Use:   "media",
	Short: "Embed, replace, and inspect slide audio/video media",
	Long: `Embed, replace, and inspect local audio/video clips on slides.

A clip is stored as a p:pic carrying the dual legacy+modern media representation
(a:videoFile/a:audioFile + p14:media), a poster image, and click-to-play wired via
a:hlinkClick action="ppaction://media" plus a passive media-registration node in
the slide's p:timing tree. Local files only (online/streaming is not supported).

  add      embed a local video/audio clip on a slide
  replace  replace the bytes/poster/kind of an existing media clip
  list     report embedded media across the presentation (read-only)`,
}

func init() {
	pptxCmd.AddCommand(pptxMediaCmd)
}

// pptxMediaListReadbackCommand builds the JSON readback follow-up command.
func pptxMediaListReadbackCommand(filePath string, slide int) string {
	cmd := fmt.Sprintf("ooxml --json pptx media list %s", pptxXLSXCommandArg(filePath))
	if slide > 0 {
		cmd += fmt.Sprintf(" --slide %d", slide)
	}
	return cmd
}

// pptxMediaReadbackCommands builds the follow-up command set for a media mutation.
func pptxMediaReadbackCommands(destinationFile string, slide int) PPTXBridgeReadbackCommands {
	return pptxBridgeReadbackCommands(destinationFile, slide, func(path string) string {
		return pptxMediaListReadbackCommand(path, slide)
	})
}

// parseMediaShapeSelector resolves a --shape id or --shape-name into a selector.
func parseMediaShapeSelector(shapeID int, shapeName string) (selectors.Selector, error) {
	if shapeID > 0 && shapeName != "" {
		return nil, InvalidArgsError("specify only one of --shape or --shape-name")
	}
	if shapeID > 0 {
		return &selectors.ShapeIDSelector{ID: shapeID}, nil
	}
	if shapeName != "" {
		return &selectors.ShapeNameSelector{Name: shapeName}, nil
	}
	return nil, InvalidArgsError("one of --shape <id> or --shape-name <name> is required")
}
