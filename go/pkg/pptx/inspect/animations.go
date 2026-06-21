package inspect

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	ns "github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

// pRgSentinel is the OOXML "all/last" range marker (0xFFFFFFFF). It appears on
// p:charRg (and occasionally p:pRg); a concrete paragraph range should be a real
// index, so this value is excluded from the out-of-range stale check.
const pRgSentinel = 4294967295

// AnimationsReport summarizes per-slide animation timing for a presentation.
// It is strictly read-only: it walks each slide's p:timing tree and reports the
// ordered effects, paragraph builds, and embedded media it finds, classifying
// each effect as one of the in-scope entrance kinds or "unsupported:<raw>".
type AnimationsReport struct {
	Slides []AnimationsSlideInfo `json:"slides"`
}

// AnimationsSlideInfo holds the animation state of a single slide.
type AnimationsSlideInfo struct {
	Slide     int               `json:"slide"`   // 1-based presentation order
	PartURI   string            `json:"partUri"` // slide part URI
	HasTiming bool              `json:"hasTiming"`
	Effects   []AnimationEffect `json:"effects"` // ordered by mainSeq document order
	Builds    []BuildInfo       `json:"builds,omitempty"`
	Media     []MediaInfo       `json:"media,omitempty"`
	// UnsupportedCount counts effect cTns whose behaviors did not match an
	// in-scope entrance pattern (EffectKind carries the "unsupported:" prefix).
	UnsupportedCount int `json:"unsupportedCount"`
}

// AnimationEffect is one collapsed effect record: one per preset-bearing effect
// cTn, NOT one per behavior element. A fly-in (p:set + 2x p:anim) and a wipe
// (p:set + p:animEffect) each collapse to a single record.
type AnimationEffect struct {
	SequencePos int `json:"sequencePos"` // 0-based position across all effects in mainSeq doc order
	ClickStep   int `json:"clickStep"`   // 0-based index of the enclosing clickEffect par
	// EffectID is the effect cTn @id (0 if absent/unparseable). It is the stable
	// selector used by `animations remove --effect-id`.
	EffectID int `json:"effectId"`
	// ClickStepID is the @id of the enclosing clickEffect cTn (0 if absent). It is
	// the stable selector used by `animations reorder --order`.
	ClickStepID     int      `json:"clickStepId"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
	// EffectKind is one of appear|fade|wipe|flyIn for in-scope effects, else
	// "unsupported:<raw>" (e.g. "unsupported:animMotion",
	// "unsupported:animEffect(blinds)").
	EffectKind     string     `json:"effectKind"`
	Supported      bool       `json:"supported"`
	PresetClass    string     `json:"presetClass,omitempty"`
	PresetID       string     `json:"presetId,omitempty"`      // advisory; surfaced not interpreted
	PresetSubtype  string     `json:"presetSubtype,omitempty"` // advisory direction hint
	Filter         string     `json:"filter,omitempty"`        // p:animEffect@filter raw string
	StartType      string     `json:"startType"`               // onClick|withPrevious|afterPrevious|unknown
	Spid           int        `json:"spid"`                    // p:spTgt@spid (0 if absent/unparseable)
	ShapeName      string     `json:"shapeName,omitempty"`     // resolved cNvPr@name ("" if stale/absent)
	ParagraphRange *ParaRange `json:"paragraphRange,omitempty"`
	Stale          bool       `json:"stale"`
	StaleReason    string     `json:"staleReason,omitempty"` // missing-shape | pRg-out-of-range:<st>-<end>/<count>
}

// ParaRange is a 0-based inclusive paragraph index range from p:txEl/p:pRg.
type ParaRange struct {
	Start int `json:"start"`
	End   int `json:"end"`
}

// BuildInfo describes a p:bldLst/p:bldP paragraph-build declaration.
type BuildInfo struct {
	Spid        int    `json:"spid"`
	ShapeName   string `json:"shapeName,omitempty"`
	Build       string `json:"build"` // raw p:bldP@build token (byParagraph|p|allAtOnce|...)
	GrpID       string `json:"grpId,omitempty"`
	Stale       bool   `json:"stale"`
	StaleReason string `json:"staleReason,omitempty"`
}

// MediaInfo describes an embedded video/audio p:pic and its play wiring.
type MediaInfo struct {
	Spid           int    `json:"spid"`
	ShapeName      string `json:"shapeName,omitempty"`
	Kind           string `json:"kind"` // video|audio|unknown
	MediaPartURI   string `json:"mediaPartUri,omitempty"`
	PosterPartURI  string `json:"posterPartUri,omitempty"`
	HasClickToPlay bool   `json:"hasClickToPlay"`
	// IsExternal marks a media reference whose rel is TargetMode=External: the
	// media (and/or poster) lives outside the package at the raw target URI and is
	// never path-joined into the package or flagged stale.
	IsExternal  bool   `json:"isExternal,omitempty"`
	Stale       bool   `json:"stale"`
	StaleReason string `json:"staleReason,omitempty"` // dangling-rel:<rid> | missing-part:<uri>
}

// ReadAnimations builds an AnimationsReport by walking each slide's p:timing
// tree. Slides without a p:timing element yield HasTiming=false and an empty
// effect list. This is a read-only inspector; it never mutates the package.
func ReadAnimations(session opc.PackageSession) (*AnimationsReport, error) {
	graph, err := ParsePresentation(session)
	if err != nil {
		return nil, err
	}
	report := &AnimationsReport{Slides: make([]AnimationsSlideInfo, 0, len(graph.Slides))}

	for _, slide := range graph.Slides {
		doc, err := session.ReadXMLPart(slide.PartURI)
		if err != nil {
			return nil, fmt.Errorf("failed to read slide %s: %w", slide.PartURI, err)
		}
		info := readSlideAnimations(session, slide.PartURI, doc.Root())
		info.Slide = slide.SlideNumber
		info.PartURI = slide.PartURI
		report.Slides = append(report.Slides, info)
	}
	return report, nil
}

// shapeIndex captures the per-slide shape facts the stale checks need: the set
// of valid cNvPr ids, the resolved name for each, and the paragraph count of
// each shape's first txBody.
type shapeIndex struct {
	names     map[int]string
	paraCount map[int]int // a:p count of the shape's txBody (only for shapes that have one)
}

func readSlideAnimations(session opc.PackageSession, partURI string, root *etree.Element) AnimationsSlideInfo {
	info := AnimationsSlideInfo{Effects: []AnimationEffect{}}
	spTree := findSpTree(root)
	idx := buildShapeIndex(spTree)

	timing := xmlx.FindChild(root, ns.NsP, "timing")

	// Media lives in the shape tree and is reported even when the slide has no
	// p:timing (an embedded clip with no animation still "has media"). The
	// click-to-play scan tolerates a nil timing.
	info.Media = collectMedia(session, partURI, spTree, timing, idx)

	if timing == nil {
		return info
	}
	info.HasTiming = true

	collectEffects(timing, idx, &info)
	info.Builds = collectBuilds(timing, idx)
	return info
}

// buildShapeIndex walks the spTree collecting every descendant cNvPr id (sp,
// pic, graphicFrame, grpSp, nested) plus, for shapes that carry one, the a:p
// count of their txBody. The recursive walk means nested-group targets resolve
// and are not falsely flagged stale.
func buildShapeIndex(spTree *etree.Element) shapeIndex {
	idx := shapeIndex{names: map[int]string{}, paraCount: map[int]int{}}
	if spTree == nil {
		return idx
	}
	for _, cNvPr := range xmlx.FindDescendants(spTree, ns.NsP, "cNvPr") {
		id, ok := parseIntAttr(cNvPr, "id")
		if !ok {
			continue
		}
		name, _ := xmlx.GetAttr(cNvPr, "name")
		idx.names[id] = name
	}
	// Paragraph counts: a shape's txBody is two levels under the shape element
	// (sp/txBody). Walk every sp/pic and record its first txBody's a:p count.
	for _, shape := range collectShapeElements(spTree) {
		id := shapeCNvPrID(shape)
		if id == 0 {
			continue
		}
		txBody := xmlx.FindChild(shape, ns.NsP, "txBody")
		if txBody == nil {
			continue
		}
		idx.paraCount[id] = len(xmlx.FindChildren(txBody, ns.NsA, "p"))
	}
	return idx
}

// collectShapeElements returns every p:sp and p:pic under spTree (recursively),
// which are the elements that can carry a txBody / be animation targets.
func collectShapeElements(spTree *etree.Element) []*etree.Element {
	var out []*etree.Element
	out = append(out, xmlx.FindDescendants(spTree, ns.NsP, "sp")...)
	out = append(out, xmlx.FindDescendants(spTree, ns.NsP, "pic")...)
	return out
}

func shapeCNvPrID(shape *etree.Element) int {
	// nvSpPr/cNvPr for sp, nvPicPr/cNvPr for pic.
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

// collectEffects walks the mainSeq, finds each click step, and collapses the
// behaviors under each preset-bearing effect cTn into one AnimationEffect.
func collectEffects(timing *etree.Element, idx shapeIndex, info *AnimationsSlideInfo) {
	mainSeq := findMainSeqCTn(timing)
	if mainSeq == nil {
		return
	}
	childTnLst := xmlx.FindChild(mainSeq, ns.NsP, "childTnLst")
	if childTnLst == nil {
		return
	}
	seqPos := 0
	clickStep := 0
	for _, par := range xmlx.FindChildren(childTnLst, ns.NsP, "par") {
		// The click-step's own cTn carries the clickEffect id used by reorder.
		clickStepID := 0
		if stepCTn := xmlx.FindChild(par, ns.NsP, "cTn"); stepCTn != nil {
			clickStepID, _ = parseIntAttr(stepCTn, "id")
		}
		// Each click-step par holds (typically) one or more effect cTns under it.
		effectCTns := findEffectCTns(par)
		for _, eff := range effectCTns {
			rec := classifyEffect(eff, idx)
			rec.SequencePos = seqPos
			rec.ClickStep = clickStep
			rec.ClickStepID = clickStepID
			rec.EffectID, _ = parseIntAttr(eff, "id")
			rec.PrimarySelector = animationEffectPrimarySelector(rec.EffectID)
			rec.Selectors = animationEffectSelectors(rec.EffectID, rec.ClickStepID)
			seqPos++
			if !rec.Supported {
				info.UnsupportedCount++
			}
			info.Effects = append(info.Effects, rec)
		}
		clickStep++
	}
}

func animationEffectPrimarySelector(effectID int) string {
	if effectID <= 0 {
		return ""
	}
	return "effect:" + strconv.Itoa(effectID)
}

func animationEffectSelectors(effectID, clickStepID int) []string {
	var out []string
	if effectID > 0 {
		out = append(out, "effect:"+strconv.Itoa(effectID), strconv.Itoa(effectID))
	}
	if clickStepID > 0 {
		out = append(out, "clickStep:"+strconv.Itoa(clickStepID))
	}
	return out
}

// findMainSeqCTn descends timing/tnLst/par/cTn[tmRoot]/childTnLst to the first
// p:seq whose inner cTn has nodeType=mainSeq, returning that inner cTn.
func findMainSeqCTn(timing *etree.Element) *etree.Element {
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

// findEffectCTns returns, in document order, the preset-bearing effect cTns
// under a click-step par. A preset-bearing cTn is one carrying a presetClass
// attribute (the effect node onto which behaviors collapse). It descends through
// intermediate (non-preset) container cTns/pars to find them.
func findEffectCTns(par *etree.Element) []*etree.Element {
	var out []*etree.Element
	// Any cTn (descendant of this par) carrying presetClass is an effect node.
	for _, cTn := range xmlx.FindDescendants(par, ns.NsP, "cTn") {
		if _, ok := xmlx.GetAttr(cTn, "presetClass"); ok {
			out = append(out, cTn)
		}
	}
	return out
}

// ClassifySupported reports whether an effect cTn is one of the in-scope
// entrance effects (appear|fade|wipe|flyIn) per the SAME logic the read-side
// classifier uses. It is the single source of truth shared with the mutate
// package so that remove/prune ownership cannot diverge from what the reader
// marks supported: anything the reader classifies as "unsupported:<raw>" (e.g.
// a p:set on style.color, or a p:anim on ppt_w/ppt_h) must never be deleted.
func ClassifySupported(eff *etree.Element) bool {
	if eff == nil {
		return false
	}
	presetClass, _ := xmlx.GetAttr(eff, "presetClass")
	behaviors := collectBehaviors(eff)
	filter := firstAnimEffectFilter(behaviors)
	_, supported := classifyKind(presetClass, behaviors, filter)
	return supported
}

// classifyEffect inspects one effect cTn (and its collapsed behaviors) and
// returns a single AnimationEffect record.
func classifyEffect(eff *etree.Element, idx shapeIndex) AnimationEffect {
	rec := AnimationEffect{StartType: "unknown"}
	rec.PresetClass, _ = xmlx.GetAttr(eff, "presetClass")
	rec.PresetID, _ = xmlx.GetAttr(eff, "presetID")
	rec.PresetSubtype, _ = xmlx.GetAttr(eff, "presetSubtype")
	if nt, ok := xmlx.GetAttr(eff, "nodeType"); ok {
		rec.StartType = startTypeFromNodeType(nt)
	}

	behaviors := collectBehaviors(eff)
	rec.Spid, rec.ParagraphRange = targetFromBehaviors(behaviors)
	rec.Filter = firstAnimEffectFilter(behaviors)

	rec.EffectKind, rec.Supported = classifyKind(rec.PresetClass, behaviors, rec.Filter)

	applyStale(&rec, idx)
	return rec
}

// startTypeFromNodeType maps an effect cTn's nodeType to a human start trigger.
func startTypeFromNodeType(nt string) string {
	switch nt {
	case "clickEffect":
		return "onClick"
	case "withEffect":
		return "withPrevious"
	case "afterEffect":
		return "afterPrevious"
	default:
		return "unknown"
	}
}

// behavior is one of the behavior element kinds we recognize under an effect.
type behavior struct {
	local string // set|animEffect|anim|cmd|animMotion|animClr|animRot|animScale|...
	elem  *etree.Element
}

// collectBehaviors gathers the behavior elements directly under the effect cTn's
// childTnLst. These all share the single effect cTn (the collapse rule).
func collectBehaviors(eff *etree.Element) []behavior {
	childTnLst := xmlx.FindChild(eff, ns.NsP, "childTnLst")
	if childTnLst == nil {
		return nil
	}
	var out []behavior
	for _, child := range childTnLst.ChildElements() {
		// Only count PresentationML behavior elements; skip nested par/cTn.
		if child.Space != "p" && child.Space != "" {
			// child.Space is a prefix; rely on local-name set below.
		}
		local := child.Tag
		switch local {
		case "set", "animEffect", "anim", "cmd", "animMotion", "animClr",
			"animRot", "animScale", "audio", "video":
			out = append(out, behavior{local: local, elem: child})
		}
	}
	return out
}

// targetFromBehaviors returns the spid and paragraph range of the first behavior
// that carries a p:cBhvr/p:tgtEl/p:spTgt target.
func targetFromBehaviors(behaviors []behavior) (int, *ParaRange) {
	for _, b := range behaviors {
		spTgt := findSpTgt(b.elem)
		if spTgt == nil {
			continue
		}
		spid := 0
		if v, ok := xmlx.GetAttr(spTgt, "spid"); ok {
			if n, err := strconv.Atoi(v); err == nil {
				spid = n
			}
		}
		pr := paraRangeFromSpTgt(spTgt)
		return spid, pr
	}
	return 0, nil
}

// findSpTgt locates p:cBhvr/p:tgtEl/p:spTgt under a behavior element.
func findSpTgt(beh *etree.Element) *etree.Element {
	cBhvr := xmlx.FindChild(beh, ns.NsP, "cBhvr")
	if cBhvr == nil {
		return nil
	}
	tgtEl := xmlx.FindChild(cBhvr, ns.NsP, "tgtEl")
	if tgtEl == nil {
		return nil
	}
	return xmlx.FindChild(tgtEl, ns.NsP, "spTgt")
}

func paraRangeFromSpTgt(spTgt *etree.Element) *ParaRange {
	txEl := xmlx.FindChild(spTgt, ns.NsP, "txEl")
	if txEl == nil {
		return nil
	}
	pRg := xmlx.FindChild(txEl, ns.NsP, "pRg")
	if pRg == nil {
		return nil
	}
	st, sok := parseIntAttr(pRg, "st")
	end, eok := parseIntAttr(pRg, "end")
	if !sok && !eok {
		return nil
	}
	return &ParaRange{Start: st, End: end}
}

func firstAnimEffectFilter(behaviors []behavior) string {
	for _, b := range behaviors {
		if b.local == "animEffect" {
			if f, ok := xmlx.GetAttr(b.elem, "filter"); ok {
				return f
			}
		}
	}
	return ""
}

// classifyKind decides the effect kind from (presetClass, behavior set, filter),
// never from presetID. A wrong filter constant only downgrades a known effect to
// unsupported:<raw> (conservative; never corrupts).
//
// spec-grounded; PowerPoint-render unconfirmed: the filter vocabulary ("fade",
// "wipe(dir)") follows CT_TLAnimateEffectBehavior/@filter (ECMA-376; the spec
// example is filter="blinds(horizontal)"). Exact fade/wipe spellings await a
// real-PowerPoint golden; a mismatch here is a safe downgrade, not a corruption.
func classifyKind(presetClass string, behaviors []behavior, filter string) (string, bool) {
	locals := behaviorLocals(behaviors)

	if presetClass == "entr" {
		switch {
		case onlyVisibilitySet(behaviors):
			return "appear", true
		case locals["animEffect"] && isInFilter(behaviors, "fade"):
			return "fade", true
		case locals["animEffect"] && isInFilterPrefix(behaviors, "wipe"):
			return "wipe", true
		case locals["anim"] && hasPositionAnim(behaviors):
			return "flyIn", true
		}
	}

	return "unsupported:" + unsupportedRaw(presetClass, behaviors, filter), false
}

func behaviorLocals(behaviors []behavior) map[string]bool {
	m := map[string]bool{}
	for _, b := range behaviors {
		m[b.local] = true
	}
	return m
}

// onlyVisibilitySet reports whether the sole behavior is a p:set on
// style.visibility (the appear pattern).
func onlyVisibilitySet(behaviors []behavior) bool {
	if len(behaviors) != 1 || behaviors[0].local != "set" {
		return false
	}
	return setAttrName(behaviors[0].elem) == "style.visibility"
}

func setAttrName(set *etree.Element) string {
	cBhvr := xmlx.FindChild(set, ns.NsP, "cBhvr")
	if cBhvr == nil {
		return ""
	}
	lst := xmlx.FindChild(cBhvr, ns.NsP, "attrNameLst")
	if lst == nil {
		return ""
	}
	if an := xmlx.FindChild(lst, ns.NsP, "attrName"); an != nil {
		return an.Text()
	}
	return ""
}

// isInFilter reports whether an animEffect behavior has transition="in" and an
// exact filter value.
func isInFilter(behaviors []behavior, filter string) bool {
	for _, b := range behaviors {
		if b.local != "animEffect" {
			continue
		}
		if t, _ := xmlx.GetAttr(b.elem, "transition"); t != "in" {
			continue
		}
		if f, _ := xmlx.GetAttr(b.elem, "filter"); f == filter {
			return true
		}
	}
	return false
}

// isInFilterPrefix reports whether an animEffect behavior has transition="in"
// and a filter starting with prefix (e.g. "wipe" matches "wipe(up)").
func isInFilterPrefix(behaviors []behavior, prefix string) bool {
	for _, b := range behaviors {
		if b.local != "animEffect" {
			continue
		}
		if t, _ := xmlx.GetAttr(b.elem, "transition"); t != "in" {
			continue
		}
		if f, _ := xmlx.GetAttr(b.elem, "filter"); strings.HasPrefix(f, prefix) {
			return true
		}
	}
	return false
}

// hasPositionAnim reports whether any p:anim behavior animates ppt_x or ppt_y
// (the fly-in motion signature).
func hasPositionAnim(behaviors []behavior) bool {
	for _, b := range behaviors {
		if b.local != "anim" {
			continue
		}
		if an := animAttrName(b.elem); an == "ppt_x" || an == "ppt_y" {
			return true
		}
	}
	return false
}

func animAttrName(anim *etree.Element) string {
	cBhvr := xmlx.FindChild(anim, ns.NsP, "cBhvr")
	if cBhvr == nil {
		return ""
	}
	lst := xmlx.FindChild(cBhvr, ns.NsP, "attrNameLst")
	if lst == nil {
		return ""
	}
	if an := xmlx.FindChild(lst, ns.NsP, "attrName"); an != nil {
		return an.Text()
	}
	return ""
}

// unsupportedRaw builds the "<raw>" suffix for an unsupported effect. It prefers
// a non-entr presetClass (path/emph/exit), then the primary behavior local-name,
// annotating an animEffect with its filter (e.g. "animEffect(blinds)").
func unsupportedRaw(presetClass string, behaviors []behavior, filter string) string {
	if presetClass != "" && presetClass != "entr" {
		// Prefer presetClass plus the behavior for motion paths etc.
		if len(behaviors) > 0 {
			return presetClass + "/" + behaviors[0].local
		}
		return presetClass
	}
	if len(behaviors) == 0 {
		return "empty"
	}
	primary := behaviors[0].local
	if primary == "animEffect" && filter != "" {
		return "animEffect(" + filter + ")"
	}
	return primary
}

// applyStale flags an effect whose target shape is missing, or whose pRg points
// past the target shape's paragraph count. spid==0 (absent/unparseable) makes no
// claim and is not flagged.
func applyStale(rec *AnimationEffect, idx shapeIndex) {
	if rec.Spid == 0 {
		return
	}
	name, ok := idx.names[rec.Spid]
	if !ok {
		rec.Stale = true
		rec.StaleReason = "missing-shape"
		return
	}
	rec.ShapeName = name
	if rec.ParagraphRange == nil {
		return
	}
	pr := rec.ParagraphRange
	if pr.Start == pRgSentinel || pr.End == pRgSentinel {
		return
	}
	n := idx.paraCount[rec.Spid] // 0 if shape has no txBody
	if pr.Start >= n || pr.End >= n {
		rec.Stale = true
		rec.StaleReason = fmt.Sprintf("pRg-out-of-range:%d-%d/%d", pr.Start, pr.End, n)
	}
}

// collectBuilds reads p:timing/p:bldLst/p:bldP entries.
func collectBuilds(timing *etree.Element, idx shapeIndex) []BuildInfo {
	bldLst := xmlx.FindChild(timing, ns.NsP, "bldLst")
	if bldLst == nil {
		return nil
	}
	var out []BuildInfo
	for _, bldP := range xmlx.FindChildren(bldLst, ns.NsP, "bldP") {
		b := BuildInfo{}
		b.Spid, _ = parseIntAttr(bldP, "spid")
		b.Build, _ = xmlx.GetAttr(bldP, "build")
		b.GrpID, _ = xmlx.GetAttr(bldP, "grpId")
		if b.Spid != 0 {
			if name, ok := idx.names[b.Spid]; ok {
				b.ShapeName = name
			} else {
				b.Stale = true
				b.StaleReason = "missing-shape"
			}
		}
		out = append(out, b)
	}
	return out
}

// collectMedia finds embedded video/audio p:pic shapes and resolves their media
// parts and click-to-play wiring.
func collectMedia(session opc.PackageSession, partURI string, spTree, timing *etree.Element, idx shapeIndex) []MediaInfo {
	if spTree == nil {
		return nil
	}
	relMap := map[string]opc.RelationshipInfo{}
	for _, rel := range session.ListRelationships(partURI) {
		relMap[rel.ID] = rel
	}
	partSet := map[string]bool{}
	for _, p := range session.ListParts() {
		partSet[p.URI] = true
	}

	var out []MediaInfo
	for _, pic := range xmlx.FindDescendants(spTree, ns.NsP, "pic") {
		mi, ok := mediaFromPic(session, partURI, pic, relMap, partSet)
		if !ok {
			continue
		}
		if mi.Spid != 0 {
			if name, present := idx.names[mi.Spid]; present {
				mi.ShapeName = name
			}
		}
		mi.HasClickToPlay = hasClickToPlay(timing, mi.Spid)
		out = append(out, mi)
	}
	return out
}

// mediaFromPic returns MediaInfo for a p:pic that carries a:videoFile/a:audioFile
// (or a p14:media embed). Non-media pictures return ok=false.
func mediaFromPic(session opc.PackageSession, partURI string, pic *etree.Element, relMap map[string]opc.RelationshipInfo, partSet map[string]bool) (MediaInfo, bool) {
	nvPicPr := xmlx.FindChild(pic, ns.NsP, "nvPicPr")
	if nvPicPr == nil {
		return MediaInfo{}, false
	}
	nvPr := xmlx.FindChild(nvPicPr, ns.NsP, "nvPr")
	if nvPr == nil {
		return MediaInfo{}, false
	}
	videoFile := xmlx.FindChild(nvPr, ns.NsA, "videoFile")
	audioFile := xmlx.FindChild(nvPr, ns.NsA, "audioFile")
	p14media := findP14Media(nvPr)
	if videoFile == nil && audioFile == nil && p14media == nil {
		return MediaInfo{}, false
	}

	mi := MediaInfo{Kind: "unknown"}
	switch {
	case videoFile != nil:
		mi.Kind = "video"
	case audioFile != nil:
		mi.Kind = "audio"
	}
	if cNvPr := xmlx.FindChild(nvPicPr, ns.NsP, "cNvPr"); cNvPr != nil {
		mi.Spid, _ = parseIntAttr(cNvPr, "id")
	}

	// Resolve the media part: prefer p14:media@r:embed, fall back to the
	// legacy a:videoFile/a:audioFile@r:link.
	var mediaRID string
	if p14media != nil {
		mediaRID = rAttr(p14media, "embed")
	}
	if mediaRID == "" {
		if videoFile != nil {
			mediaRID = rAttr(videoFile, "link")
		} else if audioFile != nil {
			mediaRID = rAttr(audioFile, "link")
		}
	}
	if mediaRID != "" {
		uri, reason, external := resolveRel(partURI, mediaRID, relMap, partSet)
		mi.MediaPartURI = uri
		if external {
			mi.IsExternal = true
		}
		if reason != "" {
			mi.Stale = true
			mi.StaleReason = reason
		}
	}

	// Poster: p:blipFill/a:blip@r:embed.
	if blipFill := xmlx.FindChild(pic, ns.NsP, "blipFill"); blipFill != nil {
		if blip := xmlx.FindChild(blipFill, ns.NsA, "blip"); blip != nil {
			if posterRID := rAttr(blip, "embed"); posterRID != "" {
				uri, reason, external := resolveRel(partURI, posterRID, relMap, partSet)
				mi.PosterPartURI = uri
				if external {
					mi.IsExternal = true
				}
				if reason != "" && !mi.Stale {
					mi.Stale = true
					mi.StaleReason = reason
				}
			}
		}
	}
	return mi, true
}

// resolveRel resolves a relationship id to a part URI, returning a stale reason
// when the rel is undeclared (dangling-rel) or its target part is absent
// (missing-part). For an external-linked target (TargetMode=External, e.g. an
// a:videoFile r:link to a file URL, drive-letter, or UNC path — the normal way
// PowerPoint LINKS large video), it returns the raw target verbatim, external=true,
// and NO stale reason: such a target lives outside the package and must not be
// path-joined into a fake in-package URI or flagged missing-part.
func resolveRel(partURI, rid string, relMap map[string]opc.RelationshipInfo, partSet map[string]bool) (uri, reason string, external bool) {
	rel, ok := relMap[rid]
	if !ok {
		return "", "dangling-rel:" + rid, false
	}
	if strings.EqualFold(rel.TargetMode, "External") {
		return rel.Target, "", true
	}
	target := opc.ResolveRelationshipTarget(partURI, rel.Target)
	if !partSet[target] {
		return target, "missing-part:" + target, false
	}
	return target, "", false
}

func findP14Media(nvPr *etree.Element) *etree.Element {
	extLst := xmlx.FindChild(nvPr, ns.NsP, "extLst")
	if extLst == nil {
		return nil
	}
	for _, ext := range xmlx.FindChildren(extLst, ns.NsP, "ext") {
		if m := xmlx.FindChild(ext, ns.Np14, "media"); m != nil {
			return m
		}
	}
	return nil
}

// hasClickToPlay reports whether the timing tree contains a p:cmd type="call"
// with a cmd starting with "playFrom" targeting spid.
//
// spec-grounded; PowerPoint-render unconfirmed: the click-to-play trigger is a
// CT_TLCommandBehavior p:cmd with type="call". The exact "playFrom(...)" string
// is matched by prefix to tolerate the unverified argument form pending a golden.
func hasClickToPlay(timing *etree.Element, spid int) bool {
	if timing == nil || spid == 0 {
		return false
	}
	for _, cmd := range xmlx.FindDescendants(timing, ns.NsP, "cmd") {
		if t, _ := xmlx.GetAttr(cmd, "type"); t != "call" {
			continue
		}
		if c, _ := xmlx.GetAttr(cmd, "cmd"); !strings.HasPrefix(c, "playFrom") {
			continue
		}
		spTgt := findSpTgt(cmd)
		if spTgt == nil {
			continue
		}
		if v, ok := parseIntAttr(spTgt, "spid"); ok && v == spid {
			return true
		}
	}
	return false
}

// rAttr returns the value of an r:-namespaced attribute (e.g. r:embed, r:link),
// matching the prefix-based handling used in images.go.
func rAttr(elem *etree.Element, local string) string {
	for _, attr := range elem.Attr {
		if attr.Key == local && attr.Space == "r" {
			return attr.Value
		}
	}
	v, _ := xmlx.GetAttrNS(elem, ns.NsR, local)
	return v
}

func parseIntAttr(elem *etree.Element, name string) (int, bool) {
	v, ok := xmlx.GetAttr(elem, name)
	if !ok {
		return 0, false
	}
	n, err := strconv.Atoi(v)
	if err != nil {
		return 0, false
	}
	return n, true
}
