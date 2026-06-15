package mutate

// animspec.go isolates every animation token whose exact spelling is NOT fully
// confirmed against a real-PowerPoint render. Per the TOKEN POLICY, presetID /
// presetClass / presetSubtype are animation-pane METADATA: PowerPoint renders off
// the BEHAVIOR elements (p:set / p:animEffect / p:anim), so a wrong preset value
// only mislabels the effect in the UI. The genuinely load-bearing tokens are the
// p:animEffect @filter strings and the p:bldP @build token; those are grounded in
// ECMA-376 / golden PowerPoint output below.
//
// Keeping all of these in one file means a later golden-fixture reconciliation is
// a single-file edit.

// ----------------------------------------------------------------------------
// presetClass / preset metadata (animation-pane hints only; render-irrelevant).
// ----------------------------------------------------------------------------

// presetClassEntrance is the presetClass for all four in-scope entrance effects.
const presetClassEntrance = "entr"

// effectPreset bundles the advisory preset metadata for one entrance kind.
type effectPreset struct {
	presetID      string
	presetSubtype string
}

// presetByEffect maps each in-scope entrance kind to its advisory preset hints.
//
//   - appear: presetID=1, subtype=0  -- golden-confirmed (real PowerPoint, 1640x).
//   - wipe:   presetID=22, subtype=1 -- golden-confirmed (real PowerPoint, from-bottom "up").
//   - fade:   presetID=10            -- spec-grounded; PowerPoint-render unconfirmed.
//   - flyIn:  presetID=2, subtype=4  -- spec-grounded (single corroborating data point); render unconfirmed.
//
// Only the appear/wipe values are golden-locked. fade/flyIn preset numbers are
// best-effort; because PowerPoint renders off behaviors, a wrong number here only
// mislabels the effect in the animation pane, never the playback.
var presetByEffect = map[string]effectPreset{
	"appear": {presetID: "1", presetSubtype: "0"},
	"fade":   {presetID: "10", presetSubtype: "0"},
	"wipe":   {presetID: "22", presetSubtype: "1"},
	"flyIn":  {presetID: "2", presetSubtype: "4"},
}

// ----------------------------------------------------------------------------
// Load-bearing tokens: p:animEffect @filter strings.
// ----------------------------------------------------------------------------

// ECMA-376 (Part 1, CT_TLAnimateEffectBehavior/@filter) defines @filter as an
// xsd:string with the syntax "type(subtype);type(subtype)" where the subtype is
// optional; the spec's worked example is filter="blinds(horizontal)". The
// specific spellings below follow that grammar:
//
//   - filterFade       -> "fade"            spec-grounded (type-only, subtype omitted);
//     PowerPoint-render unconfirmed.
//   - filterWipe(dir)  -> "wipe(up|down|left|right)"  golden-confirmed for wipe(up)
//     from real PowerPoint output; the other
//     directions follow the same type(subtype)
//     grammar (render-unconfirmed for non-up).
const filterFade = "fade"

// wipeFilterByDirection maps a direction flag to the wipe(dir) filter subtype.
// "up" is golden-confirmed; the rest are spec-grammar-grounded, render-unconfirmed.
var wipeFilterByDirection = map[string]string{
	"up":    "wipe(up)",
	"down":  "wipe(down)",
	"left":  "wipe(left)",
	"right": "wipe(right)",
}

// ----------------------------------------------------------------------------
// Load-bearing token: p:bldP @build.
// ----------------------------------------------------------------------------

// buildByParagraph is the ST_TLParaBuildType enum member that builds a shape one
// first-level paragraph at a time. The enum is {allAtOnce, p, cust, whole}; real
// PowerPoint writes build="p" for by-paragraph (golden-confirmed). NOTE: the
// human-readable "byParagraph" is NOT a schema value and would fail strict
// validation -- the schema token is "p".
const buildByParagraph = "p"

// ----------------------------------------------------------------------------
// Fly-in motion: ppt_x / ppt_y animation endpoints.
// ----------------------------------------------------------------------------

// Fly-in animates the shape's normalized position variable from an off-slide
// value to its final position via p:anim/p:tavLst. The "#ppt_*" / "1+#ppt_h/2"
// forms follow the MS Learn animation walkthrough (spec-grounded; render
// unconfirmed). Direction selects which axis (ppt_x or ppt_y) and the start value.
type flyInMotion struct {
	attrName string // ppt_x | ppt_y
	from     string // start (off-slide) value
	to       string // final value
}

// flyInMotionByDirection maps a direction to its motion endpoints. "up" enters
// from below the slide (ppt_y high -> final); "down" from above; "left"/"right"
// along ppt_x. spec-grounded; PowerPoint-render unconfirmed.
var flyInMotionByDirection = map[string]flyInMotion{
	"up":    {attrName: "ppt_y", from: "1+#ppt_h/2", to: "#ppt_y"},
	"down":  {attrName: "ppt_y", from: "0-#ppt_h/2", to: "#ppt_y"},
	"left":  {attrName: "ppt_x", from: "0-#ppt_w/2", to: "#ppt_x"},
	"right": {attrName: "ppt_x", from: "1+#ppt_w/2", to: "#ppt_x"},
}

// ----------------------------------------------------------------------------
// Media tokens (embedded audio/video). Isolated here so a later golden-fixture
// reconciliation is a single-file edit.
// ----------------------------------------------------------------------------

// mediaExtURI is the p:ext URI carrying the modern p14:media reference inside the
// p:pic's nvPr/extLst. This GUID is fixed by the Microsoft media extension and is
// Tier A (python-pptx / real-deck confirmed).
const mediaExtURI = "{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}"

// hlinkMediaAction is the a:hlinkClick @action that wires a media pic for
// click-to-play. Tier A: confirmed from python-pptx analysis and real decks.
//
// spec-grounded; PowerPoint-render confirmed via python-pptx: the
// "ppaction://media" action is the standard click-to-play hook for embedded AV.
const hlinkMediaAction = "ppaction://media"

// playFromCmd is the OPTIONAL Tier-D active click-to-play timing trigger string.
// PowerPoint renders click-to-play off the a:hlinkClick action + passive media
// node (both Tier A); this explicit p:cmd is an opt-in enhancement only.
//
// spec-grounded; PowerPoint-render unconfirmed: the grammar (CT_TLCommandBehavior
// p:cmd type="call" cmd="<command>") is confirmed, but the exact "playFrom(0.0)"
// spelling and whether it must be parented under an interactiveSeq/excl rather
// than the mainSeq await a real-PowerPoint golden fixture. Gated behind --play-cmd.
const playFromCmd = "playFrom(0.0)"

// Media relationship type URIs. The "media" type is in the Microsoft 2007
// namespace; video/audio/image are the standard OOXML 2006 types. All Tier A.
const (
	relTypeMedia = "http://schemas.microsoft.com/office/2007/relationships/media"
	relTypeVideo = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/video"
	relTypeAudio = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/audio"
	relTypeImage = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
)
