package extract

import (
	"fmt"
	"strconv"

	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// ImageReport describes a single inline image (w:drawing/wp:inline) resolved to
// its media part and EMU extent.
type ImageReport struct {
	Index           int      `json:"index"`
	ID              string   `json:"id"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
	BlockIndex      int      `json:"blockIndex"`
	BlockID         string   `json:"blockId"`
	BlockHash       string   `json:"blockHash"`
	BlipID          string   `json:"blipId"`
	MediaURI        string   `json:"mediaUri"`
	ContentType     string   `json:"contentType"`
	Width           int64    `json:"width"`
	Height          int64    `json:"height"`
}

// ExtractImagesRequest holds inputs for ExtractImages.
type ExtractImagesRequest struct {
	Session     opc.PackageSession
	DocumentURI string
}

// ExtractedImages is the document-level result of ExtractImages.
type ExtractedImages struct {
	File            string        `json:"file,omitempty"`
	DocumentPartURI string        `json:"documentPartUri"`
	Images          []ImageReport `json:"images"`
}

// ExtractImages walks the main-document body and resolves every inline image
// (w:drawing/wp:inline/a:graphic/.../pic:pic/pic:blipFill/a:blip @r:embed) to its
// backing media part via the document relationships, reporting the EMU extent.
func ExtractImages(req *ExtractImagesRequest) (*ExtractedImages, error) {
	if req == nil {
		return nil, fmt.Errorf("extract images request is nil")
	}
	if req.Session == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if req.DocumentURI == "" {
		return nil, fmt.Errorf("document URI is required")
	}

	doc, err := req.Session.ReadXMLPart(req.DocumentURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read document part %s: %w", req.DocumentURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsW, "document") {
		return nil, fmt.Errorf("document root element not found")
	}
	bodyElem, err := docxbody.FindBody(root)
	if err != nil {
		return nil, err
	}

	relTargets := imageRelationshipTargets(req.Session, req.DocumentURI)

	result := &ExtractedImages{
		DocumentPartURI: req.DocumentURI,
		Images:          make([]ImageReport, 0),
	}

	for _, block := range docxbody.Blocks(bodyElem) {
		blockReport := ReportBlock(block, false)
		for _, drawing := range namespaces.FindDescendants(block.Element, namespaces.NsW, "drawing") {
			inline := findFirstByLocal(drawing, "inline")
			if inline == nil {
				// Anchored (floating) images use wp:anchor; resolve those too.
				inline = findFirstByLocal(drawing, "anchor")
			}
			if inline == nil {
				continue
			}
			blip := findFirstByLocal(inline, "blip")
			if blip == nil {
				continue
			}
			embed := blipEmbedID(blip)
			if embed == "" {
				continue
			}
			mediaURI := relTargets[embed]
			contentType := ""
			if mediaURI != "" {
				contentType = req.Session.GetContentType(mediaURI)
			}
			cx, cy := inlineExtent(inline)
			report := ImageReport{
				Index:       len(result.Images) + 1,
				BlockIndex:  block.Index,
				BlockID:     blockReport.ID,
				BlockHash:   blockReport.ContentHash,
				BlipID:      embed,
				MediaURI:    mediaURI,
				ContentType: contentType,
				Width:       cx,
				Height:      cy,
			}
			report.PrimarySelector = strconv.Itoa(report.Index)
			report.Selectors = []string{report.PrimarySelector}
			report.ID = embed
			result.Images = append(result.Images, report)
		}
	}
	return result, nil
}

// imageRelationshipTargets maps relationship IDs of image relationships to their
// resolved (absolute) part URIs.
func imageRelationshipTargets(session opc.PackageSession, documentURI string) map[string]string {
	targets := make(map[string]string)
	for _, rel := range session.ListRelationships(documentURI) {
		if rel.Type != namespaces.RelImage || rel.TargetMode == "External" {
			continue
		}
		targets[rel.ID] = opc.ResolveRelationshipTarget(documentURI, rel.Target)
	}
	return targets
}

// findFirstByLocal returns the first descendant (or self) with the given local name,
// ignoring namespace prefix. Drawing content lives in the wp/a/pic namespaces which
// are not part of the docx namespaces helper set, so match by local name only.
func findFirstByLocal(elem *etree.Element, local string) *etree.Element {
	if docxbody.LocalName(elem.Tag) == local {
		return elem
	}
	for _, child := range elem.ChildElements() {
		if found := findFirstByLocal(child, local); found != nil {
			return found
		}
	}
	return nil
}

// blipEmbedID extracts the r:embed attribute value from an a:blip element,
// tolerating prefix/namespace variation.
func blipEmbedID(blip *etree.Element) string {
	for _, attr := range blip.Attr {
		if attr.Key == "embed" {
			return attr.Value
		}
	}
	return blip.SelectAttrValue("r:embed", "")
}

// inlineExtent returns the cx/cy EMU extent from wp:extent on a wp:inline/wp:anchor.
func inlineExtent(inline *etree.Element) (int64, int64) {
	extent := findExtent(inline)
	if extent == nil {
		return 0, 0
	}
	cx := attrInt64(extent, "cx")
	cy := attrInt64(extent, "cy")
	return cx, cy
}

// findExtent returns the wp:extent child of a wp:inline/wp:anchor without
// recursing into the inner graphic (which has its own a:ext extents).
func findExtent(inline *etree.Element) *etree.Element {
	for _, child := range inline.ChildElements() {
		if docxbody.LocalName(child.Tag) == "extent" {
			return child
		}
	}
	return nil
}

func attrInt64(elem *etree.Element, key string) int64 {
	value := elem.SelectAttrValue(key, "")
	if value == "" {
		return 0
	}
	var n int64
	if _, err := fmt.Sscan(value, &n); err != nil {
		return 0
	}
	return n
}
