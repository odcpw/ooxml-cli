package mutate

import (
	"bytes"
	"fmt"
	"image"
	"image/color"
	"image/png"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	ns "github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
)

// media.go embeds, replaces, and (via the inspect package) lists local audio and
// video clips on a slide.
//
// A clip is stored as a p:pic carrying the dual legacy+modern representation
// (a:videoFile/a:audioFile r:link + p14:media r:embed), a poster image in the
// p:blipFill, and click-to-play wired via the verified (Tier A) path:
// a:hlinkClick action="ppaction://media" plus a passive p:video/p:audio +
// p:cMediaNode registration node injected into the slide's p:timing tree.
//
// PRESERVE RULE (media-scoped): we read the slide XML and mutate only the nodes
// we own; ReplaceXMLPart re-serializes everything else verbatim. We NEVER rebuild
// an existing p:timing tree -- if one is present (authored by an animation sibling
// or by PowerPoint) we locate the tmRoot cTn's childTnLst and append ONLY the
// passive media node, leaving every existing timing child byte-intact.

// MediaKind is "video" or "audio".
type MediaKind string

const (
	MediaKindVideo MediaKind = "video"
	MediaKindAudio MediaKind = "audio"
)

// ParseMediaKind parses an explicit --kind value.
func ParseMediaKind(s string) (MediaKind, error) {
	switch strings.ToLower(strings.TrimSpace(s)) {
	case "video":
		return MediaKindVideo, nil
	case "audio":
		return MediaKindAudio, nil
	case "":
		return "", fmt.Errorf("media kind is empty")
	default:
		return "", fmt.Errorf("invalid media kind %q (must be video or audio)", s)
	}
}

// MediaKindForExtension auto-detects the media kind from a file extension. It
// returns "" when the extension cannot be classified (caller must require --kind).
func MediaKindForExtension(ext string) MediaKind {
	switch strings.ToLower(strings.TrimPrefix(ext, ".")) {
	case "mp4", "mov", "avi", "wmv", "m4v", "mpg", "mpeg", "mkv", "webm":
		return MediaKindVideo
	case "m4a", "mp3", "wav", "aac", "wma", "oga", "ogg", "flac":
		return MediaKindAudio
	default:
		return ""
	}
}

// contentTypeForMediaExt returns the best-effort content type for a media
// extension. Falls back to a generic type when unknown.
func ContentTypeForMediaExt(ext string) string {
	switch strings.ToLower(strings.TrimPrefix(ext, ".")) {
	case "mp4", "m4v":
		return "video/mp4"
	case "mov":
		return "video/quicktime"
	case "avi":
		return "video/x-msvideo"
	case "wmv":
		return "video/x-ms-wmv"
	case "mpg", "mpeg":
		return "video/mpeg"
	case "mkv":
		return "video/x-matroska"
	case "webm":
		return "video/webm"
	case "m4a":
		return "audio/x-m4a"
	case "mp3":
		return "audio/mpeg"
	case "wav":
		return "audio/wav"
	case "aac":
		return "audio/aac"
	case "wma":
		return "audio/x-ms-wma"
	case "oga", "ogg":
		return "audio/ogg"
	case "flac":
		return "audio/flac"
	default:
		return "application/octet-stream"
	}
}

// ---------------------------------------------------------------------------
// Request / result types (mirroring InsertImageRequest / ReplaceImageResult).
// ---------------------------------------------------------------------------

// InsertMediaRequest embeds a new media clip on a slide.
type InsertMediaRequest struct {
	Package  opc.PackageSession
	SlideRef *inspect.SlideRef

	MediaData        []byte
	MediaContentType string
	MediaExt         string // file extension incl. dot, e.g. ".mp4"
	Kind             MediaKind

	PosterData        []byte // nil -> synthesized placeholder
	PosterContentType string // defaults to image/png
	Name              string

	X, Y, CX, CY int64

	PlayTrigger string // "click" | "none"
	EmitPlayCmd bool   // Tier-D playFrom trigger opt-in
	Volume      int    // 0..100
	Mute        bool

	InsertAfterID int
}

// InsertMediaResult reports the embedded clip.
type InsertMediaResult struct {
	ShapeID           int    `json:"shapeId"`
	ShapeName         string `json:"shapeName"`
	Kind              string `json:"kind"`
	MediaPartURI      string `json:"mediaPartUri"`
	MediaContentType  string `json:"mediaContentType"`
	PosterPartURI     string `json:"posterPartUri"`
	MediaRelID        string `json:"mediaRelationshipId"`
	AVRelID           string `json:"avRelationshipId"`
	PosterRelID       string `json:"posterRelationshipId"`
	PlayTrigger       string `json:"playTrigger"`
	PosterSynthesized bool   `json:"posterSynthesized"`
	EmitPlayCmd       bool   `json:"emitPlayCmd"`
}

// ReplaceMediaRequest replaces the media bytes (and optionally poster/kind) of an
// existing media pic.
type ReplaceMediaRequest struct {
	Package  opc.PackageSession
	SlideRef *inspect.SlideRef
	Selector selectors.Selector

	NewMediaData        []byte
	NewMediaContentType string
	NewMediaExt         string
	NewKind             MediaKind

	NewPosterData        []byte // nil = keep existing poster
	NewPosterContentType string

	Volume *int
	Mute   *bool

	ExpectShapeName string
	ExpectKind      MediaKind // "" = no guard
}

// ReplaceMediaResult reports the replacement.
type ReplaceMediaResult struct {
	ShapeID        int    `json:"shapeId"`
	ShapeName      string `json:"shapeName"`
	OldKind        string `json:"oldKind"`
	NewKind        string `json:"newKind"`
	OldMediaURI    string `json:"oldMediaUri"`
	NewMediaURI    string `json:"newMediaUri"`
	OldContentType string `json:"oldContentType"`
	NewContentType string `json:"newContentType"`
	PosterReplaced bool   `json:"posterReplaced"`
	MediaRelID     string `json:"mediaRelationshipId"`
	AVRelID        string `json:"avRelationshipId"`
	PosterRelID    string `json:"posterRelationshipId"`
}

// ---------------------------------------------------------------------------
// InsertMedia
// ---------------------------------------------------------------------------

// InsertMedia embeds a local media clip on a slide as a dual-representation
// p:pic, registers it in the slide's p:timing tree, and wires click-to-play.
func InsertMedia(req *InsertMediaRequest) (*InsertMediaResult, error) {
	if req == nil || req.Package == nil {
		return nil, fmt.Errorf("insert media request/package is nil")
	}
	if req.SlideRef == nil {
		return nil, fmt.Errorf("slide reference is nil")
	}
	if len(req.MediaData) == 0 {
		return nil, fmt.Errorf("media data is empty")
	}
	if req.CX <= 0 || req.CY <= 0 {
		return nil, fmt.Errorf("media dimensions must be positive: cx=%d, cy=%d", req.CX, req.CY)
	}
	kind := req.Kind
	if kind != MediaKindVideo && kind != MediaKindAudio {
		return nil, fmt.Errorf("media kind must be video or audio")
	}
	playTrigger := normalizePlayTrigger(req.PlayTrigger)

	doc, err := req.Package.ReadXMLPart(req.SlideRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read slide: %w", err)
	}
	root := doc.Root()
	spTree := findSlideSpTree(root)
	if spTree == nil {
		return nil, fmt.Errorf("shape tree not found in slide")
	}

	// Allocate a fresh shape id (max cNvPr id across spTree + 1).
	spid := nextSpTreeShapeID(spTree)

	// Prepare the poster before mutating the package so validation failures are
	// side-effect free.
	posterData := req.PosterData
	posterCT := req.PosterContentType
	posterSynth := false
	if len(posterData) == 0 {
		posterData = synthesizePosterPNG()
		posterCT = "image/png"
		posterSynth = true
	}
	if posterCT == "" {
		posterCT = "image/png"
	}
	posterCT, err = validateImagePayload(posterCT, posterData)
	if err != nil {
		return nil, err
	}
	posterExt, err := getExtensionForContentType(posterCT)
	if err != nil {
		return nil, err
	}

	// Add the media part.
	mediaExt := req.MediaExt
	if mediaExt == "" {
		mediaExt = ".bin"
	}
	mediaURI := allocatePPTXNumberedPart(req.Package, "/ppt/media/media", mediaExt)
	if err := req.Package.AddPart(mediaURI, req.MediaData, req.MediaContentType, nil); err != nil {
		return nil, fmt.Errorf("failed to add media part: %w", err)
	}

	// Add the poster part (real or synthesized).
	posterURI := allocatePPTXNumberedPart(req.Package, "/ppt/media/image", posterExt)
	if err := req.Package.AddPart(posterURI, posterData, posterCT, nil); err != nil {
		return nil, fmt.Errorf("failed to add poster part: %w", err)
	}

	// Create the three slide rels: media (MS ns), video|audio (OOXML), image.
	rels := req.Package.ListRelationships(req.SlideRef.PartURI)
	mediaRelID, rels, err := addMediaRel(req.SlideRef.PartURI, rels, relTypeMedia, mediaURI)
	if err != nil {
		return nil, err
	}
	avRelType := relTypeVideo
	if kind == MediaKindAudio {
		avRelType = relTypeAudio
	}
	avRelID, rels, err := addMediaRel(req.SlideRef.PartURI, rels, avRelType, mediaURI)
	if err != nil {
		return nil, err
	}
	posterRelID, rels, err := addMediaRel(req.SlideRef.PartURI, rels, relTypeImage, posterURI)
	if err != nil {
		return nil, err
	}
	if err := writeRelationships(req.Package, req.SlideRef.PartURI, rels); err != nil {
		return nil, err
	}

	// Build and insert the p:pic.
	name := req.Name
	if name == "" {
		name = filepath.Base(strings.TrimSuffix(mediaURI, mediaExt)) + mediaExt
	}
	pic := buildMediaPic(spid, name, kind, mediaRelID, avRelID, posterRelID, req.X, req.Y, req.CX, req.CY, playTrigger == "click")
	insertShapeAfter(spTree, pic, req.InsertAfterID)

	// Inject the passive media registration node into p:timing.
	volume := clampVolume(req.Volume)
	injectMediaRegistrationNode(root, kind, spid, volume, req.Mute)

	// Optional Tier-D explicit playFrom command trigger.
	if req.EmitPlayCmd {
		injectPlayFromCmd(root, spid)
	}

	doc.IndentTabs()
	if err := req.Package.ReplaceXMLPart(req.SlideRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to write slide: %w", err)
	}

	return &InsertMediaResult{
		ShapeID:           spid,
		ShapeName:         name,
		Kind:              string(kind),
		MediaPartURI:      mediaURI,
		MediaContentType:  req.MediaContentType,
		PosterPartURI:     posterURI,
		MediaRelID:        mediaRelID,
		AVRelID:           avRelID,
		PosterRelID:       posterRelID,
		PlayTrigger:       playTrigger,
		PosterSynthesized: posterSynth,
		EmitPlayCmd:       req.EmitPlayCmd,
	}, nil
}

// ---------------------------------------------------------------------------
// ReplaceMedia
// ---------------------------------------------------------------------------

// ReplaceMedia replaces the media bytes (and optionally poster, kind, volume,
// mute) of an existing media pic, preserving geometry, cNvPr id/name, the
// hlinkClick, the p:extLst structure, and the timing node.
func ReplaceMedia(req *ReplaceMediaRequest) (*ReplaceMediaResult, error) {
	if req == nil || req.Package == nil {
		return nil, fmt.Errorf("replace media request/package is nil")
	}
	if req.SlideRef == nil {
		return nil, fmt.Errorf("slide reference is nil")
	}
	if len(req.NewMediaData) == 0 {
		return nil, fmt.Errorf("new media data is empty")
	}
	newKind := req.NewKind
	if newKind != MediaKindVideo && newKind != MediaKindAudio {
		return nil, fmt.Errorf("media kind must be video or audio")
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

	pic, spid, name, err := resolveMediaPic(spTree, req.Selector)
	if err != nil {
		return nil, err
	}
	if req.ExpectShapeName != "" && req.ExpectShapeName != name {
		return nil, fmt.Errorf("shape name guard failed: expected %q but resolved %q", req.ExpectShapeName, name)
	}

	oldKind := mediaPicKind(pic)
	if req.ExpectKind != "" && req.ExpectKind != MediaKind(oldKind) {
		return nil, fmt.Errorf("media kind guard failed: expected %q but found %q", req.ExpectKind, oldKind)
	}

	result := &ReplaceMediaResult{
		ShapeID:   spid,
		ShapeName: name,
		OldKind:   oldKind,
		NewKind:   string(newKind),
	}

	rels := req.Package.ListRelationships(req.SlideRef.PartURI)
	relMap := map[string]opc.RelationshipInfo{}
	for _, rel := range rels {
		relMap[rel.ID] = rel
	}

	// Resolve the existing media/av/poster rel ids from the pic.
	mediaRelID, avRelID, posterRelID := mediaPicRelIDs(pic)
	result.MediaRelID, result.AVRelID, result.PosterRelID = mediaRelID, avRelID, posterRelID

	// Determine the old media URI from the av (or media) rel.
	oldURI := ""
	if r, ok := relMap[avRelID]; ok {
		oldURI = opc.ResolveRelationshipTarget(req.SlideRef.PartURI, r.Target)
	} else if r, ok := relMap[mediaRelID]; ok {
		oldURI = opc.ResolveRelationshipTarget(req.SlideRef.PartURI, r.Target)
	}
	result.OldMediaURI = oldURI
	if oldURI != "" {
		result.OldContentType = req.Package.GetContentType(oldURI)
	}

	// Validate optional poster replacement before mutating media bytes so a bad
	// poster cannot leave the package partially updated.
	posterCT := req.NewPosterContentType
	posterExt := ""
	if len(req.NewPosterData) > 0 {
		if posterCT == "" {
			posterCT = "image/png"
		}
		posterCT, err = validateImagePayload(posterCT, req.NewPosterData)
		if err != nil {
			return nil, err
		}
		posterExt, err = getExtensionForContentType(posterCT)
		if err != nil {
			return nil, err
		}
	}

	// Write the new media bytes. When the extension/content-type changes, add a
	// new numbered part and retarget the rels (mirroring ReplaceImage); otherwise
	// replace in place.
	newURI := oldURI
	newExt := req.NewMediaExt
	if newExt == "" {
		newExt = filepath.Ext(oldURI)
	}
	extChanged := oldURI == "" || !strings.EqualFold(filepath.Ext(oldURI), newExt)
	if extChanged {
		newURI = allocatePPTXNumberedPart(req.Package, "/ppt/media/media", newExt)
		if err := req.Package.AddPart(newURI, req.NewMediaData, req.NewMediaContentType, nil); err != nil {
			return nil, fmt.Errorf("failed to add media part: %w", err)
		}
		retargetRel(rels, mediaRelID, req.SlideRef.PartURI, newURI)
		retargetRel(rels, avRelID, req.SlideRef.PartURI, newURI)
	} else {
		if err := req.Package.ReplaceRawPart(newURI, req.NewMediaData, req.NewMediaContentType); err != nil {
			return nil, fmt.Errorf("failed to replace media part: %w", err)
		}
	}
	result.NewMediaURI = newURI
	result.NewContentType = req.NewMediaContentType

	// Kind flip: rewrite a:videoFile<->a:audioFile, the av rel Type, and the
	// p:video<->p:audio timing node tag.
	if newKind != MediaKind(oldKind) {
		flipMediaKind(root, pic, spid, newKind, rels, avRelID)
	}

	// Optional poster replacement.
	if len(req.NewPosterData) > 0 {
		posterURI := ""
		if r, ok := relMap[posterRelID]; ok {
			posterURI = opc.ResolveRelationshipTarget(req.SlideRef.PartURI, r.Target)
		}
		if posterURI != "" && strings.EqualFold(filepath.Ext(posterURI), posterExt) {
			if err := req.Package.ReplaceRawPart(posterURI, req.NewPosterData, posterCT); err != nil {
				return nil, fmt.Errorf("failed to replace poster part: %w", err)
			}
		} else {
			newPoster := allocatePPTXNumberedPart(req.Package, "/ppt/media/image", posterExt)
			if err := req.Package.AddPart(newPoster, req.NewPosterData, posterCT, nil); err != nil {
				return nil, fmt.Errorf("failed to add poster part: %w", err)
			}
			retargetRel(rels, posterRelID, req.SlideRef.PartURI, newPoster)
		}
		result.PosterReplaced = true
	}

	// Optional volume/mute updates on the cMediaNode.
	if req.Volume != nil || req.Mute != nil {
		updateMediaNodeAttrs(root, spid, req.Volume, req.Mute)
	}

	if err := writeRelationships(req.Package, req.SlideRef.PartURI, rels); err != nil {
		return nil, err
	}

	doc.IndentTabs()
	if err := req.Package.ReplaceXMLPart(req.SlideRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to write slide: %w", err)
	}
	return result, nil
}

// ---------------------------------------------------------------------------
// p:pic builder.
// ---------------------------------------------------------------------------

// buildMediaPic constructs the dual-representation media p:pic (modeled on
// createPictureElement but with the AV nvPr and extLst).
func buildMediaPic(spid int, name string, kind MediaKind, mediaRelID, avRelID, posterRelID string, x, y, cx, cy int64, clickPlay bool) *etree.Element {
	pic := etree.NewElement("p:pic")

	nvPicPr := etree.NewElement("p:nvPicPr")
	cNvPr := etree.NewElement("p:cNvPr")
	cNvPr.CreateAttr("id", strconv.Itoa(spid))
	cNvPr.CreateAttr("name", name)
	if clickPlay {
		hlink := etree.NewElement("a:hlinkClick")
		hlink.CreateAttr("r:id", "")
		hlink.CreateAttr("action", hlinkMediaAction)
		cNvPr.AddChild(hlink)
	}
	nvPicPr.AddChild(cNvPr)

	cNvPicPr := etree.NewElement("p:cNvPicPr")
	picLocks := etree.NewElement("a:picLocks")
	picLocks.CreateAttr("noChangeAspect", "1")
	cNvPicPr.AddChild(picLocks)
	nvPicPr.AddChild(cNvPicPr)

	nvPr := etree.NewElement("p:nvPr")
	avFile := etree.NewElement("a:" + avFileLocal(kind))
	avFile.CreateAttr("r:link", avRelID)
	nvPr.AddChild(avFile)
	extLst := etree.NewElement("p:extLst")
	ext := etree.NewElement("p:ext")
	ext.CreateAttr("uri", mediaExtURI)
	p14media := etree.NewElement("p14:media")
	ensureNamespace(p14media, "p14", ns.Np14)
	p14media.CreateAttr("r:embed", mediaRelID)
	ext.AddChild(p14media)
	extLst.AddChild(ext)
	nvPr.AddChild(extLst)
	nvPicPr.AddChild(nvPr)
	pic.AddChild(nvPicPr)

	// Blip fill (poster image).
	blipFill := etree.NewElement("p:blipFill")
	blip := etree.NewElement("a:blip")
	blip.CreateAttr("r:embed", posterRelID)
	blipFill.AddChild(blip)
	stretch := etree.NewElement("a:stretch")
	stretch.AddChild(etree.NewElement("a:fillRect"))
	blipFill.AddChild(stretch)
	pic.AddChild(blipFill)

	// Shape properties (geometry).
	spPr := etree.NewElement("p:spPr")
	xfrm := etree.NewElement("a:xfrm")
	off := etree.NewElement("a:off")
	off.CreateAttr("x", strconv.FormatInt(x, 10))
	off.CreateAttr("y", strconv.FormatInt(y, 10))
	xfrm.AddChild(off)
	ext2 := etree.NewElement("a:ext")
	ext2.CreateAttr("cx", strconv.FormatInt(cx, 10))
	ext2.CreateAttr("cy", strconv.FormatInt(cy, 10))
	xfrm.AddChild(ext2)
	spPr.AddChild(xfrm)
	prstGeom := etree.NewElement("a:prstGeom")
	prstGeom.CreateAttr("prst", "rect")
	prstGeom.AddChild(etree.NewElement("a:avLst"))
	spPr.AddChild(prstGeom)
	pic.AddChild(spPr)

	return pic
}

// avFileLocal returns "videoFile" or "audioFile" for the media kind.
func avFileLocal(kind MediaKind) string {
	if kind == MediaKindAudio {
		return "audioFile"
	}
	return "videoFile"
}

// mediaNodeLocal returns "video" or "audio" for the media kind.
func mediaNodeLocal(kind MediaKind) string {
	if kind == MediaKindAudio {
		return "audio"
	}
	return "video"
}

// ---------------------------------------------------------------------------
// Timing-tree injection (passive media node + optional playFrom cmd).
// ---------------------------------------------------------------------------

// injectMediaRegistrationNode appends the passive p:video/p:audio + p:cMediaNode
// node into the tmRoot cTn's childTnLst, creating the timing skeleton only when
// absent and never rebuilding an existing tree.
func injectMediaRegistrationNode(root *etree.Element, kind MediaKind, spid, volume int, mute bool) {
	timing, _ := getOrCreateTiming(root)
	tmRootChildTnLst := getOrCreateTmRootChildTnLst(timing)

	nodeID := allocateTimingNodeID(timing)

	mediaNodeWrap := etree.NewElement("p:" + mediaNodeLocal(kind))
	cMediaNode := etree.NewElement("p:cMediaNode")
	cMediaNode.CreateAttr("vol", strconv.Itoa(volume*1000))
	if mute {
		cMediaNode.CreateAttr("mute", "1")
	}
	cTn := etree.NewElement("p:cTn")
	cTn.CreateAttr("id", strconv.Itoa(nodeID))
	cTn.CreateAttr("fill", "hold")
	cTn.CreateAttr("display", "0")
	stCond := etree.NewElement("p:stCondLst")
	cond := etree.NewElement("p:cond")
	cond.CreateAttr("delay", "indefinite")
	stCond.AddChild(cond)
	cTn.AddChild(stCond)
	cMediaNode.AddChild(cTn)
	tgtEl := etree.NewElement("p:tgtEl")
	spTgt := etree.NewElement("p:spTgt")
	spTgt.CreateAttr("spid", strconv.Itoa(spid))
	tgtEl.AddChild(spTgt)
	cMediaNode.AddChild(tgtEl)
	mediaNodeWrap.AddChild(cMediaNode)

	tmRootChildTnLst.AddChild(mediaNodeWrap)
}

// getOrCreateTmRootChildTnLst returns the childTnLst of the timing's tmRoot cTn,
// building the minimal tnLst/par/cTn[tmRoot]/childTnLst skeleton when absent.
// It descends into an existing tree rather than rebuilding it.
func getOrCreateTmRootChildTnLst(timing *etree.Element) *etree.Element {
	tnLst := xmlx.FindChild(timing, ns.NsP, "tnLst")
	if tnLst == nil {
		tnLst = etree.NewElement("p:tnLst")
		timing.AddChild(tnLst)
	}
	// Find the tmRoot cTn.
	var tmRootCTn *etree.Element
	for _, par := range xmlx.FindChildren(tnLst, ns.NsP, "par") {
		cTn := xmlx.FindChild(par, ns.NsP, "cTn")
		if cTn == nil {
			continue
		}
		if nt, _ := xmlx.GetAttr(cTn, "nodeType"); nt == "tmRoot" {
			tmRootCTn = cTn
			break
		}
	}
	if tmRootCTn == nil {
		par := etree.NewElement("p:par")
		tmRootCTn = etree.NewElement("p:cTn")
		tmRootCTn.CreateAttr("id", strconv.Itoa(allocateTimingNodeID(timing)))
		tmRootCTn.CreateAttr("dur", "indefinite")
		tmRootCTn.CreateAttr("restart", "never")
		tmRootCTn.CreateAttr("nodeType", "tmRoot")
		par.AddChild(tmRootCTn)
		tnLst.AddChild(par)
	}
	childTnLst := xmlx.FindChild(tmRootCTn, ns.NsP, "childTnLst")
	if childTnLst == nil {
		childTnLst = etree.NewElement("p:childTnLst")
		tmRootCTn.AddChild(childTnLst)
	}
	return childTnLst
}

// injectPlayFromCmd appends the OPTIONAL Tier-D clickEffect p:cmd playFrom trigger
// under the mainSeq childTnLst, creating the seq skeleton when absent.
//
// spec-grounded; PowerPoint-render unconfirmed: see playFromCmd in animspec.go.
func injectPlayFromCmd(root *etree.Element, spid int) {
	timing, _ := getOrCreateTiming(root)
	mainSeqCTn := getOrCreateMainSeq(timing)
	childTnLst := childTnLstOf(mainSeqCTn)

	clickID := allocateTimingNodeID(timing)
	bhvrID := allocateTimingNodeIDAfter(timing, clickID)

	par := etree.NewElement("p:par")
	clickCTn := etree.NewElement("p:cTn")
	clickCTn.CreateAttr("id", strconv.Itoa(clickID))
	clickCTn.CreateAttr("fill", "hold")
	clickCTn.CreateAttr("nodeType", "clickEffect")
	stCond := etree.NewElement("p:stCondLst")
	cond := etree.NewElement("p:cond")
	cond.CreateAttr("delay", "indefinite")
	stCond.AddChild(cond)
	clickCTn.AddChild(stCond)
	inner := etree.NewElement("p:childTnLst")
	cmd := etree.NewElement("p:cmd")
	cmd.CreateAttr("type", "call")
	cmd.CreateAttr("cmd", playFromCmd)
	cBhvr := etree.NewElement("p:cBhvr")
	bhvrCTn := etree.NewElement("p:cTn")
	bhvrCTn.CreateAttr("id", strconv.Itoa(bhvrID))
	bhvrCTn.CreateAttr("dur", "2000")
	bhvrCTn.CreateAttr("fill", "hold")
	cBhvr.AddChild(bhvrCTn)
	tgtEl := etree.NewElement("p:tgtEl")
	spTgt := etree.NewElement("p:spTgt")
	spTgt.CreateAttr("spid", strconv.Itoa(spid))
	tgtEl.AddChild(spTgt)
	cBhvr.AddChild(tgtEl)
	cmd.AddChild(cBhvr)
	inner.AddChild(cmd)
	clickCTn.AddChild(inner)
	par.AddChild(clickCTn)
	childTnLst.AddChild(par)
}

// updateMediaNodeAttrs updates the vol/mute attributes of the cMediaNode
// targeting spid.
func updateMediaNodeAttrs(root *etree.Element, spid int, volume *int, mute *bool) {
	timing := xmlx.FindChild(root, ns.NsP, "timing")
	if timing == nil {
		return
	}
	for _, node := range xmlx.FindDescendants(timing, ns.NsP, "cMediaNode") {
		if !mediaNodeTargetsSpid(node, spid) {
			continue
		}
		if volume != nil {
			node.CreateAttr("vol", strconv.Itoa(clampVolume(*volume)*1000))
		}
		if mute != nil {
			if *mute {
				node.CreateAttr("mute", "1")
			} else {
				if a := node.SelectAttr("mute"); a != nil {
					node.RemoveAttr("mute")
				}
			}
		}
		return
	}
}

// flipMediaKind rewrites the pic's a:videoFile<->a:audioFile element, the av rel
// Type, and the p:video<->p:audio timing node tag when the kind changes.
func flipMediaKind(root, pic *etree.Element, spid int, newKind MediaKind, rels []opc.RelationshipInfo, avRelID string) {
	// Swap the nvPr av file element local name, preserving its r:link.
	nvPicPr := xmlx.FindChild(pic, ns.NsP, "nvPicPr")
	if nvPicPr != nil {
		if nvPr := xmlx.FindChild(nvPicPr, ns.NsP, "nvPr"); nvPr != nil {
			for _, child := range nvPr.ChildElements() {
				lt := localTag(child.Tag)
				if lt == "videoFile" || lt == "audioFile" {
					child.Space = "a"
					child.Tag = avFileLocal(newKind)
				}
			}
		}
	}
	// Update the av rel Type.
	for i := range rels {
		if rels[i].ID == avRelID {
			if newKind == MediaKindAudio {
				rels[i].Type = relTypeAudio
			} else {
				rels[i].Type = relTypeVideo
			}
		}
	}
	// Swap the timing node tag (p:video<->p:audio) for the node targeting spid.
	timing := xmlx.FindChild(root, ns.NsP, "timing")
	if timing == nil {
		return
	}
	for _, node := range xmlx.FindDescendants(timing, ns.NsP, "cMediaNode") {
		if !mediaNodeTargetsSpid(node, spid) {
			continue
		}
		if wrap := node.Parent(); wrap != nil {
			lt := localTag(wrap.Tag)
			if lt == "video" || lt == "audio" {
				wrap.Space = "p"
				wrap.Tag = mediaNodeLocal(newKind)
			}
		}
	}
}

// ---------------------------------------------------------------------------
// Relationship helpers.
// ---------------------------------------------------------------------------

// addMediaRel allocates a new rel id for a media-related relationship and appends
// it to rels, returning the id and the extended list.
func addMediaRel(sourceURI string, rels []opc.RelationshipInfo, relType, targetURI string) (string, []opc.RelationshipInfo, error) {
	id := AllocateRelationshipID(rels)
	target, err := relationshipTarget(sourceURI, targetURI)
	if err != nil {
		return "", rels, fmt.Errorf("failed to relativize media target: %w", err)
	}
	rels = append(rels, opc.RelationshipInfo{
		SourceURI: sourceURI,
		ID:        id,
		Type:      relType,
		Target:    target,
	})
	return id, rels, nil
}

// retargetRel updates the Target of the rel with the given id to point at newURI.
func retargetRel(rels []opc.RelationshipInfo, id, sourceURI, newURI string) {
	if id == "" {
		return
	}
	target, err := relationshipTarget(sourceURI, newURI)
	if err != nil {
		return
	}
	for i := range rels {
		if rels[i].ID == id {
			rels[i].Target = target
			return
		}
	}
}

// ---------------------------------------------------------------------------
// Media pic resolution / inspection.
// ---------------------------------------------------------------------------

// resolveMediaPic resolves a selector to a MEDIA-bearing pic (rejecting plain
// images), returning the pic element, its spid, and its name.
func resolveMediaPic(spTree *etree.Element, sel selectors.Selector) (*etree.Element, int, string, error) {
	switch s := sel.(type) {
	case *selectors.ShapeIDSelector:
		for _, pic := range xmlx.FindDescendants(spTree, ns.NsP, "pic") {
			if shapeCNvPrIDLocal(pic) != s.ID {
				continue
			}
			if !isMediaPic(pic) {
				return nil, 0, "", fmt.Errorf("shape %d is an image, not embedded media", s.ID)
			}
			return pic, s.ID, shapeNameLocal(pic), nil
		}
		return nil, 0, "", fmt.Errorf("media shape with id %d not found on slide", s.ID)
	case *selectors.ShapeNameSelector:
		for _, pic := range xmlx.FindDescendants(spTree, ns.NsP, "pic") {
			if shapeNameLocal(pic) != s.Name {
				continue
			}
			if !isMediaPic(pic) {
				return nil, 0, "", fmt.Errorf("shape %q is an image, not embedded media", s.Name)
			}
			return pic, shapeCNvPrIDLocal(pic), s.Name, nil
		}
		return nil, 0, "", fmt.Errorf("media shape named %q not found on slide", s.Name)
	default:
		return nil, 0, "", fmt.Errorf("unsupported shape selector %q (use shape:<id> or ~<name>)", sel.String())
	}
}

// isMediaPic reports whether a p:pic carries media (a:videoFile/a:audioFile or a
// p14:media embed).
func isMediaPic(pic *etree.Element) bool {
	nvPicPr := xmlx.FindChild(pic, ns.NsP, "nvPicPr")
	if nvPicPr == nil {
		return false
	}
	nvPr := xmlx.FindChild(nvPicPr, ns.NsP, "nvPr")
	if nvPr == nil {
		return false
	}
	if xmlx.FindChild(nvPr, ns.NsA, "videoFile") != nil || xmlx.FindChild(nvPr, ns.NsA, "audioFile") != nil {
		return true
	}
	return findP14MediaIn(nvPr) != nil
}

// mediaPicKind returns "video" or "audio" for a media pic (defaults to video).
func mediaPicKind(pic *etree.Element) string {
	nvPicPr := xmlx.FindChild(pic, ns.NsP, "nvPicPr")
	if nvPicPr != nil {
		if nvPr := xmlx.FindChild(nvPicPr, ns.NsP, "nvPr"); nvPr != nil {
			if xmlx.FindChild(nvPr, ns.NsA, "audioFile") != nil {
				return "audio"
			}
		}
	}
	return "video"
}

// mediaPicRelIDs extracts the (media, av, poster) rel ids from a media pic.
func mediaPicRelIDs(pic *etree.Element) (mediaRelID, avRelID, posterRelID string) {
	nvPicPr := xmlx.FindChild(pic, ns.NsP, "nvPicPr")
	if nvPicPr != nil {
		if nvPr := xmlx.FindChild(nvPicPr, ns.NsP, "nvPr"); nvPr != nil {
			if vf := xmlx.FindChild(nvPr, ns.NsA, "videoFile"); vf != nil {
				avRelID = rAttrLocal(vf, "link")
			} else if af := xmlx.FindChild(nvPr, ns.NsA, "audioFile"); af != nil {
				avRelID = rAttrLocal(af, "link")
			}
			if p14 := findP14MediaIn(nvPr); p14 != nil {
				mediaRelID = rAttrLocal(p14, "embed")
			}
		}
	}
	if blipFill := xmlx.FindChild(pic, ns.NsP, "blipFill"); blipFill != nil {
		if blip := xmlx.FindChild(blipFill, ns.NsA, "blip"); blip != nil {
			posterRelID = rAttrLocal(blip, "embed")
		}
	}
	return mediaRelID, avRelID, posterRelID
}

// findP14MediaIn locates the p14:media element inside nvPr/extLst/ext.
func findP14MediaIn(nvPr *etree.Element) *etree.Element {
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

// rAttrLocal returns the value of an r:-namespaced attribute (r:embed, r:link).
func rAttrLocal(elem *etree.Element, local string) string {
	for _, attr := range elem.Attr {
		if attr.Key == local && attr.Space == "r" {
			return attr.Value
		}
	}
	v, _ := xmlx.GetAttrNS(elem, ns.NsR, local)
	return v
}

// mediaNodeTargetsSpid reports whether a cMediaNode's tgtEl/spTgt targets spid.
func mediaNodeTargetsSpid(node *etree.Element, spid int) bool {
	tgtEl := xmlx.FindChild(node, ns.NsP, "tgtEl")
	if tgtEl == nil {
		return false
	}
	spTgt := xmlx.FindChild(tgtEl, ns.NsP, "spTgt")
	if spTgt == nil {
		return false
	}
	v, ok := parseIntAttr(spTgt, "spid")
	return ok && v == spid
}

// ---------------------------------------------------------------------------
// Shape-tree helpers.
// ---------------------------------------------------------------------------

// insertShapeAfter inserts pic after the shape with id afterID, or appends when
// afterID<=0 or not found.
func insertShapeAfter(spTree, pic *etree.Element, afterID int) {
	insertSpTreeChildAfterShapeID(spTree, pic, afterID)
}

// ---------------------------------------------------------------------------
// Misc helpers.
// ---------------------------------------------------------------------------

// normalizePlayTrigger canonicalizes the play trigger flag.
func normalizePlayTrigger(s string) string {
	switch strings.ToLower(strings.TrimSpace(s)) {
	case "none":
		return "none"
	default:
		return "click"
	}
}

// clampVolume clamps a 0..100 volume, defaulting to 80 when out of range or zero
// is not explicitly intended. A negative or >100 value is clamped.
func clampVolume(v int) int {
	if v < 0 {
		return 0
	}
	if v > 100 {
		return 100
	}
	return v
}

// synthesizePosterPNG returns a small solid-color PNG used as a placeholder
// poster when the caller supplies none. Encoding via image/png guarantees a
// schema-valid image part that passes strict validation.
func synthesizePosterPNG() []byte {
	const w, h = 16, 16
	img := image.NewRGBA(image.Rect(0, 0, w, h))
	fill := color.RGBA{R: 0x40, G: 0x40, B: 0x40, A: 0xFF}
	for y := 0; y < h; y++ {
		for x := 0; x < w; x++ {
			img.Set(x, y, fill)
		}
	}
	var buf bytes.Buffer
	if err := png.Encode(&buf, img); err != nil {
		return nil
	}
	return buf.Bytes()
}
