package cli

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxhandle "github.com/ooxml-cli/ooxml-cli/pkg/pptx/handle"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	pptxmutate "github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
	"github.com/spf13/cobra"
)

// pptxAnimationsListReadbackCommand builds the JSON readback follow-up command.
func pptxAnimationsListReadbackCommand(filePath string) string {
	return fmt.Sprintf("ooxml --json pptx animations list %s", pptxXLSXCommandArg(filePath))
}

func pptxAnimationsReadbackCommands(destinationFile string) PPTXBridgeReadbackCommands {
	if destinationFile == "" {
		placeholder := outputPlaceholder()
		return PPTXBridgeReadbackCommands{
			ReadbackCommandTemplate: pptxAnimationsListReadbackCommand(placeholder),
			ValidateCommandTemplate: pptxValidateCommand(placeholder),
			RenderCommandTemplate:   pptxRenderCommand(placeholder),
		}
	}
	return PPTXBridgeReadbackCommands{
		ReadbackCommand: pptxAnimationsListReadbackCommand(destinationFile),
		ValidateCommand: pptxValidateCommand(destinationFile),
		RenderCommand:   pptxRenderCommand(destinationFile),
	}
}

// resolveAnimSlide validates the 1-based slide number and returns its SlideRef.
func resolveAnimSlide(session opc.PackageSession, slide int) (*inspect.SlideRef, error) {
	if slide < 1 {
		return nil, InvalidArgsError("--slide must be >= 1")
	}
	graph, err := inspect.ParsePresentation(session)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", err)
	}
	if slide > len(graph.Slides) {
		return nil, NewCLIErrorf(ExitTargetNotFound, "slide %d not found (presentation has %d slides)", slide, len(graph.Slides))
	}
	ref := graph.Slides[slide-1]
	return &ref, nil
}

// resolveAnimSlideByHandle resolves the slide scope of a shape handle by
// SEARCHING for its native sldId, so the addressed slide survives reorder /
// insert / delete of other slides. --slide is intentionally ignored here.
func resolveAnimSlideByHandle(session opc.PackageSession, h pptxhandle.Handle) (*inspect.SlideRef, error) {
	graph, err := inspect.ParsePresentation(session)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", err)
	}
	// Route through the shared scope resolver so a duplicate sldId errors
	// CodeAmbiguous instead of silently first-winning.
	ref, rerr := selectors.ResolveSlideRefForHandle(graph, h)
	if rerr != nil {
		return nil, mapPPTXHandleError(rerr)
	}
	out := *ref
	return &out, nil
}

func outputAnimationsJSON(cmd *cobra.Command, result any) error {
	config := GetGlobalConfig(cmd)
	var (
		data []byte
		err  error
	)
	if config.Pretty {
		data, err = json.MarshalIndent(result, "", "  ")
	} else {
		data, err = json.Marshal(result)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

// ---------------------------------------------------------------------------
// add
// ---------------------------------------------------------------------------

type PPTXAnimationsAddResult struct {
	File                       string `json:"file"`
	Output                     string `json:"output,omitempty"`
	DryRun                     bool   `json:"dryRun"`
	Action                     string `json:"action"`
	Slide                      int    `json:"slide"`
	ShapeID                    int    `json:"shapeId"`
	ShapeName                  string `json:"shapeName"`
	Effect                     string `json:"effect"`
	Start                      string `json:"start"`
	AddedEffectIDs             []int  `json:"addedEffectIds"`
	ClickStepID                int    `json:"clickStepId"`
	CreatedTiming              bool   `json:"createdTiming"`
	ByParagraph                bool   `json:"byParagraph"`
	ParagraphCount             int    `json:"paragraphCount,omitempty"`
	RenderUnconfirmed          bool   `json:"renderUnconfirmed"`
	PPTXBridgeReadbackCommands `json:",inline"`
}

var (
	pptxAnimAddSlide          int
	pptxAnimAddShape          string
	pptxAnimAddEffect         string
	pptxAnimAddDirection      string
	pptxAnimAddDurationMs     int
	pptxAnimAddStart          string
	pptxAnimAddByParagraph    bool
	pptxAnimAddParagraphRange string
	pptxAnimAddExpectShape    string
	pptxAnimAddExpectParaN    int
)

var pptxAnimationsAddCmd = &cobra.Command{
	Use:   "add <file>",
	Short: "Add an entrance animation (appear, fade, wipe, fly-in) to a shape",
	Long: `Author an entrance effect on a shape's p:timing tree.

The effect's behavior elements (p:set / p:animEffect / p:anim) are what PowerPoint
renders; presetID/presetClass/presetSubtype are advisory animation-pane metadata.
appear and wipe(up) tokens are confirmed against real PowerPoint output; fade and
fly-in are spec-grounded (ECMA-376) but render-unconfirmed.

The timing skeleton is created in schema order only when absent; an existing
timing tree (including unsupported motion-path / emphasis nodes) is preserved and
appended to, never rebuilt.

Examples:
  ooxml pptx animations add deck.pptx --slide 1 --shape shape:4 --effect appear
  ooxml pptx animations add deck.pptx --slide 2 --shape ~Title --effect wipe --direction up
  ooxml pptx animations add deck.pptx --slide 3 --shape ~Body --effect fade --by-paragraph
  ooxml pptx animations add deck.pptx --shape H:pptx/s:257/shape:n:2 --effect appear`,
	Args: cobra.ExactArgs(1),
	RunE: runPPTXAnimationsAdd,
}

func runPPTXAnimationsAdd(cmd *cobra.Command, args []string) error {
	filePath := args[0]
	if _, err := os.Stat(filePath); err != nil {
		return FileNotFoundError(filePath)
	}
	if strings.TrimSpace(pptxAnimAddShape) == "" {
		return InvalidArgsError("--shape is required (e.g. shape:4 or ~Title)")
	}
	// --shape additionally accepts a stable shape handle
	// (H:pptx/s:<sldId>/shape:n:<id>). When supplied, the handle's sldId is
	// authoritative for slide scope and --slide is ignored for resolution.
	var (
		sel       selectors.Selector
		shapeHnd  pptxhandle.Handle
		useHandle bool
	)
	if pptxhandle.IsHandle(pptxAnimAddShape) {
		h, herr := pptxhandle.Parse(pptxAnimAddShape)
		if herr != nil {
			return mapPPTXHandleError(herr)
		}
		shapeHnd = h
		useHandle = true
		// The handle's native cNvPr id becomes a shape-id selector, which the
		// downstream resolver searches for within the resolved slide.
		sel = &selectors.ShapeIDSelector{ID: h.ShapeID}
	} else {
		var perr error
		sel, perr = selectors.Parse(pptxAnimAddShape)
		if perr != nil {
			return NewCLIErrorf(ExitInvalidArgs, "invalid --shape selector: %v", perr)
		}
	}
	if pptxAnimAddByParagraph && strings.TrimSpace(pptxAnimAddParagraphRange) != "" {
		return InvalidArgsError("--by-paragraph and --paragraph-range are mutually exclusive")
	}
	var paraRange *inspect.ParaRange
	if r := strings.TrimSpace(pptxAnimAddParagraphRange); r != "" {
		pr, perr := parseParagraphRange(r)
		if perr != nil {
			return perr
		}
		paraRange = pr
	}
	// A positive --expect-paragraph-count activates the guard; 0 is "unset" (a
	// by-paragraph build requires >=1 paragraph, so 0 is never a real expectation).
	var expectParaN *int
	if pptxAnimAddExpectParaN > 0 {
		n := pptxAnimAddExpectParaN
		expectParaN = &n
	}

	mutOpts, err := GetValidatedMutationOptions(cmd)
	if err != nil {
		return err
	}
	wantReadback := GetGlobalConfig(cmd).Format == "json"

	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
	if err != nil {
		return err
	}

	var result *PPTXAnimationsAddResult
	if err := writer.Write(func(session opc.PackageSession) error {
		var slideRef *inspect.SlideRef
		var err error
		if useHandle {
			slideRef, err = resolveAnimSlideByHandle(session, shapeHnd)
			if err == nil {
				// Validate the handle's shape component through the shared
				// resolver before falling back to the mutator's shape-id
				// selector. This preserves HANDLE_AMBIGUOUS/HANDLE_STALE
				// semantics for duplicate or missing cNvPr ids.
				_, _, _, err = selectors.ResolvePPTXShapeHandle(session, pptxhandle.Format(shapeHnd))
				if err != nil {
					err = mapPPTXHandleError(err)
				}
			}
		} else {
			slideRef, err = resolveAnimSlide(session, pptxAnimAddSlide)
		}
		if err != nil {
			return err
		}
		addResult, err := pptxmutate.AddAnimation(&pptxmutate.AddAnimationRequest{
			Package:              session,
			SlideRef:             slideRef,
			Selector:             sel,
			Effect:               pptxAnimAddEffect,
			Direction:            pptxAnimAddDirection,
			DurationMs:           pptxAnimAddDurationMs,
			Start:                pptxAnimAddStart,
			ByParagraph:          pptxAnimAddByParagraph,
			ParagraphRange:       paraRange,
			ExpectShapeName:      pptxAnimAddExpectShape,
			ExpectParagraphCount: expectParaN,
		})
		if err != nil {
			// When a handle was used and the shape it names is absent on its live
			// slide, surface the same typed HANDLE_STALE contract as the
			// image-replace path rather than a generic search-miss error.
			if useHandle && isAnimAddSearchMiss(err) {
				return mapPPTXHandleError(&pptxhandle.Error{
					Code:    pptxhandle.CodeStale,
					Handle:  pptxhandle.Format(shapeHnd),
					Message: fmt.Sprintf("shape cNvPr id %d not found on slide sldId %d", shapeHnd.ShapeID, shapeHnd.SlideID),
				})
			}
			return mapAnimMutateError(err)
		}

		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		result = &PPTXAnimationsAddResult{
			File:              filePath,
			Output:            destinationFile,
			DryRun:            mutOpts != nil && mutOpts.DryRun,
			Action:            "pptx.animations.add",
			Slide:             slideRef.SlideNumber,
			ShapeID:           addResult.ShapeID,
			ShapeName:         addResult.ShapeName,
			Effect:            addResult.Effect,
			Start:             addResult.Start,
			AddedEffectIDs:    addResult.AddedEffectIDs,
			ClickStepID:       addResult.ClickStepID,
			CreatedTiming:     addResult.CreatedTiming,
			ByParagraph:       addResult.ByParagraph,
			ParagraphCount:    addResult.ParagraphCount,
			RenderUnconfirmed: addResult.Effect == "fade" || addResult.Effect == "flyIn",
		}
		result.PPTXBridgeReadbackCommands = pptxAnimationsReadbackCommands(destinationFile)
		_ = wantReadback
		return nil
	}); err != nil {
		return err
	}

	if GetGlobalConfig(cmd).Format == "json" {
		return outputAnimationsJSON(cmd, result)
	}
	caveat := ""
	if result.RenderUnconfirmed {
		caveat = " (render-unconfirmed token)"
	}
	return writeCLIOutput(cmd, []byte(fmt.Sprintf("added %s entrance on slide %d shape %d (effect ids %s)%s",
		result.Effect, result.Slide, result.ShapeID, joinIntList(result.AddedEffectIDs), caveat)))
}

// ---------------------------------------------------------------------------
// remove
// ---------------------------------------------------------------------------

type PPTXAnimationsRemoveResult struct {
	File                       string `json:"file"`
	Output                     string `json:"output,omitempty"`
	DryRun                     bool   `json:"dryRun"`
	Action                     string `json:"action"`
	Slide                      int    `json:"slide"`
	RemovedEffectID            int    `json:"removedEffectId"`
	RemovedClickStep           bool   `json:"removedClickStep"`
	ShapeID                    int    `json:"shapeId"`
	ShapeName                  string `json:"shapeName"`
	PPTXBridgeReadbackCommands `json:",inline"`
}

var (
	pptxAnimRemoveSlide       int
	pptxAnimRemoveEffectID    int
	pptxAnimRemoveExpectShape string
)

var pptxAnimationsRemoveCmd = &cobra.Command{
	Use:   "remove <file>",
	Short: "Remove an entrance animation by its effect id",
	Long: `Remove the entrance effect with the given --effect-id (from animations list).

Only supported entrance effects can be removed; an id resolving to a preserved or
unsupported node (motion path, emphasis, exit, media trigger) is refused so that
XML is never deleted by id collision. When the enclosing click step becomes empty
it is collapsed; all sibling effects and unknown nodes are preserved.

Examples:
  ooxml pptx animations remove deck.pptx --slide 1 --effect-id 5`,
	Args: cobra.ExactArgs(1),
	RunE: runPPTXAnimationsRemove,
}

func runPPTXAnimationsRemove(cmd *cobra.Command, args []string) error {
	filePath := args[0]
	if _, err := os.Stat(filePath); err != nil {
		return FileNotFoundError(filePath)
	}
	if pptxAnimRemoveEffectID <= 0 {
		return InvalidArgsError("--effect-id is required and must be > 0")
	}

	mutOpts, err := GetValidatedMutationOptions(cmd)
	if err != nil {
		return err
	}
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
	if err != nil {
		return err
	}

	var result *PPTXAnimationsRemoveResult
	if err := writer.Write(func(session opc.PackageSession) error {
		slideRef, err := resolveAnimSlide(session, pptxAnimRemoveSlide)
		if err != nil {
			return err
		}
		rm, err := pptxmutate.RemoveAnimation(&pptxmutate.RemoveAnimationRequest{
			Package:         session,
			SlideRef:        slideRef,
			EffectID:        pptxAnimRemoveEffectID,
			ExpectShapeName: pptxAnimRemoveExpectShape,
		})
		if err != nil {
			if isAnimTargetNotFound(err) {
				return pptxAnimationEffectNotFoundError(session, pptxAnimRemoveSlide, pptxAnimRemoveEffectID)
			}
			return mapAnimMutateError(err)
		}
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		result = &PPTXAnimationsRemoveResult{
			File:             filePath,
			Output:           destinationFile,
			DryRun:           mutOpts != nil && mutOpts.DryRun,
			Action:           "pptx.animations.remove",
			Slide:            pptxAnimRemoveSlide,
			RemovedEffectID:  rm.RemovedEffectID,
			RemovedClickStep: rm.RemovedClickStep,
			ShapeID:          rm.ShapeID,
			ShapeName:        rm.ShapeName,
		}
		result.PPTXBridgeReadbackCommands = pptxAnimationsReadbackCommands(destinationFile)
		return nil
	}); err != nil {
		return err
	}

	if GetGlobalConfig(cmd).Format == "json" {
		return outputAnimationsJSON(cmd, result)
	}
	return writeCLIOutput(cmd, []byte(fmt.Sprintf("removed effect %d on slide %d", result.RemovedEffectID, result.Slide)))
}

// ---------------------------------------------------------------------------
// reorder
// ---------------------------------------------------------------------------

type PPTXAnimationsReorderResult struct {
	File                       string `json:"file"`
	Output                     string `json:"output,omitempty"`
	DryRun                     bool   `json:"dryRun"`
	Action                     string `json:"action"`
	Slide                      int    `json:"slide"`
	Order                      []int  `json:"order"`
	ClickStepCount             int    `json:"clickStepCount"`
	PPTXBridgeReadbackCommands `json:",inline"`
}

var (
	pptxAnimReorderSlide int
	pptxAnimReorderOrder string
)

var pptxAnimationsReorderCmd = &cobra.Command{
	Use:   "reorder <file>",
	Short: "Reorder the per-click animation steps of a slide",
	Long: `Reorder the top-level click steps of a slide's main sequence.

--order is a comma-separated permutation of the clickEffect cTn ids reported by
animations list. withEffect/afterEffect children move with their parent click
step; unknown sibling nodes are preserved after the reordered set.

Examples:
  ooxml pptx animations reorder deck.pptx --slide 1 --order 7,3,5`,
	Args: cobra.ExactArgs(1),
	RunE: runPPTXAnimationsReorder,
}

func runPPTXAnimationsReorder(cmd *cobra.Command, args []string) error {
	filePath := args[0]
	if _, err := os.Stat(filePath); err != nil {
		return FileNotFoundError(filePath)
	}
	order, err := parseIntList(pptxAnimReorderOrder)
	if err != nil {
		return err
	}
	if len(order) == 0 {
		return InvalidArgsError("--order is required (comma-separated clickEffect ids)")
	}

	mutOpts, err := GetValidatedMutationOptions(cmd)
	if err != nil {
		return err
	}
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
	if err != nil {
		return err
	}

	var result *PPTXAnimationsReorderResult
	if err := writer.Write(func(session opc.PackageSession) error {
		slideRef, err := resolveAnimSlide(session, pptxAnimReorderSlide)
		if err != nil {
			return err
		}
		ro, err := pptxmutate.ReorderAnimations(&pptxmutate.ReorderAnimationsRequest{
			Package:  session,
			SlideRef: slideRef,
			Order:    order,
		})
		if err != nil {
			return mapAnimMutateError(err)
		}
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		result = &PPTXAnimationsReorderResult{
			File:           filePath,
			Output:         destinationFile,
			DryRun:         mutOpts != nil && mutOpts.DryRun,
			Action:         "pptx.animations.reorder",
			Slide:          pptxAnimReorderSlide,
			Order:          ro.Order,
			ClickStepCount: ro.ClickStepCount,
		}
		result.PPTXBridgeReadbackCommands = pptxAnimationsReadbackCommands(destinationFile)
		return nil
	}); err != nil {
		return err
	}

	if GetGlobalConfig(cmd).Format == "json" {
		return outputAnimationsJSON(cmd, result)
	}
	return writeCLIOutput(cmd, []byte(fmt.Sprintf("reordered %d click steps on slide %d", result.ClickStepCount, result.Slide)))
}

// ---------------------------------------------------------------------------
// prune-stale
// ---------------------------------------------------------------------------

type PPTXAnimationsPruneResult struct {
	File                       string                  `json:"file"`
	Output                     string                  `json:"output,omitempty"`
	DryRun                     bool                    `json:"dryRun"`
	Action                     string                  `json:"action"`
	Slide                      int                     `json:"slide"`
	Pruned                     []pptxmutate.PrunedNode `json:"pruned"`
	PrunedCount                int                     `json:"prunedCount"`
	PPTXBridgeReadbackCommands `json:",inline"`
}

var pptxAnimPruneSlide int

var pptxAnimationsPruneCmd = &cobra.Command{
	Use:   "prune-stale <file>",
	Short: "Remove animation effects/builds whose targets no longer exist",
	Long: `Remove only the effect and build nodes flagged stale by animations list.

Stale means the target shape was deleted (missing-shape) or a paragraph range
points past the shape's paragraph count (pRg-out-of-range). Non-stale and
unsupported-but-valid nodes are never touched; stale media is left to the media
tooling. Use --dry-run first to preview.

Examples:
  ooxml pptx animations prune-stale deck.pptx --dry-run
  ooxml pptx animations prune-stale deck.pptx --slide 4 --in-place`,
	Args: cobra.ExactArgs(1),
	RunE: runPPTXAnimationsPrune,
}

func runPPTXAnimationsPrune(cmd *cobra.Command, args []string) error {
	filePath := args[0]
	if _, err := os.Stat(filePath); err != nil {
		return FileNotFoundError(filePath)
	}

	mutOpts, err := GetValidatedMutationOptions(cmd)
	if err != nil {
		return err
	}
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
	if err != nil {
		return err
	}

	var result *PPTXAnimationsPruneResult
	if err := writer.Write(func(session opc.PackageSession) error {
		var slideRefs []inspect.SlideRef
		graph, gerr := inspect.ParsePresentation(session)
		if gerr != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", gerr)
		}
		if pptxAnimPruneSlide > 0 {
			if pptxAnimPruneSlide > len(graph.Slides) {
				return NewCLIErrorf(ExitTargetNotFound, "slide %d not found (presentation has %d slides)", pptxAnimPruneSlide, len(graph.Slides))
			}
			slideRefs = []inspect.SlideRef{graph.Slides[pptxAnimPruneSlide-1]}
		}
		pr, err := pptxmutate.PruneStale(&pptxmutate.PruneStaleRequest{
			Package:   session,
			SlideRefs: slideRefs,
			DryRun:    mutOpts != nil && mutOpts.DryRun,
		})
		if err != nil {
			return mapAnimMutateError(err)
		}
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		result = &PPTXAnimationsPruneResult{
			File:        filePath,
			Output:      destinationFile,
			DryRun:      mutOpts != nil && mutOpts.DryRun,
			Action:      "pptx.animations.prune-stale",
			Slide:       pptxAnimPruneSlide,
			Pruned:      pr.Pruned,
			PrunedCount: len(pr.Pruned),
		}
		result.PPTXBridgeReadbackCommands = pptxAnimationsReadbackCommands(destinationFile)
		return nil
	}); err != nil {
		return err
	}

	if GetGlobalConfig(cmd).Format == "json" {
		return outputAnimationsJSON(cmd, result)
	}
	verb := "pruned"
	if result.DryRun {
		verb = "would prune"
	}
	return writeCLIOutput(cmd, []byte(fmt.Sprintf("%s %d stale animation node(s)", verb, result.PrunedCount)))
}

// ---------------------------------------------------------------------------
// shared helpers
// ---------------------------------------------------------------------------

// mapAnimMutateError maps mutator errors to CLI exit codes. A TargetNotFoundError
// becomes ExitTargetNotFound; everything else is ExitInvalidArgs (the mutators
// validate inputs).
func mapAnimMutateError(err error) error {
	var tnf *pptxmutate.TargetNotFoundError
	if errors.As(err, &tnf) {
		return NewCLIErrorf(ExitTargetNotFound, "%v", err)
	}
	return NewCLIErrorf(ExitInvalidArgs, "%v", err)
}

func isAnimTargetNotFound(err error) bool {
	var tnf *pptxmutate.TargetNotFoundError
	return errors.As(err, &tnf) && strings.Contains(strings.ToLower(err.Error()), "not found")
}

func pptxAnimationEffectNotFoundError(session opc.PackageSession, slideNumber, effectID int) error {
	candidates := pptxAnimationEffectSelectorCandidates(session, slideNumber)
	selector := "effect:" + strconv.Itoa(effectID)
	return SelectorNotFoundError("animation effect", selector, BuildSelectorCandidates(candidates, selector, maxSelectorCandidates), "ooxml --json pptx animations list <file>")
}

func pptxAnimationEffectSelectorCandidates(session opc.PackageSession, slideNumber int) []SelectorCandidate {
	report, err := inspect.ReadAnimations(session)
	if err != nil || report == nil {
		return nil
	}
	var out []SelectorCandidate
	for _, slide := range report.Slides {
		if slide.Slide != slideNumber {
			continue
		}
		for _, effect := range slide.Effects {
			out = append(out, SelectorCandidate{Primary: effect.PrimarySelector, Selectors: effect.Selectors})
		}
		break
	}
	return out
}

// isAnimAddSearchMiss reports whether an AddAnimation error is a shape
// search-miss (the addressed cNvPr id is absent on the live slide), detected the
// same way the image-replace path detects its miss.
func isAnimAddSearchMiss(err error) bool {
	if err == nil {
		return false
	}
	return strings.Contains(err.Error(), "not found on slide")
}

// parseParagraphRange parses an "A:B" 0-based inclusive paragraph range.
func parseParagraphRange(s string) (*inspect.ParaRange, error) {
	parts := strings.SplitN(s, ":", 2)
	if len(parts) != 2 {
		return nil, InvalidArgsError("--paragraph-range must be in the form A:B (0-based inclusive)")
	}
	start, err := strconv.Atoi(strings.TrimSpace(parts[0]))
	if err != nil || start < 0 {
		return nil, InvalidArgsError("--paragraph-range start must be a non-negative integer")
	}
	end, err := strconv.Atoi(strings.TrimSpace(parts[1]))
	if err != nil || end < start {
		return nil, InvalidArgsError("--paragraph-range end must be an integer >= start")
	}
	return &inspect.ParaRange{Start: start, End: end}, nil
}

// parseIntList parses a comma-separated integer list (StringVar + split, per repo
// convention; NOT pflag StringSliceVar).
func parseIntList(s string) ([]int, error) {
	s = strings.TrimSpace(s)
	if s == "" {
		return nil, nil
	}
	var out []int
	for _, tok := range splitCommaList(s) {
		n, err := strconv.Atoi(strings.TrimSpace(tok))
		if err != nil {
			return nil, NewCLIErrorf(ExitInvalidArgs, "invalid id %q in list", tok)
		}
		out = append(out, n)
	}
	return out, nil
}

func joinIntList(ids []int) string {
	parts := make([]string, len(ids))
	for i, id := range ids {
		parts[i] = strconv.Itoa(id)
	}
	return strings.Join(parts, ",")
}

func init() {
	// add
	af := pptxAnimationsAddCmd.Flags()
	af.IntVar(&pptxAnimAddSlide, "slide", 0, "1-based slide number (required unless --shape is a stable shape handle)")
	af.StringVar(&pptxAnimAddShape, "shape", "", "target shape selector or stable shape handle: shape:<id>, ~<name>, or H:pptx/s:<sldId>/shape:n:<id> (required)")
	af.StringVar(&pptxAnimAddEffect, "effect", "", "entrance effect: appear, fade, wipe, or fly-in (required)")
	af.StringVar(&pptxAnimAddDirection, "direction", "up", "direction for wipe/fly-in: up, down, left, or right")
	af.IntVar(&pptxAnimAddDurationMs, "duration-ms", 500, "effect duration in milliseconds")
	af.StringVar(&pptxAnimAddStart, "start", "onClick", "start trigger: onClick, withPrevious, or afterPrevious")
	af.BoolVar(&pptxAnimAddByParagraph, "by-paragraph", false, "fan out one entrance per paragraph and add a by-paragraph build (text shapes)")
	af.StringVar(&pptxAnimAddParagraphRange, "paragraph-range", "", "scope a single effect to a 0-based inclusive paragraph range A:B")
	af.StringVar(&pptxAnimAddExpectShape, "expect-shape-name", "", "stale guard: require the resolved shape name to match")
	af.IntVar(&pptxAnimAddExpectParaN, "expect-paragraph-count", 0, "stale guard for --by-paragraph: require the paragraph count to match")
	AddMutationFlags(pptxAnimationsAddCmd)
	pptxAnimationsCmd.AddCommand(pptxAnimationsAddCmd)

	// remove
	rf := pptxAnimationsRemoveCmd.Flags()
	rf.IntVar(&pptxAnimRemoveSlide, "slide", 0, "1-based slide number (required)")
	rf.IntVar(&pptxAnimRemoveEffectID, "effect-id", 0, "the cTn id of the effect to remove (from animations list) (required)")
	rf.StringVar(&pptxAnimRemoveExpectShape, "expect-shape-name", "", "stale guard: require the effect's target shape name to match")
	AddMutationFlags(pptxAnimationsRemoveCmd)
	pptxAnimationsCmd.AddCommand(pptxAnimationsRemoveCmd)

	// reorder
	rof := pptxAnimationsReorderCmd.Flags()
	rof.IntVar(&pptxAnimReorderSlide, "slide", 0, "1-based slide number (required)")
	rof.StringVar(&pptxAnimReorderOrder, "order", "", "comma-separated clickEffect ids in the new playback order (required)")
	AddMutationFlags(pptxAnimationsReorderCmd)
	pptxAnimationsCmd.AddCommand(pptxAnimationsReorderCmd)

	// prune-stale
	pf := pptxAnimationsPruneCmd.Flags()
	pf.IntVar(&pptxAnimPruneSlide, "slide", 0, "1-based slide number (0 = all slides)")
	AddMutationFlags(pptxAnimationsPruneCmd)
	pptxAnimationsCmd.AddCommand(pptxAnimationsPruneCmd)
}
