package mutate

import (
	"fmt"
	"sort"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	ns "github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
)

// animations.go authors and edits the four in-scope entrance effects (appear,
// fade, wipe, fly-in) plus per-paragraph builds in a slide's p:timing tree.
//
// CARDINAL RULE (matching ReplaceImage / InsertImage idioms): we NEVER rebuild an
// existing p:timing tree. We read the slide, get-or-create only the missing
// skeleton nodes, append/remove only the nodes we own by document-order surgery,
// and ReplaceXMLPart re-serializes every unrelated/unsupported node verbatim. The
// classifier that decides what we "own" keys on (presetClass==entr + behavior
// pattern); anything else (motion paths, emphasis, exit, media triggers) is
// preserved and never authored, deleted-by-id, or reordered out of existence.

// ---------------------------------------------------------------------------
// Request / result types (mirroring InsertImageRequest / SetFieldsResult).
// ---------------------------------------------------------------------------

// AddAnimationRequest authors one entrance effect (or a per-paragraph fan-out) on
// a shape.
type AddAnimationRequest struct {
	Package        opc.PackageSession
	SlideRef       *inspect.SlideRef
	Selector       selectors.Selector
	Effect         string // appear|fade|wipe|flyIn
	Direction      string // up|down|left|right (wipe/flyIn)
	DurationMs     int
	Start          string // onClick|withPrevious|afterPrevious
	ByParagraph    bool
	ParagraphRange *inspect.ParaRange // optional single-range scope (0-based inclusive)

	ExpectShapeName      string
	ExpectParagraphCount *int
}

// AddAnimationResult reports the authored effect(s).
type AddAnimationResult struct {
	ShapeID        int    `json:"shapeId"`
	ShapeName      string `json:"shapeName"`
	Effect         string `json:"effect"`
	Start          string `json:"start"`
	AddedEffectIDs []int  `json:"addedEffectIds"` // one per paragraph when by-paragraph
	ClickStepID    int    `json:"clickStepId"`
	CreatedTiming  bool   `json:"createdTiming"`
	ByParagraph    bool   `json:"byParagraph"`
	ParagraphCount int    `json:"paragraphCount,omitempty"`
}

// RemoveAnimationRequest removes one effect by its cTn id.
type RemoveAnimationRequest struct {
	Package         opc.PackageSession
	SlideRef        *inspect.SlideRef
	EffectID        int
	ExpectShapeName string
}

// RemoveAnimationResult reports the removed effect.
type RemoveAnimationResult struct {
	RemovedEffectID  int    `json:"removedEffectId"`
	RemovedClickStep bool   `json:"removedClickStep"`
	ShapeID          int    `json:"shapeId"`
	ShapeName        string `json:"shapeName"`
}

// ReorderAnimationsRequest reorders the per-click steps of the mainSeq.
type ReorderAnimationsRequest struct {
	Package  opc.PackageSession
	SlideRef *inspect.SlideRef
	Order    []int // clickEffect cTn ids in new playback order (a permutation)
}

// ReorderAnimationsResult reports the new click-step order.
type ReorderAnimationsResult struct {
	Order          []int `json:"order"`
	ClickStepCount int   `json:"clickStepCount"`
}

// PruneStaleRequest prunes stale effects/builds across one or more slides.
type PruneStaleRequest struct {
	Package   opc.PackageSession
	SlideRefs []inspect.SlideRef
	DryRun    bool
}

// PrunedNode describes one removed (or would-be-removed) stale node.
type PrunedNode struct {
	Slide       int    `json:"slide"`
	Kind        string `json:"kind"` // effect|build
	EffectID    int    `json:"effectId,omitempty"`
	Spid        int    `json:"spid"`
	StaleReason string `json:"staleReason"`
}

// PruneStaleResult reports the pruned (or candidate) stale nodes.
type PruneStaleResult struct {
	Pruned []PrunedNode `json:"pruned"`
}

// ---------------------------------------------------------------------------
// AddAnimation
// ---------------------------------------------------------------------------

// AddAnimation authors an entrance effect on the selected shape, creating the
// p:timing skeleton if absent and never rebuilding an existing tree.
func AddAnimation(req *AddAnimationRequest) (*AddAnimationResult, error) {
	if req == nil || req.Package == nil {
		return nil, fmt.Errorf("add animation request/package is nil")
	}
	if req.SlideRef == nil {
		return nil, fmt.Errorf("slide reference is nil")
	}
	effect := normalizeEffect(req.Effect)
	if effect == "" {
		return nil, fmt.Errorf("unknown effect %q (expected appear|fade|wipe|flyIn)", req.Effect)
	}
	start := normalizeStart(req.Start)
	if start == "" {
		return nil, fmt.Errorf("unknown start %q (expected onClick|withPrevious|afterPrevious)", req.Start)
	}
	direction := req.Direction
	if direction == "" {
		direction = "up"
	}
	if effect == "wipe" || effect == "flyIn" {
		if _, ok := wipeFilterByDirection[direction]; !ok {
			return nil, fmt.Errorf("unknown direction %q (expected up|down|left|right)", direction)
		}
	}
	durationMs := req.DurationMs
	if durationMs <= 0 {
		durationMs = 500
	}

	doc, err := req.Package.ReadXMLPart(req.SlideRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read slide: %w", err)
	}
	root := doc.Root()
	spTree := findSlideSpTree(root)
	if spTree == nil {
		return nil, fmt.Errorf("shape tree not found in slide")
	}

	shape, spid, name, err := resolveShapeInTree(spTree, req.Selector)
	if err != nil {
		return nil, err
	}
	if req.ExpectShapeName != "" && req.ExpectShapeName != name {
		return nil, fmt.Errorf("shape name guard failed: expected %q but resolved %q", req.ExpectShapeName, name)
	}

	result := &AddAnimationResult{
		ShapeID:     spid,
		ShapeName:   name,
		Effect:      effect,
		Start:       start,
		ByParagraph: req.ByParagraph,
	}

	timing, created := getOrCreateTiming(root)
	result.CreatedTiming = created
	mainSeqCTn := getOrCreateMainSeq(timing)
	mainSeqChildTnLst := childTnLstOf(mainSeqCTn)

	// Determine the paragraph ranges to author. By-paragraph fans out one effect
	// per paragraph; a single --paragraph-range scopes one effect; otherwise the
	// whole shape.
	var ranges []*inspect.ParaRange
	if req.ByParagraph {
		txBody := xmlx.FindChild(shape, ns.NsP, "txBody")
		if txBody == nil {
			return nil, fmt.Errorf("shape %q has no text body; --by-paragraph requires a text shape", name)
		}
		paraCount := len(xmlx.FindChildren(txBody, ns.NsA, "p"))
		if paraCount == 0 {
			return nil, fmt.Errorf("shape %q has no paragraphs to build", name)
		}
		if req.ExpectParagraphCount != nil && *req.ExpectParagraphCount != paraCount {
			return nil, fmt.Errorf("paragraph count guard failed: expected %d but found %d", *req.ExpectParagraphCount, paraCount)
		}
		result.ParagraphCount = paraCount
		for i := 0; i < paraCount; i++ {
			ranges = append(ranges, &inspect.ParaRange{Start: i, End: i})
		}
	} else if req.ParagraphRange != nil {
		ranges = append(ranges, req.ParagraphRange)
	} else {
		ranges = append(ranges, nil)
	}

	// Build one effect par per range, each attached DIRECTLY under the mainSeq
	// childTnLst as its own click/with/after step -- matching the golden real
	// PowerPoint structure where the preset-bearing effect cTn IS the step cTn
	// (there is no separate indefinite-delay wrapper). The first range honors the
	// requested start trigger; per-paragraph follow-ups play afterPrevious so the
	// build advances.
	//
	// IDs are allocated against the LIVE timing tree and each new node is attached
	// before the next allocation, so every cTn @id is unique across the whole tree
	// (including any preserved unknown nodes). max(existing)+1 each call.
	for i, pr := range ranges {
		effStart := start
		if i > 0 {
			effStart = "afterPrevious"
		}
		effectID := allocateTimingNodeID(timing)
		behaviorBaseID := allocateTimingNodeIDAfter(timing, effectID)
		effectCTn := buildEffectCTn(effect, direction, durationMs, spid, pr, effectID, behaviorBaseID, effStart)
		mainSeqChildTnLst.AddChild(wrapEffectPar(effectCTn))
		result.AddedEffectIDs = append(result.AddedEffectIDs, effectID)
		// In the flattened structure the effect cTn is itself the click/with/after
		// step, so the click-step id equals the first effect's id.
		if result.ClickStepID == 0 {
			result.ClickStepID = effectID
		}
	}

	if req.ByParagraph {
		ensureBldP(timing, spid)
	}

	doc.IndentTabs()
	if err := req.Package.ReplaceXMLPart(req.SlideRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to write slide: %w", err)
	}
	return result, nil
}

// ---------------------------------------------------------------------------
// RemoveAnimation
// ---------------------------------------------------------------------------

// RemoveAnimation deletes the effect cTn with the given id (and collapses its
// enclosing click-step par when it becomes empty). It refuses ids that resolve to
// nodes we do not own (unsupported/preserved effects), so motion-path / emphasis
// XML can never be deleted by id collision.
func RemoveAnimation(req *RemoveAnimationRequest) (*RemoveAnimationResult, error) {
	if req == nil || req.Package == nil {
		return nil, fmt.Errorf("remove animation request/package is nil")
	}
	if req.SlideRef == nil {
		return nil, fmt.Errorf("slide reference is nil")
	}
	doc, err := req.Package.ReadXMLPart(req.SlideRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read slide: %w", err)
	}
	root := doc.Root()
	spTree := findSlideSpTree(root)
	timing := xmlx.FindChild(root, ns.NsP, "timing")
	if timing == nil {
		return nil, &TargetNotFoundError{msg: fmt.Sprintf("no animations on slide (effect id %d not found)", req.EffectID)}
	}

	effectCTn := findCTnByID(timing, req.EffectID)
	if effectCTn == nil {
		return nil, &TargetNotFoundError{msg: fmt.Sprintf("effect id %d not found", req.EffectID)}
	}
	if !isOwnedEntranceEffect(effectCTn) {
		return nil, &TargetNotFoundError{msg: fmt.Sprintf("effect id %d is not a supported entrance effect (refusing to delete preserved/unsupported XML)", req.EffectID)}
	}

	spid, name := effectTargetShape(effectCTn, spTree)
	if req.ExpectShapeName != "" && req.ExpectShapeName != name {
		return nil, fmt.Errorf("shape name guard failed: expected %q but effect targets %q", req.ExpectShapeName, name)
	}

	// In the flattened structure the effect cTn is wrapped in a single p:par that
	// sits directly under the mainSeq childTnLst. Removing that par removes the
	// whole step; sibling steps and unknown nodes are untouched.
	effectPar := effectCTn.Parent() // p:par
	if effectPar == nil || effectPar.Parent() == nil {
		return nil, &TargetNotFoundError{msg: fmt.Sprintf("effect id %d is not a removable step", req.EffectID)}
	}
	effectPar.Parent().RemoveChild(effectPar)

	result := &RemoveAnimationResult{RemovedEffectID: req.EffectID, ShapeID: spid, ShapeName: name, RemovedClickStep: true}

	doc.IndentTabs()
	if err := req.Package.ReplaceXMLPart(req.SlideRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to write slide: %w", err)
	}
	return result, nil
}

// ---------------------------------------------------------------------------
// ReorderAnimations
// ---------------------------------------------------------------------------

// ReorderAnimations permutes the per-click steps of the mainSeq childTnLst. Order
// must be a permutation of the existing clickEffect cTn ids; unknown sibling nodes
// (non-par children of the childTnLst) are preserved after the reordered set in
// their original relative order.
func ReorderAnimations(req *ReorderAnimationsRequest) (*ReorderAnimationsResult, error) {
	if req == nil || req.Package == nil {
		return nil, fmt.Errorf("reorder animations request/package is nil")
	}
	if req.SlideRef == nil {
		return nil, fmt.Errorf("slide reference is nil")
	}
	doc, err := req.Package.ReadXMLPart(req.SlideRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read slide: %w", err)
	}
	root := doc.Root()
	timing := xmlx.FindChild(root, ns.NsP, "timing")
	if timing == nil {
		return nil, fmt.Errorf("no animations on slide to reorder")
	}
	mainSeqCTn := findMainSeqCTnIn(timing)
	if mainSeqCTn == nil {
		return nil, fmt.Errorf("no main sequence found to reorder")
	}
	childTnLst := xmlx.FindChild(mainSeqCTn, ns.NsP, "childTnLst")
	if childTnLst == nil {
		return nil, fmt.Errorf("main sequence has no child steps")
	}

	// Map each click-step par by its cTn id; record original order for validation.
	steps := map[int]*etree.Element{}
	var existing []int
	for _, par := range xmlx.FindChildren(childTnLst, ns.NsP, "par") {
		stepCTn := xmlx.FindChild(par, ns.NsP, "cTn")
		if stepCTn == nil {
			continue
		}
		id, ok := parseIntAttr(stepCTn, "id")
		if !ok {
			continue
		}
		steps[id] = par
		existing = append(existing, id)
	}

	if err := validatePermutation(req.Order, existing); err != nil {
		return nil, err
	}

	// Collect non-par children (unknown siblings) to re-append after the reorder.
	var unknownSiblings []*etree.Element
	for _, child := range childTnLst.ChildElements() {
		if localTag(child.Tag) != "par" {
			unknownSiblings = append(unknownSiblings, child)
		}
	}

	// Detach every child, then re-append in the new order.
	for _, child := range childTnLst.ChildElements() {
		childTnLst.RemoveChild(child)
	}
	for _, id := range req.Order {
		childTnLst.AddChild(steps[id])
	}
	for _, sib := range unknownSiblings {
		childTnLst.AddChild(sib)
	}

	doc.IndentTabs()
	if err := req.Package.ReplaceXMLPart(req.SlideRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to write slide: %w", err)
	}
	return &ReorderAnimationsResult{Order: req.Order, ClickStepCount: len(req.Order)}, nil
}

// ---------------------------------------------------------------------------
// PruneStale
// ---------------------------------------------------------------------------

// PruneStale removes only the effect/build nodes flagged stale (missing-shape,
// pRg-out-of-range) by inspect.ReadAnimations. With DryRun it reports candidates
// without writing. Media p:pic nodes are intentionally left untouched (owned by
// the media slice). Non-stale and unsupported-but-valid nodes are never touched.
func PruneStale(req *PruneStaleRequest) (*PruneStaleResult, error) {
	if req == nil || req.Package == nil {
		return nil, fmt.Errorf("prune-stale request/package is nil")
	}
	report, err := inspect.ReadAnimations(req.Package)
	if err != nil {
		return nil, err
	}
	want := map[int]bool{}
	for _, sr := range req.SlideRefs {
		want[sr.SlideNumber] = true
	}

	result := &PruneStaleResult{Pruned: []PrunedNode{}}
	for _, slide := range report.Slides {
		if len(want) > 0 && !want[slide.Slide] {
			continue
		}
		staleEffectIDs := map[int]string{}
		var staleBuildSpids []BuildStale
		for _, e := range slide.Effects {
			// Only prune stale effects WE own (supported entrance effects). A stale
			// UNSUPPORTED effect (e.g. a motion path on a deleted shape) is preserved,
			// never deleted, and therefore not counted as pruned -- the preserve-unknown
			// contract takes precedence over staleness for nodes we do not author.
			if e.Stale && e.Supported && e.EffectID != 0 {
				staleEffectIDs[e.EffectID] = e.StaleReason
			}
		}
		for _, b := range slide.Builds {
			if b.Stale {
				staleBuildSpids = append(staleBuildSpids, BuildStale{Spid: b.Spid, Reason: b.StaleReason})
			}
		}
		if len(staleEffectIDs) == 0 && len(staleBuildSpids) == 0 {
			continue
		}

		// Record candidates first (works for dry-run and real prune).
		for id, reason := range staleEffectIDs {
			result.Pruned = append(result.Pruned, PrunedNode{Slide: slide.Slide, Kind: "effect", EffectID: id, StaleReason: reason})
		}
		for _, b := range staleBuildSpids {
			result.Pruned = append(result.Pruned, PrunedNode{Slide: slide.Slide, Kind: "build", Spid: b.Spid, StaleReason: b.Reason})
		}

		if req.DryRun {
			continue
		}

		// Apply the prune to this slide's XML.
		doc, err := req.Package.ReadXMLPart(slide.PartURI)
		if err != nil {
			return nil, fmt.Errorf("failed to read slide %s: %w", slide.PartURI, err)
		}
		root := doc.Root()
		timing := xmlx.FindChild(root, ns.NsP, "timing")
		if timing == nil {
			continue
		}
		for id := range staleEffectIDs {
			pruneEffectByID(timing, id)
		}
		for _, b := range staleBuildSpids {
			pruneBuildBySpid(timing, b.Spid)
		}
		doc.IndentTabs()
		if err := req.Package.ReplaceXMLPart(slide.PartURI, doc); err != nil {
			return nil, fmt.Errorf("failed to write slide %s: %w", slide.PartURI, err)
		}
	}

	// Sort for stable output (slide, then kind, then id/spid).
	sort.SliceStable(result.Pruned, func(i, j int) bool {
		a, b := result.Pruned[i], result.Pruned[j]
		if a.Slide != b.Slide {
			return a.Slide < b.Slide
		}
		if a.Kind != b.Kind {
			return a.Kind < b.Kind
		}
		if a.EffectID != b.EffectID {
			return a.EffectID < b.EffectID
		}
		return a.Spid < b.Spid
	})
	return result, nil
}

// BuildStale pairs a stale build's spid with its reason.
type BuildStale struct {
	Spid   int
	Reason string
}

// ---------------------------------------------------------------------------
// TargetNotFoundError — lets the CLI map to ExitTargetNotFound.
// ---------------------------------------------------------------------------

// TargetNotFoundError signals that a requested effect id does not exist or is not
// an effect we own.
type TargetNotFoundError struct{ msg string }

func (e *TargetNotFoundError) Error() string { return e.msg }

// ---------------------------------------------------------------------------
// Timing-tree get-or-create + ID allocation (never rebuild existing nodes).
// ---------------------------------------------------------------------------

// slideRootChildOrder is the CT_Slide child element sequence (ECMA-376 Part 1):
// cSld, clrMapOvr, transition, timing, extLst. p:timing inserts after
// clrMapOvr/transition and before extLst.
var slideRootChildOrder = []string{"cSld", "clrMapOvr", "transition", "timing", "extLst"}

func slideRootChildRank(local string) int {
	for i, name := range slideRootChildOrder {
		if name == local {
			return i
		}
	}
	return len(slideRootChildOrder)
}

// getOrCreateTiming returns the slide's p:timing, creating it in schema order if
// absent. The boolean reports whether it was created.
func getOrCreateTiming(root *etree.Element) (*etree.Element, bool) {
	if t := xmlx.FindChild(root, ns.NsP, "timing"); t != nil {
		return t, false
	}
	return insertSlideRootChild(root, "timing"), true
}

// insertSlideRootChild creates a schema-ordered child element at the p:sld level.
func insertSlideRootChild(root *etree.Element, local string) *etree.Element {
	newChild := etree.NewElement("p:" + local)
	rank := slideRootChildRank(local)
	for _, existing := range root.ChildElements() {
		if slideRootChildRank(localTag(existing.Tag)) > rank {
			root.InsertChildAt(existing.Index(), newChild)
			return newChild
		}
	}
	root.AddChild(newChild)
	return newChild
}

// getOrCreateMainSeq returns the inner cTn (nodeType=mainSeq) of the timing's
// main sequence, building the tmRoot/seq/mainSeq skeleton only when absent. An
// existing tree is descended into, never rebuilt.
func getOrCreateMainSeq(timing *etree.Element) *etree.Element {
	if existing := findMainSeqCTnIn(timing); existing != nil {
		// Ensure the mainSeq cTn has a childTnLst to append click steps into.
		if xmlx.FindChild(existing, ns.NsP, "childTnLst") == nil {
			existing.AddChild(etree.NewElement("p:childTnLst"))
		}
		return existing
	}

	// Converge on the SINGLE tmRoot: reuse the existing tmRoot cTn's childTnLst
	// (or build exactly one when none exists) via the same locator the media
	// registration injector uses, then attach the mainSeq seq there. This prevents
	// a second tmRoot from being authored when media already created one (or vice
	// versa).
	rootChildTnLst := getOrCreateTmRootChildTnLst(timing)

	seq := etree.NewElement("p:seq")
	seq.CreateAttr("concurrent", "1")
	seq.CreateAttr("nextAc", "seek")
	mainSeqCTn := etree.NewElement("p:cTn")
	mainSeqCTn.CreateAttr("id", strconv.Itoa(allocateTimingNodeID(timing)))
	mainSeqCTn.CreateAttr("dur", "indefinite")
	mainSeqCTn.CreateAttr("nodeType", "mainSeq")
	mainSeqCTn.AddChild(etree.NewElement("p:childTnLst"))
	seq.AddChild(mainSeqCTn)
	seq.AddChild(seqCondLst("prevCondLst", "onPrev"))
	seq.AddChild(seqCondLst("nextCondLst", "onNext"))
	rootChildTnLst.AddChild(seq)

	return mainSeqCTn
}

// seqCondLst builds a p:prevCondLst / p:nextCondLst with a single onPrev/onNext
// cond targeting the slide.
func seqCondLst(listLocal, evt string) *etree.Element {
	lst := etree.NewElement("p:" + listLocal)
	cond := etree.NewElement("p:cond")
	cond.CreateAttr("evt", evt)
	cond.CreateAttr("delay", "0")
	tgtEl := etree.NewElement("p:tgtEl")
	tgtEl.AddChild(etree.NewElement("p:sldTgt"))
	cond.AddChild(tgtEl)
	lst.AddChild(cond)
	return lst
}

// allocateTimingNodeID returns max(existing cTn @id over the WHOLE timing
// subtree)+1, so a new id cannot collide with any preserved unknown node. This
// scans the live tree each call, so successive calls allocate distinct ids only
// once the newly built nodes carry their ids; callers that need several ids before
// attaching nodes use allocateTimingNodeIDAfter.
func allocateTimingNodeID(timing *etree.Element) int {
	return maxTimingNodeID(timing) + 1
}

// allocateTimingNodeIDAfter returns the next id strictly greater than prev and
// greater than every existing id, used when allocating a second id before the
// first node has been attached to the tree.
func allocateTimingNodeIDAfter(timing *etree.Element, prev int) int {
	next := maxTimingNodeID(timing) + 1
	if next <= prev {
		next = prev + 1
	}
	return next
}

func maxTimingNodeID(timing *etree.Element) int {
	maxID := 0
	for _, cTn := range xmlx.FindDescendants(timing, ns.NsP, "cTn") {
		if id, ok := parseIntAttr(cTn, "id"); ok && id > maxID {
			maxID = id
		}
	}
	return maxID
}

// ---------------------------------------------------------------------------
// Effect / step builders.
// ---------------------------------------------------------------------------

// wrapEffectPar wraps an effect cTn in its p:par. In the flattened (golden)
// structure this p:par is attached directly under the mainSeq childTnLst, so the
// preset-bearing effect cTn is itself the click/with/after step.
func wrapEffectPar(effectCTn *etree.Element) *etree.Element {
	par := etree.NewElement("p:par")
	par.AddChild(effectCTn)
	return par
}

// buildEffectCTn builds the presetClass-bearing effect cTn and its behaviors for
// one entrance effect.
func buildEffectCTn(effect, direction string, durationMs, spid int, pr *inspect.ParaRange, effectID, behaviorBaseID int, start string) *etree.Element {
	preset := presetByEffect[effect]
	cTn := etree.NewElement("p:cTn")
	cTn.CreateAttr("id", strconv.Itoa(effectID))
	cTn.CreateAttr("presetID", preset.presetID)
	cTn.CreateAttr("presetClass", presetClassEntrance)
	cTn.CreateAttr("presetSubtype", preset.presetSubtype)
	cTn.CreateAttr("fill", "hold")
	cTn.CreateAttr("grpId", "0")
	cTn.CreateAttr("nodeType", nodeTypeForStart(start))
	cTn.AddChild(stCondLst("0"))
	childTnLst := etree.NewElement("p:childTnLst")
	cTn.AddChild(childTnLst)

	switch effect {
	case "appear":
		childTnLst.AddChild(buildVisibilitySet(behaviorBaseID, spid, pr))
	case "fade":
		childTnLst.AddChild(buildAnimEffect(filterFade, durationMs, behaviorBaseID, spid, pr))
	case "wipe":
		childTnLst.AddChild(buildVisibilitySet(behaviorBaseID, spid, pr))
		childTnLst.AddChild(buildAnimEffect(wipeFilterByDirection[direction], durationMs, behaviorBaseID+1, spid, pr))
	case "flyIn":
		childTnLst.AddChild(buildVisibilitySet(behaviorBaseID, spid, pr))
		childTnLst.AddChild(buildFlyInAnim(direction, durationMs, behaviorBaseID+1, spid, pr))
	}
	return cTn
}

// buildVisibilitySet builds the p:set that flips style.visibility to visible.
func buildVisibilitySet(cTnID, spid int, pr *inspect.ParaRange) *etree.Element {
	set := etree.NewElement("p:set")
	cBhvr := buildCBhvr(cTnID, "1", spid, pr, "style.visibility")
	set.AddChild(cBhvr)
	to := etree.NewElement("p:to")
	strVal := etree.NewElement("p:strVal")
	strVal.CreateAttr("val", "visible")
	to.AddChild(strVal)
	set.AddChild(to)
	return set
}

// buildAnimEffect builds a p:animEffect transition="in" filter=... behavior.
//
// spec-grounded; PowerPoint-render unconfirmed: the @filter vocabulary follows
// ECMA-376 CT_TLAnimateEffectBehavior/@filter ("type(subtype)" syntax). See
// animspec.go for the exact tokens and their grounding tier.
func buildAnimEffect(filter string, durationMs, cTnID, spid int, pr *inspect.ParaRange) *etree.Element {
	ae := etree.NewElement("p:animEffect")
	ae.CreateAttr("transition", "in")
	ae.CreateAttr("filter", filter)
	cBhvr := buildCBhvr(cTnID, strconv.Itoa(durationMs), spid, pr, "")
	ae.AddChild(cBhvr)
	return ae
}

// buildFlyInAnim builds the p:anim that slides the shape in along ppt_x/ppt_y.
//
// spec-grounded; PowerPoint-render unconfirmed: the ppt_x/ppt_y/#ppt_* motion
// form follows the MS Learn animation walkthrough. See animspec.go.
func buildFlyInAnim(direction string, durationMs, cTnID, spid int, pr *inspect.ParaRange) *etree.Element {
	motion := flyInMotionByDirection[direction]
	anim := etree.NewElement("p:anim")
	anim.CreateAttr("calcmode", "lin")
	anim.CreateAttr("valueType", "num")
	cBhvr := buildCBhvr(cTnID, strconv.Itoa(durationMs), spid, pr, motion.attrName)
	// cBhvr carries additive="base" for fly-in motion.
	cBhvr.CreateAttr("additive", "base")
	// Move additive to the front-most attribute is not required; order of attrs is
	// not schema-significant. We keep cTn/tgtEl/attrNameLst element order intact.
	anim.AddChild(cBhvr)
	tavLst := etree.NewElement("p:tavLst")
	tavLst.AddChild(buildTav("0", motion.from))
	tavLst.AddChild(buildTav("100000", motion.to))
	anim.AddChild(tavLst)
	return anim
}

func buildTav(tm, val string) *etree.Element {
	tav := etree.NewElement("p:tav")
	tav.CreateAttr("tm", tm)
	v := etree.NewElement("p:val")
	strVal := etree.NewElement("p:strVal")
	strVal.CreateAttr("val", val)
	v.AddChild(strVal)
	tav.AddChild(v)
	return tav
}

// buildCBhvr builds the shared p:cBhvr wrapper with strict child order:
// cTn, tgtEl, then optional attrNameLst. attrName=="" omits the attrNameLst.
func buildCBhvr(cTnID int, dur string, spid int, pr *inspect.ParaRange, attrName string) *etree.Element {
	cBhvr := etree.NewElement("p:cBhvr")
	cTn := etree.NewElement("p:cTn")
	cTn.CreateAttr("id", strconv.Itoa(cTnID))
	cTn.CreateAttr("dur", dur)
	if attrName == "style.visibility" {
		cTn.CreateAttr("fill", "hold")
	}
	cBhvr.AddChild(cTn)
	cBhvr.AddChild(buildSpTgt(spid, pr))
	if attrName != "" {
		lst := etree.NewElement("p:attrNameLst")
		an := etree.NewElement("p:attrName")
		an.SetText(attrName)
		lst.AddChild(an)
		cBhvr.AddChild(lst)
	}
	return cBhvr
}

// buildSpTgt builds p:tgtEl/p:spTgt[spid], optionally scoped to a paragraph range.
func buildSpTgt(spid int, pr *inspect.ParaRange) *etree.Element {
	tgtEl := etree.NewElement("p:tgtEl")
	spTgt := etree.NewElement("p:spTgt")
	spTgt.CreateAttr("spid", strconv.Itoa(spid))
	if pr != nil {
		txEl := etree.NewElement("p:txEl")
		pRg := etree.NewElement("p:pRg")
		pRg.CreateAttr("st", strconv.Itoa(pr.Start))
		pRg.CreateAttr("end", strconv.Itoa(pr.End))
		txEl.AddChild(pRg)
		spTgt.AddChild(txEl)
	}
	tgtEl.AddChild(spTgt)
	return tgtEl
}

func stCondLst(delay string) *etree.Element {
	lst := etree.NewElement("p:stCondLst")
	cond := etree.NewElement("p:cond")
	cond.CreateAttr("delay", delay)
	lst.AddChild(cond)
	return lst
}

func nodeTypeForStart(start string) string {
	switch start {
	case "withPrevious":
		return "withEffect"
	case "afterPrevious":
		return "afterEffect"
	default:
		return "clickEffect"
	}
}

// ensureBldP adds a build="p" (by-paragraph) p:bldP for spid into the timing's
// p:bldLst, creating the bldLst (after p:tnLst) if absent. An existing bldP for
// the spid is updated to build="p" rather than duplicated.
func ensureBldP(timing *etree.Element, spid int) {
	bldLst := xmlx.FindChild(timing, ns.NsP, "bldLst")
	if bldLst == nil {
		bldLst = etree.NewElement("p:bldLst")
		// bldLst follows tnLst.
		if tnLst := xmlx.FindChild(timing, ns.NsP, "tnLst"); tnLst != nil {
			timing.InsertChildAt(tnLst.Index()+1, bldLst)
		} else {
			timing.AddChild(bldLst)
		}
	}
	for _, bldP := range xmlx.FindChildren(bldLst, ns.NsP, "bldP") {
		if id, ok := parseIntAttr(bldP, "spid"); ok && id == spid {
			bldP.CreateAttr("build", buildByParagraph)
			return
		}
	}
	bldP := etree.NewElement("p:bldP")
	bldP.CreateAttr("spid", strconv.Itoa(spid))
	bldP.CreateAttr("grpId", "0")
	bldP.CreateAttr("build", buildByParagraph)
	bldLst.AddChild(bldP)
}

// ---------------------------------------------------------------------------
// Lookups / classification shared with surgery.
// ---------------------------------------------------------------------------

// findMainSeqCTnIn descends timing/tnLst to the first seq whose inner cTn has
// nodeType=mainSeq, returning that inner cTn.
func findMainSeqCTnIn(timing *etree.Element) *etree.Element {
	tnLst := xmlx.FindChild(timing, ns.NsP, "tnLst")
	if tnLst == nil {
		return nil
	}
	for _, seq := range xmlx.FindDescendants(tnLst, ns.NsP, "seq") {
		cTn := xmlx.FindChild(seq, ns.NsP, "cTn")
		if cTn == nil {
			continue
		}
		if nt, _ := xmlx.GetAttr(cTn, "nodeType"); nt == "mainSeq" {
			return cTn
		}
	}
	return nil
}

func childTnLstOf(cTn *etree.Element) *etree.Element {
	if cTn == nil {
		return nil
	}
	lst := xmlx.FindChild(cTn, ns.NsP, "childTnLst")
	if lst == nil {
		lst = etree.NewElement("p:childTnLst")
		cTn.AddChild(lst)
	}
	return lst
}

// findCTnByID returns the cTn with the given @id anywhere in the timing subtree.
func findCTnByID(timing *etree.Element, id int) *etree.Element {
	for _, cTn := range xmlx.FindDescendants(timing, ns.NsP, "cTn") {
		if v, ok := parseIntAttr(cTn, "id"); ok && v == id {
			return cTn
		}
	}
	return nil
}

// isOwnedEntranceEffect reports whether an effect cTn is one of the in-scope
// entrances we author. It delegates to inspect.ClassifySupported so the
// remove/prune ownership gate is byte-for-byte the SAME classifier the read-side
// uses: an effect the reader marks unsupported (e.g. a p:set on style.color or a
// p:anim on ppt_w/ppt_h) is never treated as owned, so preserved/third-party XML
// can never be deleted by id collision.
func isOwnedEntranceEffect(cTn *etree.Element) bool {
	return inspect.ClassifySupported(cTn)
}

// effectTargetShape returns the spid and resolved name an effect cTn targets.
func effectTargetShape(cTn, spTree *etree.Element) (int, string) {
	for _, sp := range xmlx.FindDescendants(cTn, ns.NsP, "spTgt") {
		if id, ok := parseIntAttr(sp, "spid"); ok {
			return id, shapeNameByID(spTree, id)
		}
	}
	return 0, ""
}

// pruneEffectByID removes the owned effect cTn with id by deleting its enclosing
// p:par step. It re-checks ownership as a safety net so it can never delete a
// preserved/unsupported node even if an unexpected id is passed.
func pruneEffectByID(timing *etree.Element, id int) {
	cTn := findCTnByID(timing, id)
	if cTn == nil || !isOwnedEntranceEffect(cTn) {
		return
	}
	effectPar := cTn.Parent()
	if effectPar == nil || effectPar.Parent() == nil {
		return
	}
	effectPar.Parent().RemoveChild(effectPar)
}

// pruneBuildBySpid removes the p:bldP for spid from the timing's bldLst.
func pruneBuildBySpid(timing *etree.Element, spid int) {
	bldLst := xmlx.FindChild(timing, ns.NsP, "bldLst")
	if bldLst == nil {
		return
	}
	for _, bldP := range xmlx.FindChildren(bldLst, ns.NsP, "bldP") {
		if id, ok := parseIntAttr(bldP, "spid"); ok && id == spid {
			bldLst.RemoveChild(bldP)
		}
	}
}

// validatePermutation checks order is a permutation of existing (no missing,
// extra, duplicate, or unknown ids).
func validatePermutation(order, existing []int) error {
	if len(order) != len(existing) {
		return fmt.Errorf("--order must list all %d click steps (got %d); valid ids: %s", len(existing), len(order), joinInts(existing))
	}
	existsSet := map[int]bool{}
	for _, id := range existing {
		existsSet[id] = true
	}
	seen := map[int]bool{}
	for _, id := range order {
		if !existsSet[id] {
			return fmt.Errorf("--order contains unknown id %d; valid ids: %s", id, joinInts(existing))
		}
		if seen[id] {
			return fmt.Errorf("--order contains duplicate id %d", id)
		}
		seen[id] = true
	}
	return nil
}

func joinInts(ids []int) string {
	var b strings.Builder
	for i, id := range ids {
		if i > 0 {
			b.WriteString(",")
		}
		b.WriteString(strconv.Itoa(id))
	}
	return b.String()
}

// ---------------------------------------------------------------------------
// Shape resolution against the spTree.
// ---------------------------------------------------------------------------

// resolveShapeInTree resolves a ShapeIDSelector / ShapeNameSelector to its shape
// element, spid, and name by walking spTree's cNvPr descendants. It returns an
// error for unsupported selector kinds or a missing target.
func resolveShapeInTree(spTree *etree.Element, sel selectors.Selector) (*etree.Element, int, string, error) {
	switch s := sel.(type) {
	case *selectors.ShapeIDSelector:
		for _, shape := range collectTargetableShapes(spTree) {
			if id := shapeCNvPrIDLocal(shape); id == s.ID {
				return shape, id, shapeNameLocal(shape), nil
			}
		}
		return nil, 0, "", fmt.Errorf("shape with id %d not found on slide", s.ID)
	case *selectors.ShapeNameSelector:
		for _, shape := range collectTargetableShapes(spTree) {
			if shapeNameLocal(shape) == s.Name {
				return shape, shapeCNvPrIDLocal(shape), s.Name, nil
			}
		}
		return nil, 0, "", fmt.Errorf("shape named %q not found on slide", s.Name)
	default:
		return nil, 0, "", fmt.Errorf("unsupported shape selector %q (use shape:<id> or ~<name>)", sel.String())
	}
}

// collectTargetableShapes returns every sp/pic/graphicFrame/grpSp under spTree.
func collectTargetableShapes(spTree *etree.Element) []*etree.Element {
	var out []*etree.Element
	for _, local := range []string{"sp", "pic", "graphicFrame", "grpSp"} {
		out = append(out, xmlx.FindDescendants(spTree, ns.NsP, local)...)
	}
	return out
}

func shapeCNvPrIDLocal(shape *etree.Element) int {
	for _, nv := range []string{"nvSpPr", "nvPicPr", "nvGrpSpPr", "nvGraphicFramePr"} {
		if nvPr := xmlx.FindChild(shape, ns.NsP, nv); nvPr != nil {
			if cNvPr := xmlx.FindChild(nvPr, ns.NsP, "cNvPr"); cNvPr != nil {
				if id, ok := parseIntAttr(cNvPr, "id"); ok {
					return id
				}
			}
		}
	}
	return 0
}

func shapeNameLocal(shape *etree.Element) string {
	for _, nv := range []string{"nvSpPr", "nvPicPr", "nvGrpSpPr", "nvGraphicFramePr"} {
		if nvPr := xmlx.FindChild(shape, ns.NsP, nv); nvPr != nil {
			if cNvPr := xmlx.FindChild(nvPr, ns.NsP, "cNvPr"); cNvPr != nil {
				name, _ := xmlx.GetAttr(cNvPr, "name")
				return name
			}
		}
	}
	return ""
}

func shapeNameByID(spTree *etree.Element, id int) string {
	if spTree == nil {
		return ""
	}
	for _, shape := range collectTargetableShapes(spTree) {
		if shapeCNvPrIDLocal(shape) == id {
			return shapeNameLocal(shape)
		}
	}
	return ""
}

// parseIntAttr parses an integer attribute.
func parseIntAttr(elem *etree.Element, name string) (int, bool) {
	v, ok := xmlx.GetAttr(elem, name)
	if !ok {
		return 0, false
	}
	n, err := strconv.Atoi(strings.TrimSpace(v))
	if err != nil {
		return 0, false
	}
	return n, true
}

// normalizeEffect canonicalizes the effect flag (accepts fly-in / flyin / flyIn).
func normalizeEffect(effect string) string {
	switch strings.ToLower(strings.TrimSpace(effect)) {
	case "appear":
		return "appear"
	case "fade":
		return "fade"
	case "wipe":
		return "wipe"
	case "fly-in", "flyin":
		return "flyIn"
	default:
		return ""
	}
}

// normalizeStart canonicalizes the start flag.
func normalizeStart(start string) string {
	switch strings.ToLower(strings.TrimSpace(start)) {
	case "", "onclick":
		return "onClick"
	case "withprevious":
		return "withPrevious"
	case "afterprevious":
		return "afterPrevious"
	default:
		return ""
	}
}
