package mutate

import (
	"fmt"
	"regexp"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	ns "github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
)

const hyperlinkRelType = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink"

// SetRunPropertiesRequest holds parameters for run/paragraph-level styling.
type SetRunPropertiesRequest struct {
	Package     opc.PackageSession
	SlideNumber int
	Target      string

	// ParagraphIndex is the 0-based paragraph (a:p) index within the shape.
	ParagraphIndex int

	// RunIndex is the 0-based text-run (a:r) index within the paragraph. When nil,
	// the options are applied to every text run in the paragraph.
	RunIndex *int

	// Hyperlink is the external URL to attach (registers an external relationship).
	Hyperlink *string

	Options *RunMutationOptions
}

// RunPropertiesSnapshot captures the human-facing run properties before/after.
type RunPropertiesSnapshot struct {
	Bold       *bool    `json:"bold,omitempty"`
	Italic     *bool    `json:"italic,omitempty"`
	Underline  string   `json:"underline,omitempty"`
	FontSize   *float64 `json:"fontSize,omitempty"`
	Color      string   `json:"color,omitempty"`
	FontFamily string   `json:"fontFamily,omitempty"`
	Hyperlink  string   `json:"hyperlink,omitempty"`
	Text       string   `json:"text,omitempty"`
}

// SetRunPropertiesResult describes the outcome of a run styling mutation.
type SetRunPropertiesResult struct {
	Slide          int                     `json:"slide"`
	PartURI        string                  `json:"partUri"`
	ShapeID        int                     `json:"shapeId"`
	ShapeName      string                  `json:"shapeName"`
	ShapeType      model.ShapeType         `json:"shapeType"`
	Target         string                  `json:"target"`
	ParagraphIndex int                     `json:"paragraphIndex"`
	RunIndex       *int                    `json:"runIndex,omitempty"`
	AppliedRuns    []int                   `json:"appliedRuns"`
	OldProperties  []RunPropertiesSnapshot `json:"oldProperties"`
	NewProperties  []RunPropertiesSnapshot `json:"newProperties"`
}

// SetRunProperties applies run/paragraph-level text styling to a targeted
// paragraph (and optionally a single run) within a slide shape, preserving
// sibling runs and paragraphs.
func SetRunProperties(req *SetRunPropertiesRequest) (*SetRunPropertiesResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set run properties request is nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if req.SlideNumber < 1 {
		return nil, fmt.Errorf("slide must be >= 1")
	}
	if req.Target == "" {
		return nil, fmt.Errorf("target selector cannot be empty")
	}
	if req.ParagraphIndex < 0 {
		return nil, fmt.Errorf("paragraph index must be >= 0")
	}
	opts := req.Options
	if opts == nil {
		opts = &RunMutationOptions{}
	}

	catalog, err := selectors.BuildSlideCatalog(req.Package, req.SlideNumber)
	if err != nil {
		return nil, err
	}
	target, shapeElem, err := catalog.ResolveTargetElement(req.Target)
	if err != nil {
		return nil, err
	}
	if !target.TextCapable {
		return nil, fmt.Errorf("target %s resolves to a non-text %s shape", req.Target, target.TargetKind)
	}

	txBody := xmlx.FindChild(shapeElem, ns.NsP, "txBody")
	if txBody == nil {
		return nil, fmt.Errorf("target %s has no text body", req.Target)
	}
	paragraphs := xmlx.FindChildren(txBody, ns.NsA, "p")
	if req.ParagraphIndex >= len(paragraphs) {
		return nil, fmt.Errorf("paragraph index %d out of range [0, %d)", req.ParagraphIndex, len(paragraphs))
	}
	pElem := paragraphs[req.ParagraphIndex]

	runs := xmlx.FindChildren(pElem, ns.NsA, "r")
	var targetRuns []*etree.Element
	var appliedIdx []int
	if req.RunIndex != nil {
		idx := *req.RunIndex
		if idx < 0 || idx >= len(runs) {
			return nil, fmt.Errorf("run index %d not found in paragraph %d (paragraph has %d text run(s))", idx, req.ParagraphIndex, len(runs))
		}
		targetRuns = []*etree.Element{runs[idx]}
		appliedIdx = []int{idx}
	} else {
		if len(runs) == 0 {
			return nil, fmt.Errorf("paragraph %d has no text runs to style", req.ParagraphIndex)
		}
		targetRuns = runs
		for i := range runs {
			appliedIdx = append(appliedIdx, i)
		}
	}

	// Validate option values before registering any relationship so a bad value
	// does not leave a dangling hyperlink relationship behind.
	if err := validateRunOptions(opts); err != nil {
		return nil, err
	}

	// Register the hyperlink relationship once (shared across applied runs) if requested.
	if req.Hyperlink != nil {
		url := strings.TrimSpace(*req.Hyperlink)
		if url == "" {
			return nil, fmt.Errorf("hyperlink URL cannot be empty")
		}
		relID, err := registerExternalHyperlink(req.Package, catalog.SlidePartURI, url)
		if err != nil {
			return nil, err
		}
		opts.HyperlinkRelID = &relID
		// The a:hlinkClick attribute is written as r:id, so the slide part root
		// must declare the xmlns:r namespace (mirrors other r:id writers in this
		// package). Gated on the hyperlink actually being set.
		ensureNamespace(catalog.SlideDocument().Root(), "r", ns.NsR)
	}

	oldProps := make([]RunPropertiesSnapshot, 0, len(targetRuns))
	for _, r := range targetRuns {
		oldProps = append(oldProps, snapshotRun(r))
	}

	for _, r := range targetRuns {
		if err := ApplyRunOptions(r, opts); err != nil {
			return nil, err
		}
	}

	newProps := make([]RunPropertiesSnapshot, 0, len(targetRuns))
	for _, r := range targetRuns {
		newProps = append(newProps, snapshotRun(r))
	}

	if err := req.Package.ReplaceXMLPart(catalog.SlidePartURI, catalog.SlideDocument()); err != nil {
		return nil, fmt.Errorf("failed to replace slide %s: %w", catalog.SlidePartURI, err)
	}

	return &SetRunPropertiesResult{
		Slide:          catalog.SlideNumber,
		PartURI:        catalog.SlidePartURI,
		ShapeID:        target.ShapeID,
		ShapeName:      target.ShapeName,
		ShapeType:      target.ShapeType,
		Target:         target.PrimarySelector,
		ParagraphIndex: req.ParagraphIndex,
		RunIndex:       req.RunIndex,
		AppliedRuns:    appliedIdx,
		OldProperties:  oldProps,
		NewProperties:  newProps,
	}, nil
}

// snapshotRun reads the effective styling of a run element into a snapshot by
// inspecting the a:rPr attributes and children directly.
func snapshotRun(rElem *etree.Element) RunPropertiesSnapshot {
	snap := RunPropertiesSnapshot{}
	if t := xmlx.FindChild(rElem, ns.NsA, "t"); t != nil {
		snap.Text = t.Text()
	}
	rPr := xmlx.FindChild(rElem, ns.NsA, "rPr")
	if rPr == nil {
		return snap
	}
	if v, ok := xmlx.GetAttr(rPr, "b"); ok {
		b := v == "1" || v == "true"
		snap.Bold = &b
	}
	if v, ok := xmlx.GetAttr(rPr, "i"); ok {
		i := v == "1" || v == "true"
		snap.Italic = &i
	}
	if v, ok := xmlx.GetAttr(rPr, "u"); ok {
		snap.Underline = v
	}
	if v, ok := xmlx.GetAttr(rPr, "sz"); ok {
		if hundredths, err := strconv.ParseFloat(v, 64); err == nil {
			pts := hundredths / 100.0
			snap.FontSize = &pts
		}
	}
	if latin := xmlx.FindChild(rPr, ns.NsA, "latin"); latin != nil {
		if typeface, ok := xmlx.GetAttr(latin, "typeface"); ok {
			snap.FontFamily = typeface
		}
	}
	if solidFill := xmlx.FindChild(rPr, ns.NsA, "solidFill"); solidFill != nil {
		if srgb := xmlx.FindChild(solidFill, ns.NsA, "srgbClr"); srgb != nil {
			if val, ok := xmlx.GetAttr(srgb, "val"); ok {
				snap.Color = val
			}
		}
	}
	if hlink := xmlx.FindChild(rPr, ns.NsA, "hlinkClick"); hlink != nil {
		if id, ok := xmlx.GetAttr(hlink, "id"); ok {
			snap.Hyperlink = id
		}
	}
	return snap
}

// registerExternalHyperlink adds an external hyperlink relationship to the slide
// part's rels and returns the new relationship ID.
func registerExternalHyperlink(pkg opc.PackageSession, slideURI, url string) (string, error) {
	rels := pkg.ListRelationships(slideURI)
	// Reuse an existing identical external hyperlink relationship when present.
	for _, rel := range rels {
		if rel.Type == hyperlinkRelType && rel.TargetMode == "External" && rel.Target == url {
			return rel.ID, nil
		}
	}
	id := opc.AllocateRelationshipID(rels)
	rels = append(rels, opc.RelationshipInfo{
		ID:         id,
		Type:       hyperlinkRelType,
		Target:     url,
		TargetMode: "External",
	})
	if err := opc.WriteRelationships(pkg, slideURI, rels); err != nil {
		return "", fmt.Errorf("failed to write hyperlink relationship: %w", err)
	}
	return id, nil
}

// RunMutationOptions holds run/paragraph-level text property changes to apply to
// a targeted text run (a:r). Pointer fields distinguish "leave unchanged" (nil)
// from an explicit set/remove. Removal is requested via the Remove* booleans.
type RunMutationOptions struct {
	Bold      *bool // set bold on/off
	Italic    *bool // set italic on/off
	Underline *string

	FontSize   *float64 // points (e.g. 24.0 -> sz="2400")
	Color      *string  // RGB hex, 6 hex digits, no leading '#'
	FontFamily *string  // latin typeface

	// Hyperlink target. When set, a relationship is registered by the caller and
	// HyperlinkRelID is written as a:hlinkClick/@r:id.
	HyperlinkRelID *string

	// Removal flags clear an existing property without setting a value.
	RemoveBold       bool
	RemoveItalic     bool
	RemoveUnderline  bool
	RemoveFontSize   bool
	RemoveColor      bool
	RemoveFontFamily bool
	RemoveHyperlink  bool
}

var (
	colorHexPattern = regexp.MustCompile(`^[0-9A-Fa-f]{6}$`)
	// validUnderlineKind is the DrawingML ST_TextUnderlineType token set
	// (ECMA-376 Part 1). Note these differ from WordprocessingML: single/double
	// underline are "sng"/"dbl" here, not "single"/"double".
	validUnderlineKind = map[string]bool{
		"none":            true,
		"words":           true,
		"sng":             true,
		"dbl":             true,
		"heavy":           true,
		"dotted":          true,
		"dottedHeavy":     true,
		"dash":            true,
		"dashHeavy":       true,
		"dashLong":        true,
		"dashLongHeavy":   true,
		"dotDash":         true,
		"dotDashHeavy":    true,
		"dotDotDash":      true,
		"dotDotDashHeavy": true,
		"wavy":            true,
		"wavyHeavy":       true,
		"wavyDbl":         true,
	}
)

// rPrChildOrder is the ECMA-376 (Part 1, CT_TextCharacterProperties) child
// element sequence for a:rPr / a:endParaRPr / a:defRPr. Children must be written
// in this order for strict schema validation. Attributes (b, i, u, sz, lang,
// strike, baseline) live on the rPr element itself and have no ordering concern.
var rPrChildOrder = []string{
	"ln",
	"noFill", "solidFill", "gradFill", "blipFill", "pattFill", "grpFill",
	"effectLst", "effectDag",
	"highlight",
	"uLnTx", "uLn",
	"uFillTx", "uFill",
	"latin", "ea", "cs", "sym",
	"hlinkClick", "hlinkMouseOver",
	"rtl",
	"extLst",
}

func rPrChildRank(localName string) int {
	for i, name := range rPrChildOrder {
		if name == localName {
			return i
		}
	}
	return len(rPrChildOrder)
}

// ApplyRunOptions applies the requested run property changes to a single text run
// element (a:r), preserving the run's text and any properties not being changed.
func ApplyRunOptions(rElem *etree.Element, opts *RunMutationOptions) error {
	if rElem == nil {
		return fmt.Errorf("run element is nil")
	}
	if opts == nil {
		return nil
	}
	// Validate before touching the run so a bad value leaves it untouched.
	if err := validateRunOptions(opts); err != nil {
		return err
	}
	rPr := getOrCreateRunProperties(rElem)
	applyRunOptionsToRPr(rPr, opts)
	return nil
}

// validateRunOptions checks option values without mutating anything.
func validateRunOptions(opts *RunMutationOptions) error {
	if opts.Underline != nil && !validUnderlineKind[*opts.Underline] {
		return fmt.Errorf("invalid underline %q", *opts.Underline)
	}
	if opts.FontSize != nil && *opts.FontSize <= 0 {
		return fmt.Errorf("invalid font size %g (must be > 0)", *opts.FontSize)
	}
	if opts.Color != nil && !colorHexPattern.MatchString(*opts.Color) {
		return fmt.Errorf("invalid color %q (expected 6 hex digits like FF0000)", *opts.Color)
	}
	return nil
}

// applyRunOptionsToRPr applies validated options to an a:rPr element.
func applyRunOptionsToRPr(rPr *etree.Element, opts *RunMutationOptions) {
	// Bold / italic (attributes).
	if opts.RemoveBold {
		rPr.RemoveAttr("b")
	} else if opts.Bold != nil {
		rPr.CreateAttr("b", boolAttr(*opts.Bold))
	}
	if opts.RemoveItalic {
		rPr.RemoveAttr("i")
	} else if opts.Italic != nil {
		rPr.CreateAttr("i", boolAttr(*opts.Italic))
	}

	// Underline (attribute).
	if opts.RemoveUnderline {
		rPr.RemoveAttr("u")
	} else if opts.Underline != nil {
		rPr.CreateAttr("u", *opts.Underline)
	}

	// Font size (attribute, points -> hundredths).
	if opts.RemoveFontSize {
		rPr.RemoveAttr("sz")
	} else if opts.FontSize != nil {
		rPr.CreateAttr("sz", strconv.FormatInt(int64(*opts.FontSize*100), 10))
	}

	// Color (a:solidFill/a:srgbClr child).
	if opts.RemoveColor {
		removeRPrChild(rPr, "solidFill")
	} else if opts.Color != nil {
		removeRPrChild(rPr, "solidFill")
		solidFill := etree.NewElement("a:solidFill")
		srgbClr := etree.NewElement("a:srgbClr")
		srgbClr.CreateAttr("val", strings.ToUpper(*opts.Color))
		solidFill.AddChild(srgbClr)
		insertRPrChild(rPr, solidFill)
	}

	// Font family (a:latin child).
	if opts.RemoveFontFamily {
		removeRPrChild(rPr, "latin")
	} else if opts.FontFamily != nil {
		removeRPrChild(rPr, "latin")
		latin := etree.NewElement("a:latin")
		latin.CreateAttr("typeface", *opts.FontFamily)
		insertRPrChild(rPr, latin)
	}

	// Hyperlink (a:hlinkClick child with r:id).
	if opts.RemoveHyperlink {
		removeRPrChild(rPr, "hlinkClick")
	} else if opts.HyperlinkRelID != nil {
		removeRPrChild(rPr, "hlinkClick")
		hlink := etree.NewElement("a:hlinkClick")
		hlink.CreateAttr("r:id", *opts.HyperlinkRelID)
		insertRPrChild(rPr, hlink)
	}
}

func boolAttr(v bool) string {
	if v {
		return "1"
	}
	return "0"
}

// getOrCreateRunProperties returns the a:rPr child of a run, creating it as the
// first child (before a:t) when absent, per CT_RegularTextRun ordering.
func getOrCreateRunProperties(rElem *etree.Element) *etree.Element {
	rPr := xmlx.FindChild(rElem, ns.NsA, "rPr")
	if rPr != nil {
		return rPr
	}
	rPr = etree.NewElement("a:rPr")
	// Insert before all other children (a:t must follow a:rPr).
	children := rElem.ChildElements()
	if len(children) > 0 {
		rElem.InsertChildAt(children[0].Index(), rPr)
	} else {
		rElem.AddChild(rPr)
	}
	return rPr
}

// removeRPrChild removes all child elements of rPr with the given local name.
func removeRPrChild(rPr *etree.Element, localName string) {
	for {
		child := xmlx.FindChild(rPr, ns.NsA, localName)
		if child == nil {
			break
		}
		rPr.RemoveChild(child)
	}
}

// insertRPrChild inserts a new child element at its schema-ordered position.
func insertRPrChild(rPr *etree.Element, newChild *etree.Element) {
	rank := rPrChildRank(localTag(newChild.Tag))
	for _, existing := range rPr.ChildElements() {
		if rPrChildRank(localTag(existing.Tag)) > rank {
			rPr.InsertChildAt(existing.Index(), newChild)
			return
		}
	}
	rPr.AddChild(newChild)
}

func localTag(tag string) string {
	if idx := strings.IndexByte(tag, '}'); idx >= 0 {
		return tag[idx+1:]
	}
	if idx := strings.IndexByte(tag, ':'); idx >= 0 {
		return tag[idx+1:]
	}
	return tag
}

// CountTextRuns returns the number of text runs (a:r) in a paragraph element.
func CountTextRuns(pElem *etree.Element) int {
	if pElem == nil {
		return 0
	}
	return len(xmlx.FindChildren(pElem, ns.NsA, "r"))
}

// TextRunAt returns the n-th text run (a:r, 0-based) in a paragraph, skipping
// non-run children (pPr, br, tab, fld, endParaRPr). Returns nil if out of range.
func TextRunAt(pElem *etree.Element, index int) *etree.Element {
	if pElem == nil || index < 0 {
		return nil
	}
	runs := xmlx.FindChildren(pElem, ns.NsA, "r")
	if index >= len(runs) {
		return nil
	}
	return runs[index]
}
