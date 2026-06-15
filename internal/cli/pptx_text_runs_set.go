package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	pptselectors "github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
	"github.com/spf13/cobra"
)

var (
	pptxTextRunsSetSlide     int
	pptxTextRunsSetTarget    string
	pptxTextRunsSetParagraph int
	pptxTextRunsSetRunIndex  int

	pptxTextRunsSetBold       bool
	pptxTextRunsSetItalic     bool
	pptxTextRunsSetUnderline  string
	pptxTextRunsSetFontSize   float64
	pptxTextRunsSetColor      string
	pptxTextRunsSetFontFamily string
	pptxTextRunsSetHyperlink  string

	pptxTextRunsRemoveBold       bool
	pptxTextRunsRemoveItalic     bool
	pptxTextRunsRemoveUnderline  bool
	pptxTextRunsRemoveFontSize   bool
	pptxTextRunsRemoveColor      bool
	pptxTextRunsRemoveFontFamily bool
	pptxTextRunsRemoveHyperlink  bool
)

type PPTXTextRunsSetResult struct {
	File        string                `json:"file"`
	Output      string                `json:"output,omitempty"`
	DryRun      bool                  `json:"dryRun"`
	Destination *PPTXShapeDestination `json:"destination,omitempty"`
	PPTXBridgeReadbackCommands
	mutate.SetRunPropertiesResult
}

var pptxTextRunsSetCmd = &cobra.Command{
	Use:   "set <file>",
	Short: "Set run-level text styling on a slide shape paragraph/run",
	Long: `Set run/paragraph-level text properties (bold, italic, underline, font size,
color, font family, hyperlink) on a targeted slide shape paragraph or run.

Target a shape via --target, a paragraph via --paragraph (0-based), and
optionally a single text run via --run-index (0-based, counting a:r runs only,
skipping line breaks/tabs/fields). When --run-index is omitted, the styling is
applied to every text run in the paragraph; sibling paragraphs and untargeted
runs are preserved.

Each property has a corresponding --remove-* flag to clear it. A set flag and
its --remove-* counterpart are mutually exclusive.

Examples:
  ooxml pptx text set deck.pptx --slide 2 --target title --paragraph 0 --bold --out out.pptx
  ooxml pptx text set deck.pptx --slide 2 --target body --paragraph 1 --run-index 0 --italic --color FF0000 --font-size 18 --out out.pptx
  ooxml pptx text set deck.pptx --slide 2 --target title --paragraph 0 --hyperlink https://example.com --in-place
  ooxml pptx text set deck.pptx --slide 2 --target title --paragraph 0 --remove-bold --dry-run`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if pptxTextRunsSetSlide < 1 {
			return InvalidArgsError("--slide must be >= 1")
		}
		if pptxTextRunsSetTarget == "" {
			return InvalidArgsError("--target is required")
		}
		if pptxTextRunsSetParagraph < 0 {
			return InvalidArgsError("--paragraph must be >= 0")
		}

		opts, hyperlink, err := buildRunMutationOptions(cmd)
		if err != nil {
			return err
		}

		var runIndex *int
		if cmd.Flags().Changed("run-index") {
			if pptxTextRunsSetRunIndex < 0 {
				return InvalidArgsError("--run-index must be >= 0")
			}
			idx := pptxTextRunsSetRunIndex
			runIndex = &idx
		}

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performPPTXTextRunsSet(filePath, runIndex, hyperlink, opts, mutOpts)
		if err != nil {
			return err
		}

		data, err := marshalWithConfig(GetGlobalConfig(cmd), result)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to marshal text set JSON: %v", err)
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeCLIOutput(cmd, data)
		}
		text := fmt.Sprintf("styled slide %d shape %d paragraph %d run(s) %v", result.Slide, result.ShapeID, result.ParagraphIndex, result.AppliedRuns)
		if result.Output != "" {
			text += fmt.Sprintf("\nOutput: %s", result.Output)
		}
		if result.Destination != nil {
			text += fmt.Sprintf("\nSelector: %s", result.Destination.PrimarySelector)
		}
		return writeCLIOutput(cmd, []byte(text))
	},
}

// buildRunMutationOptions validates flag combinations and returns the run option
// set plus an optional hyperlink URL.
func buildRunMutationOptions(cmd *cobra.Command) (*mutate.RunMutationOptions, *string, error) {
	changed := func(name string) bool { return cmd.Flags().Changed(name) }

	// Mutual exclusivity between set and remove flags.
	exclusive := [][2]string{
		{"bold", "remove-bold"},
		{"italic", "remove-italic"},
		{"underline", "remove-underline"},
		{"font-size", "remove-font-size"},
		{"color", "remove-color"},
		{"font-family", "remove-font-family"},
		{"hyperlink", "remove-hyperlink"},
	}
	for _, pair := range exclusive {
		if changed(pair[0]) && changed(pair[1]) {
			return nil, nil, InvalidArgsError(fmt.Sprintf("--%s and --%s are mutually exclusive", pair[0], pair[1]))
		}
	}

	opts := &mutate.RunMutationOptions{}
	any := false

	if changed("bold") {
		b := pptxTextRunsSetBold
		opts.Bold = &b
		any = true
	}
	if pptxTextRunsRemoveBold {
		opts.RemoveBold = true
		any = true
	}
	if changed("italic") {
		i := pptxTextRunsSetItalic
		opts.Italic = &i
		any = true
	}
	if pptxTextRunsRemoveItalic {
		opts.RemoveItalic = true
		any = true
	}
	if changed("underline") {
		u := normalizeUnderlineKind(pptxTextRunsSetUnderline)
		opts.Underline = &u
		any = true
	}
	if pptxTextRunsRemoveUnderline {
		opts.RemoveUnderline = true
		any = true
	}
	if changed("font-size") {
		sz := pptxTextRunsSetFontSize
		opts.FontSize = &sz
		any = true
	}
	if pptxTextRunsRemoveFontSize {
		opts.RemoveFontSize = true
		any = true
	}
	if changed("color") {
		c := pptxTextRunsSetColor
		opts.Color = &c
		any = true
	}
	if pptxTextRunsRemoveColor {
		opts.RemoveColor = true
		any = true
	}
	if changed("font-family") {
		f := pptxTextRunsSetFontFamily
		opts.FontFamily = &f
		any = true
	}
	if pptxTextRunsRemoveFontFamily {
		opts.RemoveFontFamily = true
		any = true
	}

	var hyperlink *string
	if changed("hyperlink") {
		h := pptxTextRunsSetHyperlink
		hyperlink = &h
		any = true
	}
	if pptxTextRunsRemoveHyperlink {
		opts.RemoveHyperlink = true
		any = true
	}

	if !any {
		return nil, nil, InvalidArgsError("no styling flags provided; specify at least one of --bold/--italic/--underline/--font-size/--color/--font-family/--hyperlink (or a --remove-* flag)")
	}
	return opts, hyperlink, nil
}

// normalizeUnderlineKind maps the common WordprocessingML-style aliases
// single/double onto the DrawingML ST_TextUnderlineType tokens sng/dbl. Any
// other value is passed through unchanged for the mutate-layer validator.
func normalizeUnderlineKind(kind string) string {
	switch kind {
	case "single":
		return "sng"
	case "double":
		return "dbl"
	default:
		return kind
	}
}

func performPPTXTextRunsSet(filePath string, runIndex *int, hyperlink *string, opts *mutate.RunMutationOptions, mutOpts *MutationOptions) (*PPTXTextRunsSetResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
	if err != nil {
		return nil, err
	}
	var result *PPTXTextRunsSetResult
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		updated, err := mutate.SetRunProperties(&mutate.SetRunPropertiesRequest{
			Package:        pkg,
			SlideNumber:    pptxTextRunsSetSlide,
			Target:         pptxTextRunsSetTarget,
			ParagraphIndex: pptxTextRunsSetParagraph,
			RunIndex:       runIndex,
			Hyperlink:      hyperlink,
			Options:        opts,
		})
		if err != nil {
			return mapPPTXTextRunsMutationError(err, pkg)
		}
		destination, err := collectPPTXShapeDestination(pkg, updated.Slide, updated.Target, destinationFile, true, false)
		if err != nil {
			return err
		}
		result = &PPTXTextRunsSetResult{
			File:                   filePath,
			Output:                 destinationFile,
			DryRun:                 mutOpts.DryRun,
			Destination:            destination,
			SetRunPropertiesResult: *updated,
		}
		result.PPTXBridgeReadbackCommands = pptxShapeMutationReadbackCommands(destination, true, false)
		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to set run properties")
	}
	return result, nil
}

// mapPPTXTextRunsMutationError maps mutate-layer errors to CLI errors, enriching
// selector-not-found errors with candidate selectors and treating paragraph/run
// index and validation errors as invalid-args.
func mapPPTXTextRunsMutationError(err error, pkg opc.PackageSession) error {
	if err == nil {
		return nil
	}
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	msg := err.Error()
	switch {
	case strings.Contains(msg, "out of range"),
		strings.Contains(msg, "not found in paragraph"),
		strings.Contains(msg, "no text runs"),
		strings.Contains(msg, "invalid underline"),
		strings.Contains(msg, "invalid font size"),
		strings.Contains(msg, "invalid color"),
		strings.Contains(msg, "non-text"),
		strings.Contains(msg, "must be >="),
		strings.Contains(msg, "has no text body"):
		return InvalidArgsError(msg)
	case strings.Contains(msg, "target not found"):
		if catalog, cerr := pptselectors.BuildSlideCatalog(pkg, pptxTextRunsSetSlide); cerr == nil {
			return mapPPTXShapeResolveError(err, catalog, pptxTextRunsSetTarget, pptxTextRunsSetSlide)
		}
		return TargetNotFoundError(msg)
	case strings.Contains(msg, "ambiguous target"):
		return InvalidArgsError(msg)
	case strings.Contains(msg, "not found (presentation has"), strings.Contains(msg, "slide must be"):
		return InvalidArgsError(msg)
	default:
		return NewCLIErrorf(ExitUnexpected, "%v", err)
	}
}

func init() {
	pptxTextRunsSetCmd.Flags().IntVar(&pptxTextRunsSetSlide, "slide", 0, "1-based slide number")
	pptxTextRunsSetCmd.Flags().StringVar(&pptxTextRunsSetTarget, "target", "", "shape selector such as title, body, shape:3, or ~Shape Name")
	pptxTextRunsSetCmd.Flags().IntVar(&pptxTextRunsSetParagraph, "paragraph", 0, "0-based paragraph index within the shape")
	pptxTextRunsSetCmd.Flags().IntVar(&pptxTextRunsSetRunIndex, "run-index", 0, "0-based text run index within the paragraph (omit to apply to all runs)")

	pptxTextRunsSetCmd.Flags().BoolVar(&pptxTextRunsSetBold, "bold", false, "set bold on/off")
	pptxTextRunsSetCmd.Flags().BoolVar(&pptxTextRunsSetItalic, "italic", false, "set italic on/off")
	pptxTextRunsSetCmd.Flags().StringVar(&pptxTextRunsSetUnderline, "underline", "", "underline kind (sng, dbl, heavy, dotted, dash, wavy, none, ...; aliases single->sng, double->dbl)")
	pptxTextRunsSetCmd.Flags().Float64Var(&pptxTextRunsSetFontSize, "font-size", 0, "font size in points (e.g. 24)")
	pptxTextRunsSetCmd.Flags().StringVar(&pptxTextRunsSetColor, "color", "", "RGB hex color (6 hex digits, e.g. FF0000)")
	pptxTextRunsSetCmd.Flags().StringVar(&pptxTextRunsSetFontFamily, "font-family", "", "latin font family typeface (e.g. Arial)")
	pptxTextRunsSetCmd.Flags().StringVar(&pptxTextRunsSetHyperlink, "hyperlink", "", "external hyperlink URL (a:hlinkClick)")

	pptxTextRunsSetCmd.Flags().BoolVar(&pptxTextRunsRemoveBold, "remove-bold", false, "remove the bold attribute")
	pptxTextRunsSetCmd.Flags().BoolVar(&pptxTextRunsRemoveItalic, "remove-italic", false, "remove the italic attribute")
	pptxTextRunsSetCmd.Flags().BoolVar(&pptxTextRunsRemoveUnderline, "remove-underline", false, "remove the underline attribute")
	pptxTextRunsSetCmd.Flags().BoolVar(&pptxTextRunsRemoveFontSize, "remove-font-size", false, "remove the font size attribute")
	pptxTextRunsSetCmd.Flags().BoolVar(&pptxTextRunsRemoveColor, "remove-color", false, "remove the solid fill color")
	pptxTextRunsSetCmd.Flags().BoolVar(&pptxTextRunsRemoveFontFamily, "remove-font-family", false, "remove the latin font family")
	pptxTextRunsSetCmd.Flags().BoolVar(&pptxTextRunsRemoveHyperlink, "remove-hyperlink", false, "remove the hyperlink (a:hlinkClick)")

	pptxTextRunsSetCmd.MarkFlagRequired("slide")
	pptxTextRunsSetCmd.MarkFlagRequired("target")
	AddMutationFlags(pptxTextRunsSetCmd)
	pptxTextCmd.AddCommand(pptxTextRunsSetCmd)
}
