package mutate

import (
	"strings"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	ns "github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
	"github.com/ooxml-cli/ooxml-cli/pkg/validate"
)

// fakeMediaBytes is opaque to the OOXML round-trip: only part existence, rel
// resolution, content type, and XML schema order are validated, so arbitrary
// bytes with a media extension suffice for these tests.
var fakeMediaBytes = []byte("fake-mp4-bytes-not-a-real-clip-but-opaque-to-opc")

func openMediaFixture(t *testing.T, name string) *opc.Package {
	t.Helper()
	pkg, err := opc.Open("../../../testdata/pptx/" + name + "/presentation.pptx")
	if err != nil {
		t.Fatalf("open fixture %s: %v", name, err)
	}
	return pkg
}

func assertValidateClean(t *testing.T, pkg *opc.Package) {
	t.Helper()
	diags, err := validate.ValidatePackage(pkg)
	if err != nil {
		t.Fatalf("validate: %v", err)
	}
	for _, d := range diags {
		if d.Severity == result.Error {
			t.Errorf("validate error: [%s] %s", d.Code, d.Message)
		}
	}
}

func TestInsertMedia_CreatesTimingSkeletonAndPic(t *testing.T) {
	pkg := openMediaFixture(t, "minimal-title")
	defer pkg.Close()
	ref := slideRef(t, pkg, 1)

	res, err := InsertMedia(&InsertMediaRequest{
		Package:          pkg,
		SlideRef:         ref,
		MediaData:        fakeMediaBytes,
		MediaContentType: "video/mp4",
		MediaExt:         ".mp4",
		Kind:             MediaKindVideo,
		Name:             "Intro Clip",
		X:                100, Y: 200, CX: 3000000, CY: 2000000,
		PlayTrigger: "click",
		Volume:      80,
	})
	if err != nil {
		t.Fatalf("InsertMedia: %v", err)
	}

	// Media part + poster part exist.
	if !packagePartExists(pkg, res.MediaPartURI) {
		t.Errorf("media part %q not added", res.MediaPartURI)
	}
	if !strings.HasPrefix(res.MediaPartURI, "/ppt/media/media") || !strings.HasSuffix(res.MediaPartURI, ".mp4") {
		t.Errorf("media part name unexpected: %q", res.MediaPartURI)
	}
	if !packagePartExists(pkg, res.PosterPartURI) {
		t.Errorf("poster part %q not added", res.PosterPartURI)
	}
	if !res.PosterSynthesized {
		t.Errorf("expected synthesized poster when none supplied")
	}
	if pkg.GetContentType(res.MediaPartURI) != "video/mp4" {
		t.Errorf("media content type = %q, want video/mp4", pkg.GetContentType(res.MediaPartURI))
	}

	// Three rels present, resolvable, with correct types.
	rels := pkg.ListRelationships(ref.PartURI)
	relByID := map[string]opc.RelationshipInfo{}
	for _, r := range rels {
		relByID[r.ID] = r
	}
	checkRel := func(id, wantType, wantTarget string) {
		r, ok := relByID[id]
		if !ok {
			t.Fatalf("rel %s missing", id)
		}
		if r.Type != wantType {
			t.Errorf("rel %s type = %q, want %q", id, r.Type, wantType)
		}
		resolved := opc.ResolveRelationshipTarget(ref.PartURI, r.Target)
		if resolved != wantTarget {
			t.Errorf("rel %s resolves to %q, want %q", id, resolved, wantTarget)
		}
		if !packagePartExists(pkg, resolved) {
			t.Errorf("rel %s target part %q missing", id, resolved)
		}
	}
	checkRel(res.MediaRelID, relTypeMedia, res.MediaPartURI)
	checkRel(res.AVRelID, relTypeVideo, res.MediaPartURI)
	checkRel(res.PosterRelID, relTypeImage, res.PosterPartURI)

	// Inspect the written slide XML.
	doc, err := pkg.ReadXMLPart(ref.PartURI)
	if err != nil {
		t.Fatalf("re-read slide: %v", err)
	}
	root := doc.Root()
	pic := findMediaPicBySpid(root, res.ShapeID)
	if pic == nil {
		t.Fatal("media pic not found in spTree")
	}
	// Dual representation.
	nvPr := xmlx.FindChild(xmlx.FindChild(pic, ns.NsP, "nvPicPr"), ns.NsP, "nvPr")
	if xmlx.FindChild(nvPr, ns.NsA, "videoFile") == nil {
		t.Error("a:videoFile missing")
	}
	if findP14MediaIn(nvPr) == nil {
		t.Error("p14:media embed missing")
	}
	// hlinkClick ppaction://media.
	cNvPr := xmlx.FindChild(xmlx.FindChild(pic, ns.NsP, "nvPicPr"), ns.NsP, "cNvPr")
	hlink := xmlx.FindChild(cNvPr, ns.NsA, "hlinkClick")
	if hlink == nil {
		t.Fatal("hlinkClick missing")
	}
	if a, _ := xmlx.GetAttr(hlink, "action"); a != "ppaction://media" {
		t.Errorf("hlink action = %q, want ppaction://media", a)
	}
	// Poster blip.
	blipFill := xmlx.FindChild(pic, ns.NsP, "blipFill")
	if xmlx.FindChild(blipFill, ns.NsA, "blip") == nil {
		t.Error("poster a:blip missing")
	}

	// Timing created in schema order (after cSld/clrMapOvr, before extLst) with a
	// passive media node targeting the spid.
	assertTimingSchemaOrder(t, root)
	timing := xmlx.FindChild(root, ns.NsP, "timing")
	if timing == nil {
		t.Fatal("p:timing not created")
	}
	node := findCMediaNodeForSpid(timing, res.ShapeID)
	if node == nil {
		t.Fatal("cMediaNode for spid not found")
	}
	if wrap := node.Parent(); localTag(wrap.Tag) != "video" {
		t.Errorf("media wrap tag = %q, want video", localTag(wrap.Tag))
	}
	if vol, _ := xmlx.GetAttr(node, "vol"); vol != "80000" {
		t.Errorf("cMediaNode vol = %q, want 80000", vol)
	}

	assertValidateClean(t, pkg)
}

func TestInsertMediaRejectsInvalidPosterPayloadWithoutMutation(t *testing.T) {
	pkg := openMediaFixture(t, "minimal-title")
	defer pkg.Close()
	ref := slideRef(t, pkg, 1)
	partsBefore := len(pkg.ListParts())

	_, err := InsertMedia(&InsertMediaRequest{
		Package:           pkg,
		SlideRef:          ref,
		MediaData:         fakeMediaBytes,
		MediaContentType:  "video/mp4",
		MediaExt:          ".mp4",
		Kind:              MediaKindVideo,
		PosterData:        []byte("not a png"),
		PosterContentType: "image/png",
		X:                 100, Y: 200, CX: 3000000, CY: 2000000,
	})
	if err == nil || !strings.Contains(err.Error(), "image payload does not match content type image/png") {
		t.Fatalf("expected poster payload mismatch error, got %v", err)
	}
	if got := len(pkg.ListParts()); got != partsBefore {
		t.Fatalf("insert with bad poster mutated package parts: before=%d after=%d", partsBefore, got)
	}
}

func TestInsertMedia_AudioKind(t *testing.T) {
	pkg := openMediaFixture(t, "minimal-title")
	defer pkg.Close()
	ref := slideRef(t, pkg, 1)

	res, err := InsertMedia(&InsertMediaRequest{
		Package:          pkg,
		SlideRef:         ref,
		MediaData:        fakeMediaBytes,
		MediaContentType: "audio/x-m4a",
		MediaExt:         ".m4a",
		Kind:             MediaKindAudio,
		X:                0, Y: 0, CX: 1000000, CY: 1000000,
		PlayTrigger: "click",
		Volume:      80,
	})
	if err != nil {
		t.Fatalf("InsertMedia: %v", err)
	}

	rels := pkg.ListRelationships(ref.PartURI)
	foundAudioRel := false
	for _, r := range rels {
		if r.ID == res.AVRelID && r.Type == relTypeAudio {
			foundAudioRel = true
		}
	}
	if !foundAudioRel {
		t.Error("audio rel type not set")
	}

	doc, _ := pkg.ReadXMLPart(ref.PartURI)
	root := doc.Root()
	pic := findMediaPicBySpid(root, res.ShapeID)
	nvPr := xmlx.FindChild(xmlx.FindChild(pic, ns.NsP, "nvPicPr"), ns.NsP, "nvPr")
	if xmlx.FindChild(nvPr, ns.NsA, "audioFile") == nil {
		t.Error("a:audioFile missing for audio kind")
	}
	timing := xmlx.FindChild(root, ns.NsP, "timing")
	node := findCMediaNodeForSpid(timing, res.ShapeID)
	if node == nil || localTag(node.Parent().Tag) != "audio" {
		t.Error("p:audio media node missing for audio kind")
	}
	assertValidateClean(t, pkg)
}

// TestInsertMedia_PreservesExistingTiming embeds media on a slide that already
// has a populated p:timing tree (entrance effects). The existing tnLst children
// and effect cTn ids must survive byte-for-structure, and the new media node id
// must be max+1 across the whole timing subtree (no collision).
func TestInsertMedia_PreservesExistingTiming(t *testing.T) {
	pkg := openMediaFixture(t, "animations-synthetic")
	defer pkg.Close()
	ref := slideRef(t, pkg, 1)

	// Snapshot the existing timing before mutation.
	before, _ := pkg.ReadXMLPart(ref.PartURI)
	beforeTiming := xmlx.FindChild(before.Root(), ns.NsP, "timing")
	if beforeTiming == nil {
		t.Skip("fixture slide1 has no p:timing; preserve test needs one")
	}
	beforeIDs := allCTnIDs(beforeTiming)
	beforeMainSeq := serializeFirst(t, before.Root(), "seq")

	res, err := InsertMedia(&InsertMediaRequest{
		Package:          pkg,
		SlideRef:         ref,
		MediaData:        fakeMediaBytes,
		MediaContentType: "video/mp4",
		MediaExt:         ".mp4",
		Kind:             MediaKindVideo,
		X:                0, Y: 0, CX: 1000000, CY: 1000000,
		PlayTrigger: "click",
		Volume:      80,
	})
	if err != nil {
		t.Fatalf("InsertMedia: %v", err)
	}

	after, _ := pkg.ReadXMLPart(ref.PartURI)
	afterTiming := xmlx.FindChild(after.Root(), ns.NsP, "timing")

	// Every pre-existing cTn id still present.
	afterIDSet := map[int]bool{}
	for _, id := range allCTnIDs(afterTiming) {
		afterIDSet[id] = true
	}
	for _, id := range beforeIDs {
		if !afterIDSet[id] {
			t.Errorf("pre-existing cTn id %d disappeared after media insert", id)
		}
	}

	// New media node id is strictly greater than every pre-existing id (max+1).
	node := findCMediaNodeForSpid(afterTiming, res.ShapeID)
	if node == nil {
		t.Fatal("media node not injected")
	}
	cTn := xmlx.FindChild(node, ns.NsP, "cTn")
	newID, _ := parseIntAttr(cTn, "id")
	maxBefore := 0
	for _, id := range beforeIDs {
		if id > maxBefore {
			maxBefore = id
		}
	}
	if newID <= maxBefore {
		t.Errorf("media node id %d not greater than max pre-existing id %d", newID, maxBefore)
	}

	// The mainSeq subtree (the entrance effects) is structurally unchanged: the
	// media node attaches under tmRoot's childTnLst, a SIBLING of the seq, not
	// inside it.
	afterMainSeq := serializeFirst(t, after.Root(), "seq")
	if beforeMainSeq != afterMainSeq {
		t.Errorf("mainSeq (entrance effects) was modified by media insert:\nBEFORE:\n%s\nAFTER:\n%s", beforeMainSeq, afterMainSeq)
	}

	assertValidateClean(t, pkg)
}

func TestReplaceMedia_PreservesGeometryAndTiming(t *testing.T) {
	pkg := openMediaFixture(t, "minimal-title")
	defer pkg.Close()
	ref := slideRef(t, pkg, 1)

	ins, err := InsertMedia(&InsertMediaRequest{
		Package:          pkg,
		SlideRef:         ref,
		MediaData:        fakeMediaBytes,
		MediaContentType: "video/mp4",
		MediaExt:         ".mp4",
		Kind:             MediaKindVideo,
		Name:             "Clip",
		X:                12345, Y: 67890, CX: 3000000, CY: 2000000,
		PlayTrigger: "click",
		Volume:      80,
	})
	if err != nil {
		t.Fatalf("InsertMedia: %v", err)
	}

	rep, err := ReplaceMedia(&ReplaceMediaRequest{
		Package:             pkg,
		SlideRef:            ref,
		Selector:            &selectors.ShapeIDSelector{ID: ins.ShapeID},
		NewMediaData:        []byte("a-different-fake-clip"),
		NewMediaContentType: "video/mp4",
		NewMediaExt:         ".mp4",
		NewKind:             MediaKindVideo,
		ExpectShapeName:     "Clip",
		ExpectKind:          MediaKindVideo,
	})
	if err != nil {
		t.Fatalf("ReplaceMedia: %v", err)
	}
	if rep.ShapeID != ins.ShapeID {
		t.Errorf("replace shape id = %d, want %d", rep.ShapeID, ins.ShapeID)
	}

	// Geometry preserved.
	doc, _ := pkg.ReadXMLPart(ref.PartURI)
	pic := findMediaPicBySpid(doc.Root(), ins.ShapeID)
	off := pic.FindElement(".//off")
	if off == nil || off.SelectAttrValue("x", "") != "12345" {
		t.Errorf("geometry x not preserved: %v", off)
	}
	assertValidateClean(t, pkg)
}

func TestReplaceMediaRejectsInvalidPosterPayloadBeforeMediaMutation(t *testing.T) {
	pkg := openMediaFixture(t, "minimal-title")
	defer pkg.Close()
	ref := slideRef(t, pkg, 1)

	ins, err := InsertMedia(&InsertMediaRequest{
		Package:          pkg,
		SlideRef:         ref,
		MediaData:        fakeMediaBytes,
		MediaContentType: "video/mp4",
		MediaExt:         ".mp4",
		Kind:             MediaKindVideo,
		X:                12345, Y: 67890, CX: 3000000, CY: 2000000,
	})
	if err != nil {
		t.Fatalf("InsertMedia: %v", err)
	}
	partsBefore := len(pkg.ListParts())
	mediaBefore, err := pkg.ReadRawPart(ins.MediaPartURI)
	if err != nil {
		t.Fatalf("read inserted media: %v", err)
	}

	_, err = ReplaceMedia(&ReplaceMediaRequest{
		Package:              pkg,
		SlideRef:             ref,
		Selector:             &selectors.ShapeIDSelector{ID: ins.ShapeID},
		NewMediaData:         []byte("new clip bytes"),
		NewMediaContentType:  "video/mp4",
		NewMediaExt:          ".mp4",
		NewKind:              MediaKindVideo,
		NewPosterData:        []byte("not a png"),
		NewPosterContentType: "image/png",
	})
	if err == nil || !strings.Contains(err.Error(), "image payload does not match content type image/png") {
		t.Fatalf("expected poster payload mismatch error, got %v", err)
	}
	if got := len(pkg.ListParts()); got != partsBefore {
		t.Fatalf("replace with bad poster mutated package parts: before=%d after=%d", partsBefore, got)
	}
	mediaAfter, err := pkg.ReadRawPart(ins.MediaPartURI)
	if err != nil {
		t.Fatalf("read media after failed replace: %v", err)
	}
	if string(mediaAfter) != string(mediaBefore) {
		t.Fatal("replace with bad poster changed media bytes before returning an error")
	}
}

func TestReplaceMedia_KindFlip(t *testing.T) {
	pkg := openMediaFixture(t, "minimal-title")
	defer pkg.Close()
	ref := slideRef(t, pkg, 1)

	ins, _ := InsertMedia(&InsertMediaRequest{
		Package: pkg, SlideRef: ref,
		MediaData: fakeMediaBytes, MediaContentType: "video/mp4", MediaExt: ".mp4",
		Kind: MediaKindVideo, X: 0, Y: 0, CX: 1000000, CY: 1000000,
		PlayTrigger: "click", Volume: 80,
	})

	rep, err := ReplaceMedia(&ReplaceMediaRequest{
		Package: pkg, SlideRef: ref,
		Selector:     &selectors.ShapeIDSelector{ID: ins.ShapeID},
		NewMediaData: []byte("audio-bytes"), NewMediaContentType: "audio/x-m4a", NewMediaExt: ".m4a",
		NewKind: MediaKindAudio,
	})
	if err != nil {
		t.Fatalf("ReplaceMedia kind flip: %v", err)
	}
	if rep.OldKind != "video" || rep.NewKind != "audio" {
		t.Errorf("kind flip = %s->%s, want video->audio", rep.OldKind, rep.NewKind)
	}

	doc, _ := pkg.ReadXMLPart(ref.PartURI)
	root := doc.Root()
	pic := findMediaPicBySpid(root, ins.ShapeID)
	nvPr := xmlx.FindChild(xmlx.FindChild(pic, ns.NsP, "nvPicPr"), ns.NsP, "nvPr")
	if xmlx.FindChild(nvPr, ns.NsA, "audioFile") == nil {
		t.Error("a:videoFile not flipped to a:audioFile")
	}
	if xmlx.FindChild(nvPr, ns.NsA, "videoFile") != nil {
		t.Error("a:videoFile still present after flip")
	}
	timing := xmlx.FindChild(root, ns.NsP, "timing")
	node := findCMediaNodeForSpid(timing, ins.ShapeID)
	if node == nil || localTag(node.Parent().Tag) != "audio" {
		t.Error("p:video node not flipped to p:audio")
	}
	// av rel type flipped.
	for _, r := range pkg.ListRelationships(ref.PartURI) {
		if r.ID == rep.AVRelID && r.Type != relTypeAudio {
			t.Errorf("av rel type = %q, want audio", r.Type)
		}
	}
	assertValidateClean(t, pkg)
}

func TestReplaceMedia_GuardFailures(t *testing.T) {
	pkg := openMediaFixture(t, "minimal-title")
	defer pkg.Close()
	ref := slideRef(t, pkg, 1)

	ins, _ := InsertMedia(&InsertMediaRequest{
		Package: pkg, SlideRef: ref,
		MediaData: fakeMediaBytes, MediaContentType: "video/mp4", MediaExt: ".mp4",
		Kind: MediaKindVideo, Name: "RealName", X: 0, Y: 0, CX: 1000000, CY: 1000000,
		PlayTrigger: "click", Volume: 80,
	})

	_, err := ReplaceMedia(&ReplaceMediaRequest{
		Package: pkg, SlideRef: ref,
		Selector:     &selectors.ShapeIDSelector{ID: ins.ShapeID},
		NewMediaData: fakeMediaBytes, NewMediaContentType: "video/mp4", NewMediaExt: ".mp4",
		NewKind:         MediaKindVideo,
		ExpectShapeName: "WrongName",
	})
	if err == nil {
		t.Error("expected shape-name guard failure")
	}

	_, err = ReplaceMedia(&ReplaceMediaRequest{
		Package: pkg, SlideRef: ref,
		Selector:     &selectors.ShapeIDSelector{ID: ins.ShapeID},
		NewMediaData: fakeMediaBytes, NewMediaContentType: "video/mp4", NewMediaExt: ".mp4",
		NewKind:    MediaKindVideo,
		ExpectKind: MediaKindAudio,
	})
	if err == nil {
		t.Error("expected media-kind guard failure")
	}
}

func TestReplaceMedia_RejectsNonMediaPic(t *testing.T) {
	pkg := openMediaFixture(t, "picture-placeholder")
	defer pkg.Close()
	ref := slideRef(t, pkg, 2)

	// Slide 2 of picture-placeholder has a plain image pic (id 2).
	_, err := ReplaceMedia(&ReplaceMediaRequest{
		Package: pkg, SlideRef: ref,
		Selector:     &selectors.ShapeIDSelector{ID: 2},
		NewMediaData: fakeMediaBytes, NewMediaContentType: "video/mp4", NewMediaExt: ".mp4",
		NewKind: MediaKindVideo,
	})
	if err == nil || !strings.Contains(err.Error(), "not embedded media") {
		t.Errorf("expected non-media rejection, got %v", err)
	}
}

func TestInsertMedia_PlayCmdOptIn(t *testing.T) {
	pkg := openMediaFixture(t, "minimal-title")
	defer pkg.Close()
	ref := slideRef(t, pkg, 1)

	res, err := InsertMedia(&InsertMediaRequest{
		Package: pkg, SlideRef: ref,
		MediaData: fakeMediaBytes, MediaContentType: "video/mp4", MediaExt: ".mp4",
		Kind: MediaKindVideo, X: 0, Y: 0, CX: 1000000, CY: 1000000,
		PlayTrigger: "click", EmitPlayCmd: true, Volume: 80,
	})
	if err != nil {
		t.Fatalf("InsertMedia: %v", err)
	}
	doc, _ := pkg.ReadXMLPart(ref.PartURI)
	timing := xmlx.FindChild(doc.Root(), ns.NsP, "timing")
	foundCmd := false
	for _, cmd := range xmlx.FindDescendants(timing, ns.NsP, "cmd") {
		if c, _ := xmlx.GetAttr(cmd, "cmd"); c == "playFrom(0.0)" {
			foundCmd = true
		}
	}
	if !foundCmd {
		t.Error("--play-cmd did not emit playFrom(0.0)")
	}
	_ = res
	assertValidateClean(t, pkg)
}

func TestMediaKindForExtension(t *testing.T) {
	cases := map[string]MediaKind{
		".mp4": MediaKindVideo, ".mov": MediaKindVideo,
		".m4a": MediaKindAudio, ".mp3": MediaKindAudio, ".wav": MediaKindAudio,
		".txt": "",
	}
	for ext, want := range cases {
		if got := MediaKindForExtension(ext); got != want {
			t.Errorf("MediaKindForExtension(%q) = %q, want %q", ext, got, want)
		}
	}
}

func TestSynthesizePosterPNG_IsValidPNG(t *testing.T) {
	data := synthesizePosterPNG()
	if len(data) < 8 || string(data[1:4]) != "PNG" {
		t.Fatalf("synthesized poster is not a PNG (len=%d)", len(data))
	}
}

// countTmRoots counts the cTn[nodeType=tmRoot] nodes anywhere in the timing tree.
// A correct tree has EXACTLY one; two means the timing tree is corrupt.
func countTmRoots(timing *etree.Element) int {
	if timing == nil {
		return 0
	}
	n := 0
	for _, cTn := range xmlx.FindDescendants(timing, ns.NsP, "cTn") {
		if nt, _ := xmlx.GetAttr(cTn, "nodeType"); nt == "tmRoot" {
			n++
		}
	}
	return n
}

func slideTiming(t *testing.T, pkg *opc.Package, uri string) *etree.Element {
	t.Helper()
	doc, err := pkg.ReadXMLPart(uri)
	if err != nil {
		t.Fatalf("read %s: %v", uri, err)
	}
	return xmlx.FindChild(doc.Root(), ns.NsP, "timing")
}

// TestInsertMedia_PlayCmd_SingleTmRoot is the Finding-1 regression: media add
// --play-cmd on a slide with NO prior timing must yield EXACTLY one tmRoot (the
// media registration node and the playFrom mainSeq must converge on the same
// tmRoot, never author a second one).
func TestInsertMedia_PlayCmd_SingleTmRoot(t *testing.T) {
	pkg := openMediaFixture(t, "minimal-title")
	defer pkg.Close()
	ref := slideRef(t, pkg, 1)

	if existing := slideTiming(t, pkg, ref.PartURI); existing != nil {
		t.Fatalf("fixture precondition: slide already has timing; expected none")
	}

	res, err := InsertMedia(&InsertMediaRequest{
		Package:          pkg,
		SlideRef:         ref,
		MediaData:        fakeMediaBytes,
		MediaContentType: "video/mp4",
		MediaExt:         ".mp4",
		Kind:             MediaKindVideo,
		Name:             "Clip",
		X:                100, Y: 200, CX: 3000000, CY: 2000000,
		PlayTrigger: "click",
		EmitPlayCmd: true,
		Volume:      80,
	})
	if err != nil {
		t.Fatalf("InsertMedia: %v", err)
	}

	timing := slideTiming(t, pkg, ref.PartURI)
	if got := countTmRoots(timing); got != 1 {
		t.Fatalf("expected exactly 1 tmRoot after media add --play-cmd, got %d", got)
	}
	// Both the media node and the playFrom cmd must be present in that one tree.
	if findCMediaNodeForSpid(timing, res.ShapeID) == nil {
		t.Error("media registration node missing")
	}
	if !hasPlayFromCmd(timing, res.ShapeID) {
		t.Error("playFrom cmd missing")
	}
	assertValidateClean(t, pkg)
}

// TestInsertMedia_PlayCmd_ThenAnimation_SingleTmRoot and its reverse assert that
// media (--play-cmd) and a subsequent animations add (and vice-versa) converge on
// a SINGLE tmRoot containing both the mainSeq effects and the media nodes.
func TestInsertMedia_PlayCmd_ThenAnimation_SingleTmRoot(t *testing.T) {
	pkg := openMediaFixture(t, "minimal-title")
	defer pkg.Close()
	ref := slideRef(t, pkg, 1)

	mres, err := InsertMedia(&InsertMediaRequest{
		Package:          pkg,
		SlideRef:         ref,
		MediaData:        fakeMediaBytes,
		MediaContentType: "video/mp4",
		MediaExt:         ".mp4",
		Kind:             MediaKindVideo,
		Name:             "Clip",
		X:                1, Y: 1, CX: 3000000, CY: 2000000,
		PlayTrigger: "click",
		EmitPlayCmd: true,
		Volume:      80,
	})
	if err != nil {
		t.Fatalf("InsertMedia: %v", err)
	}

	// Add an entrance effect to an existing shape on the slide.
	ares, err := AddAnimation(&AddAnimationRequest{
		Package:  pkg,
		SlideRef: ref,
		Selector: pickAnyShapeSelector(t, pkg, ref),
		Effect:   "appear",
		Start:    "onClick",
	})
	if err != nil {
		t.Fatalf("AddAnimation: %v", err)
	}

	timing := slideTiming(t, pkg, ref.PartURI)
	if got := countTmRoots(timing); got != 1 {
		t.Fatalf("expected exactly 1 tmRoot after media+anim, got %d", got)
	}
	if findCMediaNodeForSpid(timing, mres.ShapeID) == nil {
		t.Error("media node missing after animation add")
	}
	if findMainSeqCTnIn(timing) == nil {
		t.Error("mainSeq missing after animation add")
	}
	if len(ares.AddedEffectIDs) == 0 {
		t.Error("no effect ids returned")
	}
	assertValidateClean(t, pkg)
}

func TestInsertMedia_AnimationThenPlayCmd_SingleTmRoot(t *testing.T) {
	pkg := openMediaFixture(t, "minimal-title")
	defer pkg.Close()
	ref := slideRef(t, pkg, 1)

	// Animation first (creates the tmRoot + mainSeq skeleton).
	if _, err := AddAnimation(&AddAnimationRequest{
		Package:  pkg,
		SlideRef: ref,
		Selector: pickAnyShapeSelector(t, pkg, ref),
		Effect:   "appear",
		Start:    "onClick",
	}); err != nil {
		t.Fatalf("AddAnimation: %v", err)
	}

	mres, err := InsertMedia(&InsertMediaRequest{
		Package:          pkg,
		SlideRef:         ref,
		MediaData:        fakeMediaBytes,
		MediaContentType: "video/mp4",
		MediaExt:         ".mp4",
		Kind:             MediaKindVideo,
		Name:             "Clip",
		X:                1, Y: 1, CX: 3000000, CY: 2000000,
		PlayTrigger: "click",
		EmitPlayCmd: true,
		Volume:      80,
	})
	if err != nil {
		t.Fatalf("InsertMedia: %v", err)
	}

	timing := slideTiming(t, pkg, ref.PartURI)
	if got := countTmRoots(timing); got != 1 {
		t.Fatalf("expected exactly 1 tmRoot after anim+media, got %d", got)
	}
	if findCMediaNodeForSpid(timing, mres.ShapeID) == nil {
		t.Error("media node missing")
	}
	if !hasPlayFromCmd(timing, mres.ShapeID) {
		t.Error("playFrom cmd missing")
	}
	if findMainSeqCTnIn(timing) == nil {
		t.Error("mainSeq missing")
	}
	assertValidateClean(t, pkg)
}

// hasPlayFromCmd reports whether the timing tree carries a p:cmd playFrom call
// targeting spid (mirrors the inspect-side click-to-play scan).
func hasPlayFromCmd(timing *etree.Element, spid int) bool {
	for _, cmd := range xmlx.FindDescendants(timing, ns.NsP, "cmd") {
		if t, _ := xmlx.GetAttr(cmd, "type"); t != "call" {
			continue
		}
		if c, _ := xmlx.GetAttr(cmd, "cmd"); !strings.HasPrefix(c, "playFrom") {
			continue
		}
		for _, sp := range xmlx.FindDescendants(cmd, ns.NsP, "spTgt") {
			if v, ok := parseIntAttr(sp, "spid"); ok && v == spid {
				return true
			}
		}
	}
	return false
}

// pickAnyShapeSelector returns a selector for the first targetable shape on the
// slide (used to attach an entrance effect alongside media).
func pickAnyShapeSelector(t *testing.T, pkg *opc.Package, ref *inspect.SlideRef) selectors.Selector {
	t.Helper()
	doc, err := pkg.ReadXMLPart(ref.PartURI)
	if err != nil {
		t.Fatalf("read slide: %v", err)
	}
	spTree := findSlideSpTree(doc.Root())
	for _, sp := range xmlx.FindDescendants(spTree, ns.NsP, "sp") {
		if id := shapeCNvPrIDLocal(sp); id != 0 {
			return &selectors.ShapeIDSelector{ID: id}
		}
	}
	t.Fatalf("no targetable shape on slide")
	return nil
}

// --- helpers ---

func findMediaPicBySpid(root *etree.Element, spid int) *etree.Element {
	spTree := findSlideSpTree(root)
	if spTree == nil {
		return nil
	}
	for _, pic := range xmlx.FindDescendants(spTree, ns.NsP, "pic") {
		if shapeCNvPrIDLocal(pic) == spid && isMediaPic(pic) {
			return pic
		}
	}
	return nil
}

func findCMediaNodeForSpid(timing *etree.Element, spid int) *etree.Element {
	if timing == nil {
		return nil
	}
	for _, node := range xmlx.FindDescendants(timing, ns.NsP, "cMediaNode") {
		if mediaNodeTargetsSpid(node, spid) {
			return node
		}
	}
	return nil
}

func allCTnIDs(timing *etree.Element) []int {
	var out []int
	for _, cTn := range xmlx.FindDescendants(timing, ns.NsP, "cTn") {
		if id, ok := parseIntAttr(cTn, "id"); ok {
			out = append(out, id)
		}
	}
	return out
}

func assertTimingSchemaOrder(t *testing.T, root *etree.Element) {
	t.Helper()
	seenTiming := false
	for _, child := range root.ChildElements() {
		lt := localTag(child.Tag)
		switch lt {
		case "extLst":
			if seenTiming {
				// timing before extLst: correct.
			}
		case "timing":
			seenTiming = true
		case "cSld", "clrMapOvr", "transition":
			if seenTiming {
				t.Errorf("p:timing appears before p:%s (wrong CT_Slide order)", lt)
			}
		}
	}
	if !seenTiming {
		t.Error("p:timing not found at slide root")
	}
}

// serializeFirst serializes the first descendant element with the given local
// name to a canonical string for byte-structure comparison.
func serializeFirst(t *testing.T, root *etree.Element, local string) string {
	t.Helper()
	for _, e := range xmlx.FindDescendants(root, ns.NsP, local) {
		doc := etree.NewDocument()
		doc.SetRoot(e.Copy())
		doc.Indent(2)
		s, err := doc.WriteToString()
		if err != nil {
			t.Fatalf("serialize %s: %v", local, err)
		}
		return s
	}
	return ""
}
