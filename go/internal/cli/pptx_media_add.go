package cli

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	pptxmutate "github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/spf13/cobra"
)

// PPTXMediaAddResult is the JSON result for `pptx media add`.
type PPTXMediaAddResult struct {
	File                       string   `json:"file"`
	Output                     string   `json:"output,omitempty"`
	DryRun                     bool     `json:"dryRun"`
	Action                     string   `json:"action"`
	Slide                      int      `json:"slide"`
	ShapeID                    int      `json:"shapeId"`
	ShapeName                  string   `json:"shapeName"`
	Kind                       string   `json:"kind"`
	MediaPartURI               string   `json:"mediaPartUri"`
	MediaContentType           string   `json:"mediaContentType"`
	PosterPartURI              string   `json:"posterPartUri"`
	MediaRelationshipID        string   `json:"mediaRelationshipId"`
	AVRelationshipID           string   `json:"avRelationshipId"`
	PosterRelationshipID       string   `json:"posterRelationshipId"`
	PlayTrigger                string   `json:"playTrigger"`
	PosterSynthesized          bool     `json:"posterSynthesized"`
	EmitPlayCmd                bool     `json:"emitPlayCmd"`
	RenderUnconfirmed          bool     `json:"renderUnconfirmed"`
	Warnings                   []string `json:"warnings,omitempty"`
	PPTXBridgeReadbackCommands `json:",inline"`
}

var (
	pptxMediaAddSlide       int
	pptxMediaAddFile        string
	pptxMediaAddKind        string
	pptxMediaAddPoster      string
	pptxMediaAddName        string
	pptxMediaAddX           int64
	pptxMediaAddY           int64
	pptxMediaAddCX          int64
	pptxMediaAddCY          int64
	pptxMediaAddPlayTrigger string
	pptxMediaAddPlayCmd     bool
	pptxMediaAddVolume      int
	pptxMediaAddMute        bool
	pptxMediaAddInsertAfter int
)

var pptxMediaAddCmd = &cobra.Command{
	Use:   "add <file>",
	Short: "Embed a local video or audio clip on a slide",
	Long: `Embed a local media clip on a slide as a click-to-play p:pic.

The clip is stored with the dual legacy+modern media representation, a poster
frame (synthesized when --poster is omitted), and click-to-play wired via the
verified a:hlinkClick action="ppaction://media" + passive media node path. The
optional --play-cmd also emits the Tier-D p:cmd playFrom(0.0) timing trigger,
whose exact spelling is unverified against real PowerPoint output.

Only local files are accepted; online/streaming media is not supported.

Examples:
  ooxml pptx media add deck.pptx --slide 1 --file clip.mp4
  ooxml pptx media add deck.pptx --slide 2 --file narration.m4a --kind audio
  ooxml pptx media add deck.pptx --slide 1 --file clip.mp4 --poster thumb.png --name Intro`,
	Args: cobra.ExactArgs(1),
	RunE: runPPTXMediaAdd,
}

func runPPTXMediaAdd(cmd *cobra.Command, args []string) error {
	filePath := args[0]
	if _, err := os.Stat(filePath); err != nil {
		return FileNotFoundError(filePath)
	}
	if pptxMediaAddSlide < 1 {
		return InvalidArgsError("--slide must be >= 1")
	}
	mediaPath := strings.TrimSpace(pptxMediaAddFile)
	if mediaPath == "" {
		return InvalidArgsError("--file is required (local .mp4/.m4a/.mp3/... path)")
	}
	if isLikelyURL(mediaPath) {
		return InvalidArgsError("online/streaming media is not supported; --file must be a local path")
	}
	mediaData, err := os.ReadFile(mediaPath)
	if err != nil {
		return NewCLIErrorf(ExitInvalidArgs, "failed to read --file: %v", err)
	}
	mediaExt := filepath.Ext(mediaPath)

	// Resolve kind: explicit --kind wins, else auto-detect from extension.
	var kind pptxmutate.MediaKind
	if strings.TrimSpace(pptxMediaAddKind) != "" {
		kind, err = pptxmutate.ParseMediaKind(pptxMediaAddKind)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
	} else {
		kind = pptxmutate.MediaKindForExtension(mediaExt)
		if kind == "" {
			return NewCLIErrorf(ExitInvalidArgs, "could not detect media kind from extension %q; pass --kind video|audio", mediaExt)
		}
	}
	mediaCT := pptxmutate.ContentTypeForMediaExt(mediaExt)

	// Optional poster.
	var posterData []byte
	var posterCT string
	if p := strings.TrimSpace(pptxMediaAddPoster); p != "" {
		posterData, err = os.ReadFile(p)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "failed to read --poster: %v", err)
		}
		posterCT = posterContentTypeForPath(p)
	}

	playTrigger := strings.ToLower(strings.TrimSpace(pptxMediaAddPlayTrigger))
	if playTrigger != "" && playTrigger != "click" && playTrigger != "none" {
		return InvalidArgsError("--play-trigger must be click or none")
	}

	var warnings []string
	volume := pptxMediaAddVolume
	if volume < 0 || volume > 100 {
		warnings = append(warnings, fmt.Sprintf("--volume %d clamped to 0..100", volume))
	}

	mutOpts, err := GetValidatedMutationOptions(cmd)
	if err != nil {
		return err
	}
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
	if err != nil {
		return err
	}

	var result *PPTXMediaAddResult
	if err := writer.Write(func(session opc.PackageSession) error {
		graph, err := inspect.ParsePresentation(session)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", err)
		}
		if pptxMediaAddSlide > len(graph.Slides) {
			return NewCLIErrorf(ExitTargetNotFound, "slide %d not found (presentation has %d slides)", pptxMediaAddSlide, len(graph.Slides))
		}
		slideRef := graph.Slides[pptxMediaAddSlide-1]
		x, y, cx, cy := resolvePPTXMediaGeometry(cmd, graph.SlideSize)

		ins, err := pptxmutate.InsertMedia(&pptxmutate.InsertMediaRequest{
			Package:           session,
			SlideRef:          &slideRef,
			MediaData:         mediaData,
			MediaContentType:  mediaCT,
			MediaExt:          mediaExt,
			Kind:              kind,
			PosterData:        posterData,
			PosterContentType: posterCT,
			Name:              strings.TrimSpace(pptxMediaAddName),
			X:                 x,
			Y:                 y,
			CX:                cx,
			CY:                cy,
			PlayTrigger:       playTrigger,
			EmitPlayCmd:       pptxMediaAddPlayCmd,
			Volume:            volume,
			Mute:              pptxMediaAddMute,
			InsertAfterID:     pptxMediaAddInsertAfter,
		})
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "failed to embed media: %v", err)
		}

		if ins.PosterSynthesized {
			warnings = append(warnings, "no --poster supplied; a placeholder poster image was synthesized")
		}
		if ins.EmitPlayCmd {
			warnings = append(warnings, "--play-cmd emitted the Tier-D playFrom(0.0) trigger; its exact spelling is unverified against real PowerPoint")
		}

		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		result = &PPTXMediaAddResult{
			File:                 filePath,
			Output:               destinationFile,
			DryRun:               mutOpts != nil && mutOpts.DryRun,
			Action:               "pptx.media.add",
			Slide:                pptxMediaAddSlide,
			ShapeID:              ins.ShapeID,
			ShapeName:            ins.ShapeName,
			Kind:                 ins.Kind,
			MediaPartURI:         ins.MediaPartURI,
			MediaContentType:     ins.MediaContentType,
			PosterPartURI:        ins.PosterPartURI,
			MediaRelationshipID:  ins.MediaRelID,
			AVRelationshipID:     ins.AVRelID,
			PosterRelationshipID: ins.PosterRelID,
			PlayTrigger:          ins.PlayTrigger,
			PosterSynthesized:    ins.PosterSynthesized,
			EmitPlayCmd:          ins.EmitPlayCmd,
			RenderUnconfirmed:    ins.EmitPlayCmd,
			Warnings:             warnings,
		}
		result.PPTXBridgeReadbackCommands = pptxMediaReadbackCommands(destinationFile, pptxMediaAddSlide)
		return nil
	}); err != nil {
		return err
	}

	if GetGlobalConfig(cmd).Format == "json" {
		return outputAnimationsJSON(cmd, result)
	}
	return writeCLIOutput(cmd, []byte(fmt.Sprintf("embedded %s %s on slide %d as shape %d (%s)",
		result.Kind, result.MediaPartURI, result.Slide, result.ShapeID, result.PlayTrigger)))
}

// resolvePPTXMediaGeometry returns the media pic EMU geometry, defaulting to a
// centred box sized at half the slide when flags are not provided. Explicit
// --x/--y are honoured (an explicit 0 anchors at the slide edge).
func resolvePPTXMediaGeometry(cmd *cobra.Command, size inspect.SlideSizeInfo) (x, y, cx, cy int64) {
	slideCX := size.CX
	slideCY := size.CY
	if slideCX <= 0 {
		slideCX = 10 * emuPerInch
	}
	if slideCY <= 0 {
		slideCY = int64(7.5 * float64(emuPerInch))
	}

	cx = pptxMediaAddCX
	cy = pptxMediaAddCY
	if cx <= 0 {
		cx = slideCX / 2
	}
	if cy <= 0 {
		cy = slideCY / 2
	}

	xChanged := cmd != nil && cmd.Flags().Changed("x")
	yChanged := cmd != nil && cmd.Flags().Changed("y")

	x = pptxMediaAddX
	y = pptxMediaAddY
	if !xChanged {
		x = (slideCX - cx) / 2
		if x < 0 {
			x = 0
		}
	}
	if !yChanged {
		y = (slideCY - cy) / 2
		if y < 0 {
			y = 0
		}
	}
	return x, y, cx, cy
}

// isLikelyURL reports whether a path looks like a remote URL rather than a local
// file (online/streaming media is out of scope).
func isLikelyURL(s string) bool {
	lower := strings.ToLower(s)
	return strings.HasPrefix(lower, "http://") ||
		strings.HasPrefix(lower, "https://") ||
		strings.HasPrefix(lower, "ftp://") ||
		strings.HasPrefix(lower, "rtmp://") ||
		strings.HasPrefix(lower, "rtsp://")
}

// posterContentTypeForPath returns a content type for a poster image path.
func posterContentTypeForPath(path string) string {
	switch strings.ToLower(filepath.Ext(path)) {
	case ".jpg", ".jpeg":
		return "image/jpeg"
	case ".gif":
		return "image/gif"
	case ".bmp":
		return "image/bmp"
	default:
		return "image/png"
	}
}

func init() {
	f := pptxMediaAddCmd.Flags()
	f.IntVar(&pptxMediaAddSlide, "slide", 0, "1-based slide number to embed the clip on (required)")
	f.StringVar(&pptxMediaAddFile, "file", "", "local media file (.mp4/.m4a/.mp3/.wav/...) (required)")
	f.StringVar(&pptxMediaAddKind, "kind", "", "media kind: video or audio (default: auto-detect from --file extension)")
	f.StringVar(&pptxMediaAddPoster, "poster", "", "poster image (default: a placeholder PNG is synthesized)")
	f.StringVar(&pptxMediaAddName, "name", "", "shape name (default: derived from the media file basename)")
	f.Int64Var(&pptxMediaAddX, "x", 0, "left position in EMUs (default: centred)")
	f.Int64Var(&pptxMediaAddY, "y", 0, "top position in EMUs (default: centred)")
	f.Int64Var(&pptxMediaAddCX, "cx", 0, "width in EMUs (default: half the slide width)")
	f.Int64Var(&pptxMediaAddCY, "cy", 0, "height in EMUs (default: half the slide height)")
	f.StringVar(&pptxMediaAddPlayTrigger, "play-trigger", "click", "play trigger: click or none")
	f.BoolVar(&pptxMediaAddPlayCmd, "play-cmd", false, "also emit the Tier-D p:cmd playFrom(0.0) timing trigger (fixture-unverified)")
	f.IntVar(&pptxMediaAddVolume, "volume", 80, "playback volume 0..100")
	f.BoolVar(&pptxMediaAddMute, "mute", false, "mute the clip")
	f.IntVar(&pptxMediaAddInsertAfter, "insert-after-shape", 0, "place the media pic after this shape id (default: append)")

	// Reuse the chart geometry flag variables for x/y/cx/cy resolution so
	// resolvePPTXChartGeometry honours explicit --x/--y. We bind the same vars.
	AddMutationFlags(pptxMediaAddCmd)
	pptxMediaCmd.AddCommand(pptxMediaAddCmd)
}
