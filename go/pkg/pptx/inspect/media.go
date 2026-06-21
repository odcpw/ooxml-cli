package inspect

import (
	"fmt"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	ns "github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

// media.go is the read-only inspector for embedded audio/video clips. It walks
// each slide's shape tree, filters p:pic elements down to media-bearing pictures
// (those carrying a:videoFile / a:audioFile / p14:media / ppaction://media), and
// reports per clip its kind, resolved media/poster part URIs, play trigger,
// volume/mute, and a media-scoped stale block (dangling rel / missing part).
//
// It reuses the pic-parsing helpers shipped with the animations reader
// (mediaFromPic, resolveRel, rAttr, findP14Media) rather than duplicating them,
// and never mutates the package.

// defaultMediaVolume is the assumed playback volume (0..100) when a clip has no
// p:cMediaNode @vol (PowerPoint defaults full-ish playback; 80 mirrors the value
// the mutator writes).
const defaultMediaVolume = 80

// MediaReport summarizes every embedded media clip in a presentation.
type MediaReport struct {
	Slides []MediaSlideInfo `json:"slides"`
}

// MediaSlideInfo holds the media clips on a single slide.
type MediaSlideInfo struct {
	Slide   int         `json:"slide"`   // 1-based presentation order
	PartURI string      `json:"partUri"` // slide part URI
	Clips   []MediaClip `json:"clips"`
}

// MediaClip describes one embedded video/audio p:pic and its playback wiring.
type MediaClip struct {
	ShapeID          int    `json:"shapeId"`
	ShapeName        string `json:"shapeName"`
	Kind             string `json:"kind"`                       // video|audio
	MediaPartURI     string `json:"mediaPartUri"`               // resolved /ppt/media/mediaN.ext ("" if dangling)
	MediaContentType string `json:"mediaContentType,omitempty"` // resolved content type of the media part
	PosterPartURI    string `json:"posterPartUri,omitempty"`
	PlayTrigger      string `json:"playTrigger"` // click|none|cmd
	Volume           int    `json:"volume"`      // 0..100 (from cMediaNode @vol per-mille; default 80)
	Mute             bool   `json:"mute,omitempty"`
	// IsExternal marks a clip whose media/poster rel is TargetMode=External: the
	// MediaPartURI is the raw external target (file URL, drive-letter, or UNC path),
	// not an in-package part, and the clip is never flagged stale for it.
	IsExternal  bool   `json:"isExternal,omitempty"`
	Stale       bool   `json:"stale,omitempty"`
	StaleReason string `json:"staleReason,omitempty"` // e.g. "media rel rId1 missing target part"
}

// ReadMedia builds a MediaReport by walking each slide's shape tree. Slides with
// no embedded media yield an empty clip list. Read-only.
func ReadMedia(session opc.PackageSession) (*MediaReport, error) {
	graph, err := ParsePresentation(session)
	if err != nil {
		return nil, err
	}
	report := &MediaReport{Slides: make([]MediaSlideInfo, 0, len(graph.Slides))}
	for _, slide := range graph.Slides {
		doc, err := session.ReadXMLPart(slide.PartURI)
		if err != nil {
			return nil, fmt.Errorf("failed to read slide %s: %w", slide.PartURI, err)
		}
		info := readSlideMedia(session, slide.PartURI, doc.Root())
		info.Slide = slide.SlideNumber
		info.PartURI = slide.PartURI
		report.Slides = append(report.Slides, info)
	}
	return report, nil
}

func readSlideMedia(session opc.PackageSession, partURI string, root *etree.Element) MediaSlideInfo {
	info := MediaSlideInfo{Clips: []MediaClip{}}
	spTree := findSpTree(root)
	if spTree == nil {
		return info
	}
	idx := buildShapeIndex(spTree)
	timing := xmlx.FindChild(root, ns.NsP, "timing")

	relMap := map[string]opc.RelationshipInfo{}
	for _, rel := range session.ListRelationships(partURI) {
		relMap[rel.ID] = rel
	}
	partSet := map[string]bool{}
	for _, p := range session.ListParts() {
		partSet[p.URI] = true
	}

	for _, pic := range xmlx.FindDescendants(spTree, ns.NsP, "pic") {
		mi, ok := mediaFromPic(session, partURI, pic, relMap, partSet)
		if !ok {
			continue
		}
		clip := MediaClip{
			ShapeID:       mi.Spid,
			Kind:          mi.Kind,
			MediaPartURI:  mi.MediaPartURI,
			PosterPartURI: mi.PosterPartURI,
			IsExternal:    mi.IsExternal,
			Stale:         mi.Stale,
			StaleReason:   mi.StaleReason,
		}
		if mi.Spid != 0 {
			if name, present := idx.names[mi.Spid]; present {
				clip.ShapeName = name
			}
		}
		// Never call GetContentType on an external URI: it is not an in-package part
		// and the raw target (file:/..., C:\..., \\server\...) is not a package key.
		if clip.MediaPartURI != "" && !clip.IsExternal {
			clip.MediaContentType = session.GetContentType(clip.MediaPartURI)
		}
		clip.Volume, clip.Mute = mediaNodeVolume(timing, mi.Spid)
		clip.PlayTrigger = mediaPlayTrigger(pic, timing, mi.Spid)
		info.Clips = append(info.Clips, clip)
	}
	return info
}

// mediaPlayTrigger classifies how a clip is started: "cmd" when an explicit
// p:cmd playFrom targets it, "click" when the pic carries an
// a:hlinkClick@action="ppaction://media", else "none".
func mediaPlayTrigger(pic, timing *etree.Element, spid int) string {
	if timing != nil && spid != 0 && hasClickToPlay(timing, spid) {
		return "cmd"
	}
	if hasMediaHlink(pic) {
		return "click"
	}
	return "none"
}

// hasMediaHlink reports whether the pic's cNvPr carries the click-to-play media
// hyperlink (a:hlinkClick @action="ppaction://media").
func hasMediaHlink(pic *etree.Element) bool {
	nvPicPr := xmlx.FindChild(pic, ns.NsP, "nvPicPr")
	if nvPicPr == nil {
		return false
	}
	cNvPr := xmlx.FindChild(nvPicPr, ns.NsP, "cNvPr")
	if cNvPr == nil {
		return false
	}
	hlink := xmlx.FindChild(cNvPr, ns.NsA, "hlinkClick")
	if hlink == nil {
		return false
	}
	action, _ := xmlx.GetAttr(hlink, "action")
	return action == "ppaction://media"
}

// mediaNodeVolume reads the p:cMediaNode @vol (per-mille) and @mute for the media
// node targeting spid. Returns the default volume when no node/attr is present.
func mediaNodeVolume(timing *etree.Element, spid int) (volume int, mute bool) {
	volume = defaultMediaVolume
	if timing == nil || spid == 0 {
		return volume, false
	}
	for _, node := range xmlx.FindDescendants(timing, ns.NsP, "cMediaNode") {
		if !mediaNodeTargets(node, spid) {
			continue
		}
		if v, ok := parseIntAttr(node, "vol"); ok {
			volume = v / 1000
		}
		if m, _ := xmlx.GetAttr(node, "mute"); m == "1" || m == "true" {
			mute = true
		}
		return volume, mute
	}
	return volume, false
}

// mediaNodeTargets reports whether a p:cMediaNode's tgtEl/spTgt targets spid.
func mediaNodeTargets(node *etree.Element, spid int) bool {
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
