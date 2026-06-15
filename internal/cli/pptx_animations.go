package cli

import "github.com/spf13/cobra"

var pptxAnimationsCmd = &cobra.Command{
	Use:   "animations",
	Short: "Inspect per-slide animations and embedded media",
	Long: `Inspect per-slide animations and embedded media.

PowerPoint stores entrance/exit/emphasis effects, paragraph builds, and media
playback wiring in each slide's p:timing tree (a sibling of p:cSld). This command
group walks that tree read-only and reports, per slide, the ordered effects
(classified as appear/fade/wipe/flyIn or unsupported:<raw>), paragraph builds,
and embedded video/audio, flagging stale targets (deleted shapes, paragraph
ranges past the text, dangling media relationships).

  list  report per-slide effects, builds, media, and stale-target flags`,
}

func init() {
	pptxCmd.AddCommand(pptxAnimationsCmd)
}
