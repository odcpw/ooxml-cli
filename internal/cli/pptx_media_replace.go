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

// PPTXMediaReplaceResult is the JSON result for `pptx media replace`.
type PPTXMediaReplaceResult struct {
	File                       string `json:"file"`
	Output                     string `json:"output,omitempty"`
	DryRun                     bool   `json:"dryRun"`
	Action                     string `json:"action"`
	Slide                      int    `json:"slide"`
	ShapeID                    int    `json:"shapeId"`
	ShapeName                  string `json:"shapeName"`
	OldKind                    string `json:"oldKind"`
	NewKind                    string `json:"newKind"`
	OldMediaURI                string `json:"oldMediaUri"`
	NewMediaURI                string `json:"newMediaUri"`
	OldContentType             string `json:"oldContentType"`
	NewContentType             string `json:"newContentType"`
	PosterReplaced             bool   `json:"posterReplaced"`
	PPTXBridgeReadbackCommands `json:",inline"`
}

var (
	pptxMediaReplaceSlide      int
	pptxMediaReplaceShape      int
	pptxMediaReplaceShapeName  string
	pptxMediaReplaceFile       string
	pptxMediaReplaceKind       string
	pptxMediaReplacePoster     string
	pptxMediaReplaceVolume     int
	pptxMediaReplaceMute       bool
	pptxMediaReplaceExpectName string
	pptxMediaReplaceExpectKind string
)

var pptxMediaReplaceCmd = &cobra.Command{
	Use:   "replace <file>",
	Short: "Replace the bytes, poster, or kind of an existing media clip",
	Long: `Replace the media (and optionally poster) of an existing embedded clip.

The clip is resolved by --shape <id> or --shape-name <name> and must be a media
pic (a plain image is rejected). Geometry, the cNvPr id/name, the hyperlink, the
p:extLst structure, and the timing node are preserved. When --kind flips, the
a:videoFile/a:audioFile element, the av relationship type, and the p:video/p:audio
timing node are rewritten.

Examples:
  ooxml pptx media replace deck.pptx --slide 1 --shape 5 --file new.mp4
  ooxml pptx media replace deck.pptx --slide 1 --shape-name Intro --file new.m4a --kind audio
  ooxml pptx media replace deck.pptx --slide 1 --shape 5 --file new.mp4 --poster thumb.png`,
	Args: cobra.ExactArgs(1),
	RunE: runPPTXMediaReplace,
}

func runPPTXMediaReplace(cmd *cobra.Command, args []string) error {
	filePath := args[0]
	if _, err := os.Stat(filePath); err != nil {
		return FileNotFoundError(filePath)
	}
	if pptxMediaReplaceSlide < 1 {
		return InvalidArgsError("--slide must be >= 1")
	}
	sel, err := parseMediaShapeSelector(pptxMediaReplaceShape, strings.TrimSpace(pptxMediaReplaceShapeName))
	if err != nil {
		return err
	}
	mediaPath := strings.TrimSpace(pptxMediaReplaceFile)
	if mediaPath == "" {
		return InvalidArgsError("--file is required (local media path)")
	}
	if isLikelyURL(mediaPath) {
		return InvalidArgsError("online/streaming media is not supported; --file must be a local path")
	}
	mediaData, err := os.ReadFile(mediaPath)
	if err != nil {
		return NewCLIErrorf(ExitInvalidArgs, "failed to read --file: %v", err)
	}
	mediaExt := filepath.Ext(mediaPath)

	var kind pptxmutate.MediaKind
	if strings.TrimSpace(pptxMediaReplaceKind) != "" {
		kind, err = pptxmutate.ParseMediaKind(pptxMediaReplaceKind)
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

	var posterData []byte
	var posterCT string
	if p := strings.TrimSpace(pptxMediaReplacePoster); p != "" {
		posterData, err = os.ReadFile(p)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "failed to read --poster: %v", err)
		}
		posterCT = posterContentTypeForPath(p)
	}

	var expectKind pptxmutate.MediaKind
	if strings.TrimSpace(pptxMediaReplaceExpectKind) != "" {
		expectKind, err = pptxmutate.ParseMediaKind(pptxMediaReplaceExpectKind)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "invalid --expect-media-kind: %v", err)
		}
	}

	var volumePtr *int
	if cmd.Flags().Changed("volume") {
		v := pptxMediaReplaceVolume
		volumePtr = &v
	}
	var mutePtr *bool
	if cmd.Flags().Changed("mute") {
		m := pptxMediaReplaceMute
		mutePtr = &m
	}

	mutOpts, err := GetValidatedMutationOptions(cmd)
	if err != nil {
		return err
	}
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
	if err != nil {
		return err
	}

	var result *PPTXMediaReplaceResult
	if err := writer.Write(func(session opc.PackageSession) error {
		graph, err := inspect.ParsePresentation(session)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", err)
		}
		if pptxMediaReplaceSlide > len(graph.Slides) {
			return NewCLIErrorf(ExitTargetNotFound, "slide %d not found (presentation has %d slides)", pptxMediaReplaceSlide, len(graph.Slides))
		}
		slideRef := graph.Slides[pptxMediaReplaceSlide-1]

		rep, err := pptxmutate.ReplaceMedia(&pptxmutate.ReplaceMediaRequest{
			Package:              session,
			SlideRef:             &slideRef,
			Selector:             sel,
			NewMediaData:         mediaData,
			NewMediaContentType:  mediaCT,
			NewMediaExt:          mediaExt,
			NewKind:              kind,
			NewPosterData:        posterData,
			NewPosterContentType: posterCT,
			Volume:               volumePtr,
			Mute:                 mutePtr,
			ExpectShapeName:      strings.TrimSpace(pptxMediaReplaceExpectName),
			ExpectKind:           expectKind,
		})
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "failed to replace media: %v", err)
		}

		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		result = &PPTXMediaReplaceResult{
			File:           filePath,
			Output:         destinationFile,
			DryRun:         mutOpts != nil && mutOpts.DryRun,
			Action:         "pptx.media.replace",
			Slide:          pptxMediaReplaceSlide,
			ShapeID:        rep.ShapeID,
			ShapeName:      rep.ShapeName,
			OldKind:        rep.OldKind,
			NewKind:        rep.NewKind,
			OldMediaURI:    rep.OldMediaURI,
			NewMediaURI:    rep.NewMediaURI,
			OldContentType: rep.OldContentType,
			NewContentType: rep.NewContentType,
			PosterReplaced: rep.PosterReplaced,
		}
		result.PPTXBridgeReadbackCommands = pptxMediaReadbackCommands(destinationFile, pptxMediaReplaceSlide)
		return nil
	}); err != nil {
		return err
	}

	if GetGlobalConfig(cmd).Format == "json" {
		return outputAnimationsJSON(cmd, result)
	}
	return writeCLIOutput(cmd, []byte(fmt.Sprintf("replaced media on slide %d shape %d (%s -> %s)",
		result.Slide, result.ShapeID, result.OldMediaURI, result.NewMediaURI)))
}

func init() {
	f := pptxMediaReplaceCmd.Flags()
	f.IntVar(&pptxMediaReplaceSlide, "slide", 0, "1-based slide number (required)")
	f.IntVar(&pptxMediaReplaceShape, "shape", 0, "target shape id (a media pic)")
	f.StringVar(&pptxMediaReplaceShapeName, "shape-name", "", "target shape name (a media pic)")
	f.StringVar(&pptxMediaReplaceFile, "file", "", "new local media file (required)")
	f.StringVar(&pptxMediaReplaceKind, "kind", "", "media kind: video or audio (default: auto from extension)")
	f.StringVar(&pptxMediaReplacePoster, "poster", "", "replacement poster image (optional)")
	f.IntVar(&pptxMediaReplaceVolume, "volume", 80, "playback volume 0..100 (only applied when set)")
	f.BoolVar(&pptxMediaReplaceMute, "mute", false, "mute the clip (only applied when set)")
	f.StringVar(&pptxMediaReplaceExpectName, "expect-shape-name", "", "guard: require the resolved shape name to match")
	f.StringVar(&pptxMediaReplaceExpectKind, "expect-media-kind", "", "guard: require the current media kind to match before replace")
	AddMutationFlags(pptxMediaReplaceCmd)
	pptxMediaCmd.AddCommand(pptxMediaReplaceCmd)
}
