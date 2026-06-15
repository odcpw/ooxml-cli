package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/spf13/cobra"
)

var pptxMediaListSlide int

var pptxMediaListCmd = &cobra.Command{
	Use:   "list <file>",
	Short: "List embedded audio/video media across the presentation",
	Long: `Report every embedded media clip in a presentation (read-only).

For each clip it reports the shape id/name, kind (video/audio), resolved media and
poster part URIs, content type, play trigger (click/cmd/none), volume, mute, and a
media-scoped stale flag (dangling media/poster relationship or missing target
part). Plain image pictures are never reported as media.

Examples:
  ooxml pptx media list deck.pptx
  ooxml --json pptx media list deck.pptx --slide 1`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		pkg, err := openPackageExpectType(filePath, opc.PackageTypePPTX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		report, err := inspect.ReadMedia(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to inspect media: %v", err)
		}

		if pptxMediaListSlide > 0 {
			report = filterMediaReportToSlide(report, pptxMediaListSlide)
		}

		if GetGlobalConfig(cmd).Format == "json" {
			return writeGlobalJSON(cmd, report)
		}
		return writeCLIOutput(cmd, []byte(formatPPTXMediaText(report)))
	},
}

// filterMediaReportToSlide keeps only the requested slide's clips.
func filterMediaReportToSlide(report *inspect.MediaReport, slide int) *inspect.MediaReport {
	out := &inspect.MediaReport{}
	for _, s := range report.Slides {
		if s.Slide == slide {
			out.Slides = append(out.Slides, s)
		}
	}
	return out
}

func formatPPTXMediaText(report *inspect.MediaReport) string {
	var b strings.Builder
	total := 0
	for _, s := range report.Slides {
		if len(s.Clips) == 0 {
			continue
		}
		fmt.Fprintf(&b, "Slide %d (%s):\n", s.Slide, s.PartURI)
		for _, c := range s.Clips {
			total++
			name := c.ShapeName
			if name == "" {
				name = "(unnamed)"
			}
			location := c.MediaPartURI
			if c.IsExternal {
				location = "external " + c.MediaPartURI
			}
			fmt.Fprintf(&b, "  shape %d %q: %s %s trigger=%s vol=%d",
				c.ShapeID, name, c.Kind, location, c.PlayTrigger, c.Volume)
			if c.Mute {
				b.WriteString(" mute")
			}
			if c.Stale {
				fmt.Fprintf(&b, " STALE(%s)", c.StaleReason)
			}
			b.WriteString("\n")
		}
	}
	if total == 0 {
		return "No embedded media found.\n"
	}
	return b.String()
}

func init() {
	pptxMediaListCmd.Flags().IntVar(&pptxMediaListSlide, "slide", 0, "restrict to one 1-based slide (0 = all)")
	pptxMediaCmd.AddCommand(pptxMediaListCmd)
}
