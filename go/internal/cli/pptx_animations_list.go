package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/spf13/cobra"
)

var pptxAnimationsListCmd = &cobra.Command{
	Use:   "list <file>",
	Short: "List per-slide animations, builds, and embedded media",
	Long: `Report each slide's animation timing read-only.

For every slide it walks the p:timing tree and reports, in playback order, the
animation effects it finds. Each effect is classified by (presetClass, behavior
element, animEffect filter) - NOT by presetID - as one of the in-scope entrance
kinds (appear, fade, wipe, flyIn) or, conservatively, as "unsupported:<raw>" when
it is out of scope (motion paths, emphasis, exit, etc.). Out-of-scope effects are
reported, never dropped, and counted in unsupportedCount.

It also reports per-paragraph builds (p:bldP) and embedded video/audio (p:pic
with a:videoFile/a:audioFile + p14:media), resolving their media and poster parts
and whether a click-to-play trigger is present.

Stale targets are flagged without modification: an effect or build whose shape id
is absent (missing-shape), a paragraph range past the shape's paragraph count
(pRg-out-of-range), or a media reference whose relationship is undeclared
(dangling-rel) or whose part is missing (missing-part).

Examples:
  ooxml pptx animations list deck.pptx
  ooxml --json pptx animations list deck.pptx`,
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

		report, err := inspect.ReadAnimations(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to inspect animations: %v", err)
		}

		if GetGlobalConfig(cmd).Format == "json" {
			return writeGlobalJSON(cmd, report)
		}
		return writeCLIOutput(cmd, []byte(formatPPTXAnimationsText(report)))
	},
}

func formatPPTXAnimationsText(report *inspect.AnimationsReport) string {
	var b strings.Builder
	b.WriteString("Slides:\n")
	if len(report.Slides) == 0 {
		b.WriteString("  (none)\n")
	}
	for _, s := range report.Slides {
		fmt.Fprintf(&b, "  Slide %d:\n", s.Slide)
		if !s.HasTiming && len(s.Media) == 0 {
			b.WriteString("    no animations\n")
			continue
		}
		fmt.Fprintf(&b, "    timing: %s\n", presentAbsent(s.HasTiming))
		if len(s.Effects) == 0 {
			b.WriteString("    effects: (none)\n")
		} else {
			fmt.Fprintf(&b, "    effects (%d, unsupported=%d):\n", len(s.Effects), s.UnsupportedCount)
			for _, e := range s.Effects {
				b.WriteString("      " + describeEffect(e) + "\n")
			}
		}
		for _, bld := range s.Builds {
			b.WriteString("    build: " + describeBuild(bld) + "\n")
		}
		for _, m := range s.Media {
			b.WriteString("    media: " + describeMedia(m) + "\n")
		}
	}
	return strings.TrimRight(b.String(), "\n")
}

func describeEffect(e inspect.AnimationEffect) string {
	target := fmt.Sprintf("spid=%d", e.Spid)
	if e.ShapeName != "" {
		target = fmt.Sprintf("%q (spid=%d)", e.ShapeName, e.Spid)
	}
	line := fmt.Sprintf("[%d] %s start=%s target=%s", e.SequencePos, e.EffectKind, e.StartType, target)
	if e.Filter != "" {
		line += " filter=" + e.Filter
	}
	if e.ParagraphRange != nil {
		line += fmt.Sprintf(" paragraphs=%d-%d", e.ParagraphRange.Start, e.ParagraphRange.End)
	}
	if e.Stale {
		line += " STALE:" + e.StaleReason
	}
	return line
}

func describeBuild(b inspect.BuildInfo) string {
	target := fmt.Sprintf("spid=%d", b.Spid)
	if b.ShapeName != "" {
		target = fmt.Sprintf("%q (spid=%d)", b.ShapeName, b.Spid)
	}
	line := fmt.Sprintf("%s build=%s", target, b.Build)
	if b.Stale {
		line += " STALE:" + b.StaleReason
	}
	return line
}

func describeMedia(m inspect.MediaInfo) string {
	target := fmt.Sprintf("spid=%d", m.Spid)
	if m.ShapeName != "" {
		target = fmt.Sprintf("%q (spid=%d)", m.ShapeName, m.Spid)
	}
	line := fmt.Sprintf("%s kind=%s clickToPlay=%t", target, m.Kind, m.HasClickToPlay)
	if m.IsExternal {
		line += " external part=" + m.MediaPartURI
	} else if m.MediaPartURI != "" {
		line += " part=" + m.MediaPartURI
	}
	if m.Stale {
		line += " STALE:" + m.StaleReason
	}
	return line
}

func presentAbsent(v bool) string {
	if v {
		return "present"
	}
	return "absent"
}

func init() {
	pptxAnimationsCmd.AddCommand(pptxAnimationsListCmd)
}
