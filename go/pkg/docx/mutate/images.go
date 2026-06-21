package mutate

import (
	"errors"
	"fmt"
	"path"
	"path/filepath"
	"sort"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/imagex"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// DrawingML namespaces used by inline images. These are not part of the docx
// namespaces helper set because the wordprocessingml schema references them only
// through w:drawing content.
const (
	nsDrawingWP  = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
	nsDrawingA   = "http://schemas.openxmlformats.org/drawingml/2006/main"
	nsDrawingPic = "http://schemas.openxmlformats.org/drawingml/2006/picture"
)

var (
	// ErrImageNotFound is returned when an image index/id does not resolve.
	ErrImageNotFound = errors.New("image not found")
)

// ReplaceImageRequest swaps the media bytes (and optionally the EMU extent) of an
// existing inline image, anchored to the containing body block by content hash.
type ReplaceImageRequest struct {
	Package      opc.PackageSession
	DocumentURI  string
	Selector     string // 1-based index or relationship id
	ExpectedHash string // optional sha256: hash of the containing body block
	ImageData    []byte
	ContentType  string
	Width        int64 // EMU; 0 leaves the extent unchanged
	Height       int64 // EMU; 0 leaves the extent unchanged
}

// ReplaceImageResult reports the outcome of ReplaceImage.
type ReplaceImageResult struct {
	Index          int    `json:"index"`
	ID             string `json:"id"`
	BlockIndex     int    `json:"blockIndex"`
	BlockID        string `json:"blockId"`
	BlockHash      string `json:"blockHash"`
	PreviousURI    string `json:"previousUri"`
	PreviousType   string `json:"previousContentType"`
	NewURI         string `json:"newUri"`
	NewContentType string `json:"newContentType"`
	Width          int64  `json:"width"`
	Height         int64  `json:"height"`
}

// InsertImageRequest inserts a new inline image as a body paragraph after a block.
type InsertImageRequest struct {
	Package      opc.PackageSession
	DocumentURI  string
	AfterIndex   int
	ExpectedHash string // optional sha256: hash of the anchor body block (required when AfterIndex > 0)
	ImageData    []byte
	ContentType  string
	Width        int64 // EMU; required
	Height       int64 // EMU; required
}

// InsertImageResult reports the outcome of InsertImage.
type InsertImageResult struct {
	Index          int    `json:"index"`
	ID             string `json:"id"`
	InsertAfter    int    `json:"insertAfter"`
	AnchorHash     string `json:"anchorHash,omitempty"`
	MediaURI       string `json:"mediaUri"`
	NewContentType string `json:"newContentType"`
	Width          int64  `json:"width"`
	Height         int64  `json:"height"`
}

// ReplaceImage swaps the media bytes of an existing inline image. When the new
// content type changes the extension a new media part is allocated; otherwise the
// existing part is replaced in place. The mutation is guarded by the content hash
// of the body block that contains the image.
func ReplaceImage(req *ReplaceImageRequest) (*ReplaceImageResult, error) {
	if req == nil {
		return nil, fmt.Errorf("replace image request is nil")
	}
	if len(req.ImageData) == 0 {
		return nil, fmt.Errorf("image data is empty")
	}
	if req.Width < 0 || req.Height < 0 {
		return nil, fmt.Errorf("image dimensions must be >= 0")
	}

	doc, bodyElem, _, err := locateBody(req.Package, req.DocumentURI)
	if err != nil {
		return nil, err
	}

	target, err := selectImage(bodyElem, req.Selector)
	if err != nil {
		return nil, err
	}

	report := extract.ReportBlock(target.block, false)
	if req.ExpectedHash != "" && req.ExpectedHash != report.ContentHash {
		return nil, fmt.Errorf("%w: block %d expected %s but found %s", ErrBlockHashMismatch, target.block.Index, req.ExpectedHash, report.ContentHash)
	}

	rels := req.Package.ListRelationships(req.DocumentURI)
	oldURI := ""
	for _, rel := range rels {
		if rel.ID == target.embed {
			oldURI = opc.ResolveRelationshipTarget(req.DocumentURI, rel.Target)
			break
		}
	}
	if oldURI == "" {
		return nil, fmt.Errorf("%w: relationship %s does not resolve to a media part", ErrImageNotFound, target.embed)
	}
	oldType := req.Package.GetContentType(oldURI)
	contentType := req.ContentType
	if contentType == "" {
		contentType = oldType
	}
	contentType, err = validateImagePayload(contentType, req.ImageData)
	if err != nil {
		return nil, err
	}

	newURI := oldURI
	if contentType != oldType {
		newExt, err := extensionForImageContentType(contentType)
		if err != nil {
			return nil, err
		}
		oldExt := filepath.Ext(oldURI)
		if newExt != oldExt {
			newURI = strings.TrimSuffix(oldURI, oldExt) + newExt
			newURI = uniqueMediaURI(req.Package, newURI)
		}
	}

	if newURI != oldURI {
		if err := req.Package.AddPart(newURI, req.ImageData, contentType, nil); err != nil {
			return nil, fmt.Errorf("failed to add media part %s: %w", newURI, err)
		}
		updated := opc.RelationshipTarget(req.DocumentURI, newURI)
		for i := range rels {
			if rels[i].ID == target.embed {
				rels[i].Target = updated
				break
			}
		}
		if err := opc.WriteRelationships(req.Package, req.DocumentURI, rels); err != nil {
			return nil, fmt.Errorf("failed to update document relationships: %w", err)
		}
		// The old media part is intentionally preserved; it may be shared.
	} else {
		if err := req.Package.ReplaceRawPart(newURI, req.ImageData, contentType); err != nil {
			return nil, fmt.Errorf("failed to replace media part %s: %w", newURI, err)
		}
	}

	cx, cy := drawingExtent(target.inline)
	if req.Width > 0 {
		cx = req.Width
	}
	if req.Height > 0 {
		cy = req.Height
	}
	if req.Width > 0 || req.Height > 0 {
		setDrawingExtent(target.inline, cx, cy)
	}

	ensureDocumentTableScaffolds(doc.Root(), doc.Root().Space)
	if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
	}

	return &ReplaceImageResult{
		Index:          target.index,
		ID:             target.embed,
		BlockIndex:     target.block.Index,
		BlockID:        report.ID,
		BlockHash:      report.ContentHash,
		PreviousURI:    oldURI,
		PreviousType:   oldType,
		NewURI:         newURI,
		NewContentType: contentType,
		Width:          cx,
		Height:         cy,
	}, nil
}

// InsertImage adds a new media part, a document relationship, and a body paragraph
// containing a w:drawing/wp:inline run referencing it. The new paragraph is inserted
// after the AfterIndex body block (0 inserts before the first block).
func InsertImage(req *InsertImageRequest) (*InsertImageResult, error) {
	if req == nil {
		return nil, fmt.Errorf("insert image request is nil")
	}
	if req.AfterIndex < 0 {
		return nil, fmt.Errorf("insert-after index must be >= 0")
	}
	if len(req.ImageData) == 0 {
		return nil, fmt.Errorf("image data is empty")
	}
	if req.Width <= 0 || req.Height <= 0 {
		return nil, fmt.Errorf("image dimensions must be positive: width=%d height=%d", req.Width, req.Height)
	}
	if req.ContentType == "" {
		return nil, fmt.Errorf("content type is required")
	}
	contentType, err := validateImagePayload(req.ContentType, req.ImageData)
	if err != nil {
		return nil, err
	}

	var (
		doc        *etree.Document
		bodyElem   *etree.Element
		prefix     string
		anchorHash string
		newIndex   int
	)

	if req.AfterIndex == 0 {
		doc, bodyElem, prefix, err = locateBody(req.Package, req.DocumentURI)
		if err != nil {
			return nil, err
		}
		newIndex = 1
	} else {
		var block docxbody.BodyBlock
		doc, bodyElem, block, prefix, err = locateBlock(req.Package, req.DocumentURI, req.AfterIndex)
		if err != nil {
			return nil, err
		}
		report, hashErr := verifyExpectedBlockHash(block, req.ExpectedHash)
		if hashErr != nil {
			return nil, hashErr
		}
		anchorHash = report.ContentHash
		newIndex = req.AfterIndex + 1
	}

	mediaURI, err := allocateMediaURI(req.Package, contentType)
	if err != nil {
		return nil, err
	}
	if err := req.Package.AddPart(mediaURI, req.ImageData, contentType, nil); err != nil {
		return nil, fmt.Errorf("failed to add media part %s: %w", mediaURI, err)
	}

	rels := req.Package.ListRelationships(req.DocumentURI)
	relID := opc.AllocateRelationshipID(rels)
	rels = append(rels, opc.RelationshipInfo{
		SourceURI: req.DocumentURI,
		ID:        relID,
		Type:      namespaces.RelImage,
		Target:    opc.RelationshipTarget(req.DocumentURI, mediaURI),
	})
	if err := opc.WriteRelationships(req.Package, req.DocumentURI, rels); err != nil {
		return nil, fmt.Errorf("failed to write document relationships: %w", err)
	}

	docPrID := nextDocPrID(bodyElem)
	paragraph := buildImageParagraph(doc.Root(), prefix, relID, docPrID, req.Width, req.Height)

	if req.AfterIndex == 0 {
		if firstBlock := firstBodyBlock(bodyElem); firstBlock != nil {
			bodyElem.InsertChildAt(firstBlock.Index(), paragraph)
		} else {
			appendBodyBlock(bodyElem, paragraph)
		}
	} else {
		block := blockByIndex(bodyElem, req.AfterIndex)
		bodyElem.InsertChildAt(block.Element.Index()+1, paragraph)
	}

	ensureDocumentTableScaffolds(doc.Root(), prefix)
	if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
	}

	return &InsertImageResult{
		Index:          newIndex,
		ID:             relID,
		InsertAfter:    req.AfterIndex,
		AnchorHash:     anchorHash,
		MediaURI:       mediaURI,
		NewContentType: contentType,
		Width:          req.Width,
		Height:         req.Height,
	}, nil
}

// imageTarget bundles a resolved inline image with its containing body block.
type imageTarget struct {
	index  int
	embed  string
	inline *etree.Element
	block  docxbody.BodyBlock
}

// selectImage finds an inline image by 1-based index or by relationship id.
func selectImage(bodyElem *etree.Element, selector string) (*imageTarget, error) {
	selector = strings.TrimSpace(selector)
	if selector == "" {
		return nil, fmt.Errorf("%w: empty selector", ErrImageNotFound)
	}
	wantIndex := -1
	if n, err := strconv.Atoi(selector); err == nil {
		if n < 1 {
			return nil, fmt.Errorf("%w: index must be >= 1", ErrImageNotFound)
		}
		wantIndex = n
	}

	count := 0
	for _, block := range docxbody.Blocks(bodyElem) {
		for _, drawing := range namespaces.FindDescendants(block.Element, namespaces.NsW, "drawing") {
			inline := descendantByLocal(drawing, "inline")
			if inline == nil {
				inline = descendantByLocal(drawing, "anchor")
			}
			if inline == nil {
				continue
			}
			blip := descendantByLocal(inline, "blip")
			if blip == nil {
				continue
			}
			embed := blipEmbed(blip)
			if embed == "" {
				continue
			}
			count++
			if (wantIndex > 0 && count == wantIndex) || (wantIndex < 0 && embed == selector) {
				return &imageTarget{index: count, embed: embed, inline: inline, block: block}, nil
			}
		}
	}
	return nil, fmt.Errorf("%w: %s", ErrImageNotFound, selector)
}

func blockByIndex(bodyElem *etree.Element, index int) docxbody.BodyBlock {
	for _, block := range docxbody.Blocks(bodyElem) {
		if block.Index == index {
			return block
		}
	}
	return docxbody.BodyBlock{}
}

// buildImageParagraph constructs <w:p><w:r><w:drawing><wp:inline>...</wp:inline></w:drawing></w:r></w:p>.
func buildImageParagraph(root *etree.Element, prefix, relID string, docPrID int, cx, cy int64) *etree.Element {
	ensureDrawingNamespaces(root)

	paragraph := newElement(prefix, "p")
	run := newElement(prefix, "r")
	drawing := newElement(prefix, "drawing")

	inline := etree.NewElement("wp:inline")
	inline.CreateAttr("distT", "0")
	inline.CreateAttr("distB", "0")
	inline.CreateAttr("distL", "0")
	inline.CreateAttr("distR", "0")

	extent := etree.NewElement("wp:extent")
	extent.CreateAttr("cx", strconv.FormatInt(cx, 10))
	extent.CreateAttr("cy", strconv.FormatInt(cy, 10))
	inline.AddChild(extent)

	effectExtent := etree.NewElement("wp:effectExtent")
	effectExtent.CreateAttr("l", "0")
	effectExtent.CreateAttr("t", "0")
	effectExtent.CreateAttr("r", "0")
	effectExtent.CreateAttr("b", "0")
	inline.AddChild(effectExtent)

	docPr := etree.NewElement("wp:docPr")
	docPr.CreateAttr("id", strconv.Itoa(docPrID))
	docPr.CreateAttr("name", fmt.Sprintf("Picture %d", docPrID))
	inline.AddChild(docPr)

	cNvGraphicFramePr := etree.NewElement("wp:cNvGraphicFramePr")
	graphicFrameLocks := etree.NewElement("a:graphicFrameLocks")
	graphicFrameLocks.CreateAttr("xmlns:a", nsDrawingA)
	graphicFrameLocks.CreateAttr("noChangeAspect", "1")
	cNvGraphicFramePr.AddChild(graphicFrameLocks)
	inline.AddChild(cNvGraphicFramePr)

	graphic := etree.NewElement("a:graphic")
	graphic.CreateAttr("xmlns:a", nsDrawingA)
	graphicData := etree.NewElement("a:graphicData")
	graphicData.CreateAttr("uri", nsDrawingPic)

	pic := etree.NewElement("pic:pic")
	pic.CreateAttr("xmlns:pic", nsDrawingPic)

	nvPicPr := etree.NewElement("pic:nvPicPr")
	cNvPr := etree.NewElement("pic:cNvPr")
	cNvPr.CreateAttr("id", "0")
	cNvPr.CreateAttr("name", fmt.Sprintf("Picture %d", docPrID))
	nvPicPr.AddChild(cNvPr)
	cNvPicPr := etree.NewElement("pic:cNvPicPr")
	nvPicPr.AddChild(cNvPicPr)
	pic.AddChild(nvPicPr)

	blipFill := etree.NewElement("pic:blipFill")
	blip := etree.NewElement("a:blip")
	blip.CreateAttr("r:embed", relID)
	blipFill.AddChild(blip)
	stretch := etree.NewElement("a:stretch")
	stretch.AddChild(etree.NewElement("a:fillRect"))
	blipFill.AddChild(stretch)
	pic.AddChild(blipFill)

	spPr := etree.NewElement("pic:spPr")
	xfrm := etree.NewElement("a:xfrm")
	off := etree.NewElement("a:off")
	off.CreateAttr("x", "0")
	off.CreateAttr("y", "0")
	xfrm.AddChild(off)
	ext := etree.NewElement("a:ext")
	ext.CreateAttr("cx", strconv.FormatInt(cx, 10))
	ext.CreateAttr("cy", strconv.FormatInt(cy, 10))
	xfrm.AddChild(ext)
	spPr.AddChild(xfrm)
	prstGeom := etree.NewElement("a:prstGeom")
	prstGeom.CreateAttr("prst", "rect")
	prstGeom.AddChild(etree.NewElement("a:avLst"))
	spPr.AddChild(prstGeom)
	pic.AddChild(spPr)

	graphicData.AddChild(pic)
	graphic.AddChild(graphicData)
	inline.AddChild(graphic)

	drawing.AddChild(inline)
	run.AddChild(drawing)
	paragraph.AddChild(run)
	return paragraph
}

// ensureDrawingNamespaces declares the wp/r prefixes on the document root so the
// inline image XML round-trips with resolvable prefixes. The a/pic prefixes are
// declared locally on their elements (matching real Office output).
func ensureDrawingNamespaces(root *etree.Element) {
	ensureNamespacePrefix(root, "wp", nsDrawingWP)
	ensureNamespacePrefix(root, "r", namespaces.NsR)
}

// drawingExtent reads the wp:extent cx/cy of a wp:inline/wp:anchor.
func drawingExtent(inline *etree.Element) (int64, int64) {
	extent := childByLocal(inline, "extent")
	if extent == nil {
		return 0, 0
	}
	return parseInt64(extent.SelectAttrValue("cx", "0")), parseInt64(extent.SelectAttrValue("cy", "0"))
}

// setDrawingExtent updates both wp:extent and the inner a:ext (pic:spPr/a:xfrm/a:ext) to cx/cy.
func setDrawingExtent(inline *etree.Element, cx, cy int64) {
	if extent := childByLocal(inline, "extent"); extent != nil {
		extent.CreateAttr("cx", strconv.FormatInt(cx, 10))
		extent.CreateAttr("cy", strconv.FormatInt(cy, 10))
	}
	for _, ext := range descendantsByLocal(inline, "ext") {
		// Skip wp:effectExtent (it carries l/t/r/b, not cx/cy).
		if ext.SelectAttr("cx") == nil && ext.SelectAttr("cy") == nil {
			continue
		}
		ext.CreateAttr("cx", strconv.FormatInt(cx, 10))
		ext.CreateAttr("cy", strconv.FormatInt(cy, 10))
	}
}

func childByLocal(elem *etree.Element, local string) *etree.Element {
	for _, child := range elem.ChildElements() {
		if docxbody.LocalName(child.Tag) == local {
			return child
		}
	}
	return nil
}

func descendantByLocal(elem *etree.Element, local string) *etree.Element {
	if docxbody.LocalName(elem.Tag) == local {
		return elem
	}
	for _, child := range elem.ChildElements() {
		if found := descendantByLocal(child, local); found != nil {
			return found
		}
	}
	return nil
}

func descendantsByLocal(elem *etree.Element, local string) []*etree.Element {
	var out []*etree.Element
	for _, child := range elem.ChildElements() {
		if docxbody.LocalName(child.Tag) == local {
			out = append(out, child)
		}
		out = append(out, descendantsByLocal(child, local)...)
	}
	return out
}

func blipEmbed(blip *etree.Element) string {
	for _, attr := range blip.Attr {
		if attr.Key == "embed" {
			return attr.Value
		}
	}
	return blip.SelectAttrValue("r:embed", "")
}

// nextDocPrID returns max(existing wp:docPr @id) + 1, defaulting to 1.
func nextDocPrID(bodyElem *etree.Element) int {
	maxID := 0
	for _, docPr := range descendantsByLocal(bodyElem, "docPr") {
		if v := docPr.SelectAttrValue("id", ""); v != "" {
			if n, err := strconv.Atoi(v); err == nil && n > maxID {
				maxID = n
			}
		}
	}
	return maxID + 1
}

// allocateMediaURI returns the next free /word/media/imageN.ext URI for the content type.
func allocateMediaURI(session opc.PackageSession, contentType string) (string, error) {
	ext, err := extensionForImageContentType(contentType)
	if err != nil {
		return "", err
	}
	used := make(map[int]bool)
	for _, part := range session.ListParts() {
		uri := opc.NormalizeURI(part.URI)
		if !strings.HasPrefix(uri, "/word/media/image") {
			continue
		}
		base := strings.TrimPrefix(uri, "/word/media/image")
		numStr := strings.TrimSuffix(base, path.Ext(base))
		if n, err := strconv.Atoi(numStr); err == nil {
			used[n] = true
		}
	}
	nums := make([]int, 0, len(used))
	for n := range used {
		nums = append(nums, n)
	}
	sort.Ints(nums)
	next := 1
	for _, n := range nums {
		if n == next {
			next++
		}
	}
	return fmt.Sprintf("/word/media/image%d%s", next, ext), nil
}

// uniqueMediaURI returns the candidate URI or a non-colliding variant.
func uniqueMediaURI(session opc.PackageSession, candidate string) string {
	candidate = opc.NormalizeURI(candidate)
	if !mediaPartExists(session, candidate) {
		return candidate
	}
	ext := path.Ext(candidate)
	base := strings.TrimSuffix(candidate, ext)
	for i := 1; ; i++ {
		next := fmt.Sprintf("%s_%d%s", base, i, ext)
		if !mediaPartExists(session, next) {
			return next
		}
	}
}

func mediaPartExists(session opc.PackageSession, uri string) bool {
	uri = opc.NormalizeURI(uri)
	for _, part := range session.ListParts() {
		if opc.NormalizeURI(part.URI) == uri {
			return true
		}
	}
	return false
}

func validateImagePayload(contentType string, raw []byte) (string, error) {
	normalized := imagex.NormalizedContentType(contentType)
	if !imagex.IsContentType(normalized) {
		return "", fmt.Errorf("unsupported image content type %q", contentType)
	}
	if _, ok := imagex.ExtensionForContentType(normalized); !ok {
		return "", fmt.Errorf("unsupported image content type %q", contentType)
	}
	if imagex.HasKnownSignature(normalized) && !imagex.PayloadMatchesContentType(normalized, raw) {
		return "", fmt.Errorf("image payload does not match content type %s", normalized)
	}
	return normalized, nil
}

func extensionForImageContentType(contentType string) (string, error) {
	switch imagex.NormalizedContentType(contentType) {
	case "image/png":
		return ".png", nil
	case "image/jpeg", "image/jpg", "image/pjpeg":
		return ".jpeg", nil
	}
	ext, ok := imagex.ExtensionForContentType(contentType)
	if !ok {
		return "", fmt.Errorf("unsupported image content type %q", contentType)
	}
	return ext, nil
}

func parseInt64(value string) int64 {
	n, err := strconv.ParseInt(value, 10, 64)
	if err != nil {
		return 0
	}
	return n
}
