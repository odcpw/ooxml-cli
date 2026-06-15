package mutate

import (
	"path/filepath"
	"strconv"
	"strings"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
)

// openAnimMutateFixture opens an in-memory PPTX fixture for mutation. Changes are
// applied to the in-memory package and are not persisted to disk.
func openAnimMutateFixture(t *testing.T, name string) *opc.Package {
	t.Helper()
	path := filepath.Join("..", "..", "..", "testdata", "pptx", name, "presentation.pptx")
	pkg, err := opc.Open(path)
	if err != nil {
		t.Fatalf("failed to open fixture %s: %v", name, err)
	}
	t.Cleanup(func() { pkg.Close() })
	return pkg
}

func slideRef(t *testing.T, pkg *opc.Package, n int) *inspect.SlideRef {
	t.Helper()
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("parse presentation: %v", err)
	}
	if n < 1 || n > len(graph.Slides) {
		t.Fatalf("slide %d out of range (have %d)", n, len(graph.Slides))
	}
	ref := graph.Slides[n-1]
	return &ref
}

func slideXML(t *testing.T, pkg *opc.Package, uri string) string {
	t.Helper()
	doc, err := pkg.ReadXMLPart(uri)
	if err != nil {
		t.Fatalf("read %s: %v", uri, err)
	}
	doc.IndentTabs()
	s, err := doc.WriteToString()
	if err != nil {
		t.Fatalf("serialize %s: %v", uri, err)
	}
	return s
}

// injectMotionTiming installs a hand-authored p:timing tree carrying an
// UNSUPPORTED motion-path effect (p:animMotion, presetClass=path) on the slide.
// The marker attribute lets the preserve test prove the node survives byte-intact.
func injectMotionTiming(t *testing.T, pkg *opc.Package, uri string, spid int) {
	t.Helper()
	doc, err := pkg.ReadXMLPart(uri)
	if err != nil {
		t.Fatalf("read %s: %v", uri, err)
	}
	root := doc.Root()
	timingXML := `<p:timing xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:tnLst>
    <p:par>
      <p:cTn id="1" dur="indefinite" restart="never" nodeType="tmRoot">
        <p:childTnLst>
          <p:seq concurrent="1" nextAc="seek">
            <p:cTn id="2" dur="indefinite" nodeType="mainSeq">
              <p:childTnLst>
                <p:par>
                  <p:cTn id="50" fill="hold" nodeType="clickEffect">
                    <p:stCondLst><p:cond delay="indefinite"/></p:stCondLst>
                    <p:childTnLst>
                      <p:par>
                        <p:cTn id="51" presetID="0" presetClass="path" presetSubtype="0" fill="hold" nodeType="clickEffect" data-preserve-marker="motion-path-keep">
                          <p:stCondLst><p:cond delay="0"/></p:stCondLst>
                          <p:childTnLst>
                            <p:animMotion origin="layout" path="M 0 0 L 0.5 0.5 E" pathEditMode="relative">
                              <p:cBhvr>
                                <p:cTn id="52" dur="2000" fill="hold"/>
                                <p:tgtEl><p:spTgt spid="` + strconv.Itoa(spid) + `"/></p:tgtEl>
                                <p:attrNameLst><p:attrName>ppt_x</p:attrName><p:attrName>ppt_y</p:attrName></p:attrNameLst>
                              </p:cBhvr>
                            </p:animMotion>
                          </p:childTnLst>
                        </p:cTn>
                      </p:par>
                    </p:childTnLst>
                  </p:cTn>
                </p:par>
              </p:childTnLst>
            </p:cTn>
            <p:prevCondLst><p:cond evt="onPrev" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst>
            <p:nextCondLst><p:cond evt="onNext" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst>
          </p:seq>
        </p:childTnLst>
      </p:cTn>
    </p:par>
  </p:tnLst>
</p:timing>`
	td := etree.NewDocument()
	if err := td.ReadFromString(timingXML); err != nil {
		t.Fatalf("parse injected timing: %v", err)
	}
	// Insert after cSld/clrMapOvr (CT_Slide order), before extLst.
	appendTimingInOrder(root, td.Root().Copy())
	if err := pkg.ReplaceXMLPart(uri, doc); err != nil {
		t.Fatalf("write injected timing: %v", err)
	}
}

func appendTimingInOrder(root, timing *etree.Element) {
	rank := slideRootChildRank("timing")
	for _, existing := range root.ChildElements() {
		if slideRootChildRank(localTag(existing.Tag)) > rank {
			root.InsertChildAt(existing.Index(), timing)
			return
		}
	}
	root.AddChild(timing)
}

// ---------------------------------------------------------------------------
// Tier A: appear end-to-end (get-or-create timing, ID alloc, insertion, readback)
// ---------------------------------------------------------------------------

func TestAddAnimation_AppearCreatesTiming(t *testing.T) {
	pkg := openAnimMutateFixture(t, "title-content")
	ref := slideRef(t, pkg, 1)

	res, err := AddAnimation(&AddAnimationRequest{
		Package:  pkg,
		SlideRef: ref,
		Selector: &selectors.ShapeIDSelector{ID: 2},
		Effect:   "appear",
		Start:    "onClick",
	})
	if err != nil {
		t.Fatalf("add appear: %v", err)
	}
	if !res.CreatedTiming {
		t.Fatal("expected timing to be created on a no-timing slide")
	}
	if res.ShapeID != 2 || res.ShapeName == "" {
		t.Fatalf("unexpected target: %+v", res)
	}
	if len(res.AddedEffectIDs) != 1 {
		t.Fatalf("expected 1 effect id, got %v", res.AddedEffectIDs)
	}

	// Read back through the inspector.
	rep, err := inspect.ReadAnimations(pkg)
	if err != nil {
		t.Fatalf("readback: %v", err)
	}
	s1 := findReportSlide(t, rep, 1)
	if !s1.HasTiming || len(s1.Effects) != 1 {
		t.Fatalf("expected one effect with timing: %+v", s1)
	}
	e := s1.Effects[0]
	if e.EffectKind != "appear" || !e.Supported || e.Spid != 2 {
		t.Fatalf("unexpected effect: %+v", e)
	}
	if e.EffectID != res.AddedEffectIDs[0] {
		t.Fatalf("effect id mismatch: readback %d vs result %d", e.EffectID, res.AddedEffectIDs[0])
	}
}

// TestAddAnimation_TimingInsertedInSchemaOrder confirms p:timing is a sibling of
// p:cSld inserted after clrMapOvr and before any extLst.
func TestAddAnimation_TimingInsertedInSchemaOrder(t *testing.T) {
	pkg := openAnimMutateFixture(t, "title-content")
	ref := slideRef(t, pkg, 1)
	if _, err := AddAnimation(&AddAnimationRequest{
		Package: pkg, SlideRef: ref, Selector: &selectors.ShapeIDSelector{ID: 2}, Effect: "appear",
	}); err != nil {
		t.Fatalf("add: %v", err)
	}
	doc, _ := pkg.ReadXMLPart(ref.PartURI)
	var order []string
	for _, c := range doc.Root().ChildElements() {
		order = append(order, localTag(c.Tag))
	}
	cSldIdx, timingIdx := indexOf(order, "cSld"), indexOf(order, "timing")
	if cSldIdx < 0 || timingIdx < 0 || timingIdx <= cSldIdx {
		t.Fatalf("timing not after cSld: %v", order)
	}
	if cm := indexOf(order, "clrMapOvr"); cm >= 0 && timingIdx <= cm {
		t.Fatalf("timing must follow clrMapOvr: %v", order)
	}
	if ext := indexOf(order, "extLst"); ext >= 0 && timingIdx >= ext {
		t.Fatalf("timing must precede extLst: %v", order)
	}
}

// ---------------------------------------------------------------------------
// Preserve-unknown: the motion-path node survives add/remove/reorder byte-intact.
// ---------------------------------------------------------------------------

func TestAddAnimation_PreservesUnknownTimingNode(t *testing.T) {
	pkg := openAnimMutateFixture(t, "title-content")
	ref := slideRef(t, pkg, 1)
	injectMotionTiming(t, pkg, ref.PartURI, 2)

	before := slideXML(t, pkg, ref.PartURI)
	if !strings.Contains(before, "motion-path-keep") {
		t.Fatal("setup: motion marker missing")
	}
	motionFragment := extractFragment(before, "<p:animMotion", "</p:animMotion>")
	if motionFragment == "" {
		t.Fatal("setup: could not isolate animMotion fragment")
	}

	res, err := AddAnimation(&AddAnimationRequest{
		Package: pkg, SlideRef: ref, Selector: &selectors.ShapeIDSelector{ID: 2}, Effect: "appear",
	})
	if err != nil {
		t.Fatalf("add over existing timing: %v", err)
	}
	if res.CreatedTiming {
		t.Fatal("must NOT recreate an existing timing tree")
	}
	// New id must not collide with the preserved nodes (50,51,52).
	for _, id := range res.AddedEffectIDs {
		if id <= 52 {
			t.Fatalf("new effect id %d collides with preserved node ids (<=52)", id)
		}
	}

	after := slideXML(t, pkg, ref.PartURI)
	if !strings.Contains(after, motionFragment) {
		t.Fatalf("animMotion fragment not preserved byte-for-structure after add\n--- want fragment ---\n%s", motionFragment)
	}

	// Now the reader should report 2 effects: the appear + the preserved unsupported.
	rep, _ := inspect.ReadAnimations(pkg)
	s1 := findReportSlide(t, rep, 1)
	if len(s1.Effects) != 2 {
		t.Fatalf("expected appear + preserved motion, got %d effects", len(s1.Effects))
	}
	if s1.UnsupportedCount != 1 {
		t.Fatalf("expected 1 unsupported, got %d", s1.UnsupportedCount)
	}

	// remove must refuse the unsupported motion-path effect id (51).
	_, err = RemoveAnimation(&RemoveAnimationRequest{Package: pkg, SlideRef: ref, EffectID: 51})
	var tnf *TargetNotFoundError
	if err == nil || !asTargetNotFound(err, &tnf) {
		t.Fatalf("expected TargetNotFoundError refusing to delete motion path, got %v", err)
	}
	stillThere := slideXML(t, pkg, ref.PartURI)
	if !strings.Contains(stillThere, motionFragment) {
		t.Fatal("motion path deleted by refused remove")
	}
}

// ---------------------------------------------------------------------------
// ID allocation across the whole timing subtree (max+1, not just mainSeq).
// ---------------------------------------------------------------------------

func TestAddAnimation_IDAllocationMaxPlusOne(t *testing.T) {
	pkg := openAnimMutateFixture(t, "title-content")
	ref := slideRef(t, pkg, 1)
	injectMotionTiming(t, pkg, ref.PartURI, 2) // ids up to 52 present.

	res, err := AddAnimation(&AddAnimationRequest{
		Package: pkg, SlideRef: ref, Selector: &selectors.ShapeIDSelector{ID: 2}, Effect: "wipe", Direction: "up",
	})
	if err != nil {
		t.Fatalf("add wipe: %v", err)
	}
	// Click step gets 53, effect 54, set behavior 55, animEffect behavior 56.
	doc, _ := pkg.ReadXMLPart(ref.PartURI)
	ids := map[int]int{}
	for _, cTn := range doc.Root().FindElements(".//cTn") {
		if n, err := strconv.Atoi(cTn.SelectAttrValue("id", "")); err == nil {
			ids[n]++
		}
	}
	for id, count := range ids {
		if count != 1 {
			t.Fatalf("duplicate cTn id %d (count %d)", id, count)
		}
	}
	if res.AddedEffectIDs[0] <= 52 {
		t.Fatalf("effect id %d not allocated above existing max 52", res.AddedEffectIDs[0])
	}
}

// ---------------------------------------------------------------------------
// reorder preserves unknown sibling nodes; rejects non-permutations.
// ---------------------------------------------------------------------------

func TestReorderAnimations_Permutation(t *testing.T) {
	pkg := openAnimMutateFixture(t, "title-content")
	ref := slideRef(t, pkg, 1)

	// Author three click steps.
	for _, eff := range []string{"appear", "wipe", "fade"} {
		if _, err := AddAnimation(&AddAnimationRequest{
			Package: pkg, SlideRef: ref, Selector: &selectors.ShapeIDSelector{ID: 2}, Effect: eff, Direction: "up",
		}); err != nil {
			t.Fatalf("add %s: %v", eff, err)
		}
	}
	rep, _ := inspect.ReadAnimations(pkg)
	s1 := findReportSlide(t, rep, 1)
	var clickIDs []int
	for _, e := range s1.Effects {
		clickIDs = append(clickIDs, e.ClickStepID)
	}
	if len(clickIDs) != 3 {
		t.Fatalf("expected 3 click steps, got %d", len(clickIDs))
	}

	// Reverse order.
	reversed := []int{clickIDs[2], clickIDs[1], clickIDs[0]}
	if _, err := ReorderAnimations(&ReorderAnimationsRequest{Package: pkg, SlideRef: ref, Order: reversed}); err != nil {
		t.Fatalf("reorder: %v", err)
	}
	rep2, _ := inspect.ReadAnimations(pkg)
	s1b := findReportSlide(t, rep2, 1)
	got := []string{s1b.Effects[0].EffectKind, s1b.Effects[1].EffectKind, s1b.Effects[2].EffectKind}
	want := []string{"fade", "wipe", "appear"}
	for i := range want {
		if got[i] != want[i] {
			t.Fatalf("reorder result %v, want %v", got, want)
		}
	}

	// Non-permutation rejected.
	if _, err := ReorderAnimations(&ReorderAnimationsRequest{Package: pkg, SlideRef: ref, Order: []int{clickIDs[0]}}); err == nil {
		t.Fatal("expected error for short order list")
	}
	if _, err := ReorderAnimations(&ReorderAnimationsRequest{Package: pkg, SlideRef: ref, Order: []int{9999, clickIDs[0], clickIDs[1]}}); err == nil {
		t.Fatal("expected error for unknown id in order")
	}
}

// ---------------------------------------------------------------------------
// prune-stale removes only stale effects, leaving valid ones; dry-run no-writes.
// ---------------------------------------------------------------------------

func TestPruneStale_RemovesMissingShapeEffect(t *testing.T) {
	pkg := openAnimMutateFixture(t, "title-content")
	ref := slideRef(t, pkg, 1)

	// Author a valid appear on shape 2.
	if _, err := AddAnimation(&AddAnimationRequest{
		Package: pkg, SlideRef: ref, Selector: &selectors.ShapeIDSelector{ID: 2}, Effect: "appear",
	}); err != nil {
		t.Fatalf("add valid: %v", err)
	}
	// Author an effect targeting a shape that does not exist (stale missing-shape).
	if _, err := AddAnimation(&AddAnimationRequest{
		Package: pkg, SlideRef: ref, Selector: &selectors.ShapeIDSelector{ID: 2}, Effect: "fade",
	}); err != nil {
		t.Fatalf("add second: %v", err)
	}
	// Rewrite the second effect's spTgt to a missing shape id by editing XML.
	mutateSpid(t, pkg, ref.PartURI, "fade", 99)

	rep, _ := inspect.ReadAnimations(pkg)
	s1 := findReportSlide(t, rep, 1)
	staleCount := 0
	for _, e := range s1.Effects {
		if e.Stale {
			staleCount++
		}
	}
	if staleCount != 1 {
		t.Fatalf("expected exactly 1 stale effect, got %d (%+v)", staleCount, s1.Effects)
	}

	// Dry-run reports the candidate but does not write.
	beforeDry := slideXML(t, pkg, ref.PartURI)
	dry, err := PruneStale(&PruneStaleRequest{Package: pkg, SlideRefs: []inspect.SlideRef{*ref}, DryRun: true})
	if err != nil {
		t.Fatalf("dry-run prune: %v", err)
	}
	if len(dry.Pruned) != 1 || dry.Pruned[0].StaleReason != "missing-shape" {
		t.Fatalf("dry-run should report 1 missing-shape candidate: %+v", dry.Pruned)
	}
	if slideXML(t, pkg, ref.PartURI) != beforeDry {
		t.Fatal("dry-run must not modify the slide")
	}

	// Real prune removes exactly the stale effect.
	if _, err := PruneStale(&PruneStaleRequest{Package: pkg, SlideRefs: []inspect.SlideRef{*ref}}); err != nil {
		t.Fatalf("prune: %v", err)
	}
	rep2, _ := inspect.ReadAnimations(pkg)
	s1b := findReportSlide(t, rep2, 1)
	if len(s1b.Effects) != 1 || s1b.Effects[0].EffectKind != "appear" || s1b.Effects[0].Stale {
		t.Fatalf("after prune expected only the valid appear: %+v", s1b.Effects)
	}
}

// TestPruneStale_PreservesStaleUnsupported confirms a STALE UNSUPPORTED effect (a
// motion path targeting a deleted shape) is neither pruned nor counted: the
// preserve-unknown contract overrides staleness for nodes we do not author.
func TestPruneStale_PreservesStaleUnsupported(t *testing.T) {
	pkg := openAnimMutateFixture(t, "title-content")
	ref := slideRef(t, pkg, 1)
	// Motion path targets shape id 99, which does not exist -> stale missing-shape.
	injectMotionTiming(t, pkg, ref.PartURI, 99)

	rep := mustReadAnimations(t, pkg)
	s1 := findReportSlide(t, rep, 1)
	if len(s1.Effects) != 1 || s1.Effects[0].Supported || !s1.Effects[0].Stale {
		t.Fatalf("setup: expected one stale unsupported effect: %+v", s1.Effects)
	}

	before := slideXML(t, pkg, ref.PartURI)
	res, err := PruneStale(&PruneStaleRequest{Package: pkg, SlideRefs: []inspect.SlideRef{*ref}})
	if err != nil {
		t.Fatalf("prune: %v", err)
	}
	if len(res.Pruned) != 0 {
		t.Fatalf("stale UNSUPPORTED effect must not be reported as pruned: %+v", res.Pruned)
	}
	if slideXML(t, pkg, ref.PartURI) != before {
		t.Fatal("stale unsupported motion path must be preserved byte-intact by prune-stale")
	}
}

func mustReadAnimations(t *testing.T, pkg *opc.Package) *inspect.AnimationsReport {
	t.Helper()
	rep, err := inspect.ReadAnimations(pkg)
	if err != nil {
		t.Fatalf("read animations: %v", err)
	}
	return rep
}

// ---------------------------------------------------------------------------
// Guards.
// ---------------------------------------------------------------------------

func TestAddAnimation_ExpectShapeNameGuard(t *testing.T) {
	pkg := openAnimMutateFixture(t, "title-content")
	ref := slideRef(t, pkg, 1)
	_, err := AddAnimation(&AddAnimationRequest{
		Package: pkg, SlideRef: ref, Selector: &selectors.ShapeIDSelector{ID: 2}, Effect: "appear",
		ExpectShapeName: "Definitely Not The Name",
	})
	if err == nil || !strings.Contains(err.Error(), "shape name guard") {
		t.Fatalf("expected shape-name guard failure, got %v", err)
	}
}

// ---------------------------------------------------------------------------
// Finding 2: remove/prune ownership gate uses the SAME classifier as the reader.
// An entr cTn whose behaviors do not match a supported pattern (p:set on a non
// style.visibility attr, p:anim on a non ppt_x/ppt_y attr) is reader-unsupported
// and MUST be refused by remove (never silently deleting preserved third-party XML).
// ---------------------------------------------------------------------------

// injectUnsupportedEntrTiming installs a timing tree with three click steps under
// the mainSeq: (60) an entr cTn whose sole behavior is p:set on style.color,
// (62) an entr cTn whose sole behavior is p:anim on ppt_w, and (70) a GENUINE
// appear (p:set on style.visibility). The first two are reader-unsupported; the
// third is owned/removable.
func injectUnsupportedEntrTiming(t *testing.T, pkg *opc.Package, uri string, spid int) {
	t.Helper()
	doc, err := pkg.ReadXMLPart(uri)
	if err != nil {
		t.Fatalf("read %s: %v", uri, err)
	}
	root := doc.Root()
	sp := strconv.Itoa(spid)
	timingXML := `<p:timing xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:tnLst>
    <p:par>
      <p:cTn id="1" dur="indefinite" restart="never" nodeType="tmRoot">
        <p:childTnLst>
          <p:seq concurrent="1" nextAc="seek">
            <p:cTn id="2" dur="indefinite" nodeType="mainSeq">
              <p:childTnLst>
                <p:par>
                  <p:cTn id="60" presetID="1" presetClass="entr" presetSubtype="0" fill="hold" grpId="0" nodeType="clickEffect" data-preserve-marker="color-set-keep">
                    <p:stCondLst><p:cond delay="0"/></p:stCondLst>
                    <p:childTnLst>
                      <p:set>
                        <p:cBhvr>
                          <p:cTn id="61" dur="1" fill="hold"/>
                          <p:tgtEl><p:spTgt spid="` + sp + `"/></p:tgtEl>
                          <p:attrNameLst><p:attrName>style.color</p:attrName></p:attrNameLst>
                        </p:cBhvr>
                        <p:to><p:strVal val="#FF0000"/></p:to>
                      </p:set>
                    </p:childTnLst>
                  </p:cTn>
                </p:par>
                <p:par>
                  <p:cTn id="62" presetID="2" presetClass="entr" presetSubtype="0" fill="hold" grpId="0" nodeType="clickEffect" data-preserve-marker="width-anim-keep">
                    <p:stCondLst><p:cond delay="0"/></p:stCondLst>
                    <p:childTnLst>
                      <p:anim calcmode="lin" valueType="num">
                        <p:cBhvr additive="base">
                          <p:cTn id="63" dur="500" fill="hold"/>
                          <p:tgtEl><p:spTgt spid="` + sp + `"/></p:tgtEl>
                          <p:attrNameLst><p:attrName>ppt_w</p:attrName></p:attrNameLst>
                        </p:cBhvr>
                        <p:tavLst><p:tav tm="0"><p:val><p:strVal val="0"/></p:val></p:tav></p:tavLst>
                      </p:anim>
                    </p:childTnLst>
                  </p:cTn>
                </p:par>
                <p:par>
                  <p:cTn id="70" presetID="1" presetClass="entr" presetSubtype="0" fill="hold" grpId="0" nodeType="clickEffect">
                    <p:stCondLst><p:cond delay="0"/></p:stCondLst>
                    <p:childTnLst>
                      <p:set>
                        <p:cBhvr>
                          <p:cTn id="71" dur="1" fill="hold"/>
                          <p:tgtEl><p:spTgt spid="` + sp + `"/></p:tgtEl>
                          <p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst>
                        </p:cBhvr>
                        <p:to><p:strVal val="visible"/></p:to>
                      </p:set>
                    </p:childTnLst>
                  </p:cTn>
                </p:par>
              </p:childTnLst>
            </p:cTn>
            <p:prevCondLst><p:cond evt="onPrev" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst>
            <p:nextCondLst><p:cond evt="onNext" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst>
          </p:seq>
        </p:childTnLst>
      </p:cTn>
    </p:par>
  </p:tnLst>
</p:timing>`
	td := etree.NewDocument()
	if err := td.ReadFromString(timingXML); err != nil {
		t.Fatalf("parse injected timing: %v", err)
	}
	appendTimingInOrder(root, td.Root().Copy())
	if err := pkg.ReplaceXMLPart(uri, doc); err != nil {
		t.Fatalf("write injected timing: %v", err)
	}
}

func TestRemoveAnimation_RefusesReaderUnsupportedEntrance(t *testing.T) {
	pkg := openAnimMutateFixture(t, "title-content")
	ref := slideRef(t, pkg, 1)
	injectUnsupportedEntrTiming(t, pkg, ref.PartURI, 2)

	// The reader must mark the two unsupported-but-entr effects supported=false.
	rep, _ := inspect.ReadAnimations(pkg)
	s1 := findReportSlide(t, rep, 1)
	supportedByID := map[int]bool{}
	for _, e := range s1.Effects {
		supportedByID[e.EffectID] = e.Supported
	}
	if supportedByID[60] {
		t.Fatal("reader marked the style.color p:set effect (id 60) supported; expected unsupported")
	}
	if supportedByID[62] {
		t.Fatal("reader marked the ppt_w p:anim effect (id 62) supported; expected unsupported")
	}
	if !supportedByID[70] {
		t.Fatal("reader marked the genuine appear (id 70) unsupported; expected supported")
	}

	before := slideXML(t, pkg, ref.PartURI)
	colorFrag := extractFragment(before, `data-preserve-marker="color-set-keep"`, "</p:cTn>")
	widthFrag := extractFragment(before, `data-preserve-marker="width-anim-keep"`, "</p:cTn>")
	if colorFrag == "" || widthFrag == "" {
		t.Fatal("setup: could not isolate the unsupported fragments")
	}

	// remove must REFUSE both unsupported ids and leave the XML untouched.
	for _, id := range []int{60, 62} {
		_, err := RemoveAnimation(&RemoveAnimationRequest{Package: pkg, SlideRef: ref, EffectID: id})
		var tnf *TargetNotFoundError
		if err == nil || !asTargetNotFound(err, &tnf) {
			t.Fatalf("remove id %d: expected TargetNotFoundError, got %v", id, err)
		}
	}
	after := slideXML(t, pkg, ref.PartURI)
	if !strings.Contains(after, colorFrag) {
		t.Error("style.color p:set effect deleted by refused remove (preserved XML lost)")
	}
	if !strings.Contains(after, widthFrag) {
		t.Error("ppt_w p:anim effect deleted by refused remove (preserved XML lost)")
	}

	// The genuine appear (id 70) IS removable.
	if _, err := RemoveAnimation(&RemoveAnimationRequest{Package: pkg, SlideRef: ref, EffectID: 70}); err != nil {
		t.Fatalf("remove genuine appear (id 70): unexpected error %v", err)
	}
	final := slideXML(t, pkg, ref.PartURI)
	if strings.Contains(final, "style.visibility") {
		t.Error("genuine appear not removed")
	}
	// The two preserved fragments still survive the appear removal.
	if !strings.Contains(final, colorFrag) || !strings.Contains(final, widthFrag) {
		t.Error("preserved unsupported effects lost after removing the appear")
	}
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

func findReportSlide(t *testing.T, rep *inspect.AnimationsReport, n int) inspect.AnimationsSlideInfo {
	t.Helper()
	for _, s := range rep.Slides {
		if s.Slide == n {
			return s
		}
	}
	t.Fatalf("slide %d not in report", n)
	return inspect.AnimationsSlideInfo{}
}

func indexOf(ss []string, want string) int {
	for i, s := range ss {
		if s == want {
			return i
		}
	}
	return -1
}

func extractFragment(s, start, end string) string {
	i := strings.Index(s, start)
	if i < 0 {
		return ""
	}
	j := strings.Index(s[i:], end)
	if j < 0 {
		return ""
	}
	return s[i : i+j+len(end)]
}

func asTargetNotFound(err error, target **TargetNotFoundError) bool {
	if e, ok := err.(*TargetNotFoundError); ok {
		*target = e
		return true
	}
	return false
}

// mutateSpid finds the effect of the given kind and rewrites its behavior spTgt
// spid to newSpid (to synthesize a stale missing-shape target).
func mutateSpid(t *testing.T, pkg *opc.Package, uri, kind string, newSpid int) {
	t.Helper()
	doc, err := pkg.ReadXMLPart(uri)
	if err != nil {
		t.Fatalf("read: %v", err)
	}
	// Locate the effect cTn whose filter/behavior matches the kind, then rewrite
	// every spTgt under it.
	for _, cTn := range doc.Root().FindElements(".//cTn") {
		if cTn.SelectAttrValue("presetClass", "") != "entr" {
			continue
		}
		isFade := false
		for _, ae := range cTn.FindElements(".//animEffect") {
			if ae.SelectAttrValue("filter", "") == "fade" {
				isFade = true
			}
		}
		if (kind == "fade") != isFade {
			continue
		}
		for _, sp := range cTn.FindElements(".//spTgt") {
			sp.CreateAttr("spid", strconv.Itoa(newSpid))
		}
		break
	}
	if err := pkg.ReplaceXMLPart(uri, doc); err != nil {
		t.Fatalf("write: %v", err)
	}
}
