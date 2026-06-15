// Package mutate applies safe, semantic mutations to DOCX parts.
package mutate

import (
	"errors"
	"fmt"
	"strings"

	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

var (
	ErrBlockIndexOutOfRange = errors.New("block index out of range")
	ErrBlockNotParagraph    = errors.New("block is not a paragraph")
)

type SetParagraphTextRequest struct {
	Package     opc.PackageSession
	DocumentURI string
	Index       int
	Text        string
}

type SetParagraphTextResult struct {
	Index        int    `json:"index"`
	Style        string `json:"style,omitempty"`
	Text         string `json:"text"`
	PreviousText string `json:"previousText"`
	Flattened    bool   `json:"flattened"`
	// ParaID is the durable w14:paraId marker of the mutated paragraph,
	// injected if the paragraph carried none. It is the value the caller should
	// encode into a stable paragraph handle.
	ParaID string `json:"paraId,omitempty"`
}

type ClearParagraphTextRequest struct {
	Package     opc.PackageSession
	DocumentURI string
	Index       int
}

type ClearParagraphTextResult struct {
	Index        int    `json:"index"`
	Style        string `json:"style,omitempty"`
	PreviousText string `json:"previousText"`
	ParaID       string `json:"paraId,omitempty"`
}

type AppendParagraphRequest struct {
	Package     opc.PackageSession
	DocumentURI string
	Text        string
	Style       string
}

type AppendParagraphResult struct {
	Index int    `json:"index"`
	Style string `json:"style,omitempty"`
	Text  string `json:"text"`
}

type InsertParagraphRequest struct {
	Package     opc.PackageSession
	DocumentURI string
	AfterIndex  int
	Text        string
	Style       string
}

type InsertParagraphResult struct {
	Index int    `json:"index"`
	Style string `json:"style,omitempty"`
	Text  string `json:"text"`
}

func SetParagraphText(req *SetParagraphTextRequest) (*SetParagraphTextResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set paragraph text request is nil")
	}
	doc, paragraph, prefix, err := locateParagraph(req.Package, req.DocumentURI, req.Index)
	if err != nil {
		return nil, err
	}

	previousText := docxbody.ParagraphText(paragraph)
	style := docxbody.ParagraphStyle(paragraph)
	rPrCopy := firstDirectRunProperties(paragraph)
	flattened := clearParagraphChildren(paragraph)

	run := newElement(prefix, "r")
	if rPrCopy != nil {
		run.AddChild(rPrCopy)
	}
	appendTextChildren(run, prefix, req.Text)
	paragraph.AddChild(run)

	// LAZY-UPGRADE: stamp a durable paraId on the mutated paragraph (idempotent;
	// the attribute survives this text rewrite, which is what makes a handle to a
	// translated paragraph keep resolving).
	paraID := ""
	if root := doc.Root(); root != nil {
		if bodyElem, ferr := docxbody.FindBody(root); ferr == nil {
			paraID = ensureParagraphMarker(root, bodyElem, paragraph)
		}
	}

	ensureDocumentTableScaffolds(doc.Root(), prefix)
	if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
	}
	return &SetParagraphTextResult{
		Index:        req.Index,
		Style:        style,
		Text:         req.Text,
		PreviousText: previousText,
		Flattened:    flattened,
		ParaID:       paraID,
	}, nil
}

func ClearParagraphText(req *ClearParagraphTextRequest) (*ClearParagraphTextResult, error) {
	if req == nil {
		return nil, fmt.Errorf("clear paragraph text request is nil")
	}
	doc, paragraph, _, err := locateParagraph(req.Package, req.DocumentURI, req.Index)
	if err != nil {
		return nil, err
	}

	previousText := docxbody.ParagraphText(paragraph)
	style := docxbody.ParagraphStyle(paragraph)
	clearParagraphChildren(paragraph)

	paraID := ""
	if root := doc.Root(); root != nil {
		if bodyElem, ferr := docxbody.FindBody(root); ferr == nil {
			paraID = ensureParagraphMarker(root, bodyElem, paragraph)
		}
	}

	ensureDocumentTableScaffolds(doc.Root(), doc.Root().Space)
	if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
	}
	return &ClearParagraphTextResult{
		Index:        req.Index,
		Style:        style,
		PreviousText: previousText,
		ParaID:       paraID,
	}, nil
}

func AppendParagraph(req *AppendParagraphRequest) (*AppendParagraphResult, error) {
	if req == nil {
		return nil, fmt.Errorf("append paragraph request is nil")
	}
	doc, bodyElem, prefix, err := locateBody(req.Package, req.DocumentURI)
	if err != nil {
		return nil, err
	}

	newParagraph := buildParagraph(doc.Root(), prefix, req.Text, req.Style)
	index := len(docxbody.Blocks(bodyElem)) + 1
	appendBodyBlock(bodyElem, newParagraph)

	ensureDocumentTableScaffolds(doc.Root(), prefix)
	if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
	}
	return &AppendParagraphResult{
		Index: index,
		Style: req.Style,
		Text:  req.Text,
	}, nil
}

func InsertParagraph(req *InsertParagraphRequest) (*InsertParagraphResult, error) {
	if req == nil {
		return nil, fmt.Errorf("insert paragraph request is nil")
	}
	if req.AfterIndex < 0 {
		return nil, fmt.Errorf("insert-after index must be >= 0")
	}

	var (
		doc      *etree.Document
		bodyElem *etree.Element
		prefix   string
		err      error
	)
	if req.AfterIndex == 0 {
		doc, bodyElem, prefix, err = locateBody(req.Package, req.DocumentURI)
		if err != nil {
			return nil, err
		}
	} else {
		var block docxbody.BodyBlock
		doc, bodyElem, block, prefix, err = locateBlock(req.Package, req.DocumentURI, req.AfterIndex)
		if err != nil {
			return nil, err
		}
		newParagraph := buildParagraph(doc.Root(), prefix, req.Text, req.Style)
		bodyElem.InsertChildAt(block.Element.Index()+1, newParagraph)
		ensureDocumentTableScaffolds(doc.Root(), prefix)
		if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
			return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
		}
		return &InsertParagraphResult{
			Index: req.AfterIndex + 1,
			Style: req.Style,
			Text:  req.Text,
		}, nil
	}

	newParagraph := buildParagraph(doc.Root(), prefix, req.Text, req.Style)
	if firstBlock := firstBodyBlock(bodyElem); firstBlock != nil {
		bodyElem.InsertChildAt(firstBlock.Index(), newParagraph)
	} else {
		appendBodyBlock(bodyElem, newParagraph)
	}

	ensureDocumentTableScaffolds(doc.Root(), prefix)
	if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
	}
	return &InsertParagraphResult{
		Index: 1,
		Style: req.Style,
		Text:  req.Text,
	}, nil
}

func locateParagraph(session opc.PackageSession, documentURI string, index int) (*etree.Document, *etree.Element, string, error) {
	if session == nil {
		return nil, nil, "", fmt.Errorf("package session is nil")
	}
	if documentURI == "" {
		return nil, nil, "", fmt.Errorf("document URI is required")
	}
	if index < 1 {
		return nil, nil, "", fmt.Errorf("paragraph index must be >= 1")
	}

	doc, err := session.ReadXMLPart(documentURI)
	if err != nil {
		return nil, nil, "", fmt.Errorf("failed to read document part %s: %w", documentURI, err)
	}
	root := doc.Root()
	bodyElem, err := docxbody.FindBody(root)
	if err != nil {
		return nil, nil, "", err
	}

	for _, block := range docxbody.Blocks(bodyElem) {
		if block.Index != index {
			continue
		}
		if block.Kind != model.BlockKindParagraph {
			return nil, nil, "", fmt.Errorf("%w: block %d is %s", ErrBlockNotParagraph, index, block.Kind)
		}
		return doc, block.Element, root.Space, nil
	}
	return nil, nil, "", fmt.Errorf("%w: %d", ErrBlockIndexOutOfRange, index)
}

func locateBlock(session opc.PackageSession, documentURI string, index int) (*etree.Document, *etree.Element, docxbody.BodyBlock, string, error) {
	doc, bodyElem, prefix, err := locateBody(session, documentURI)
	if err != nil {
		return nil, nil, docxbody.BodyBlock{}, "", err
	}
	if index < 1 {
		return nil, nil, docxbody.BodyBlock{}, "", fmt.Errorf("block index must be >= 1")
	}
	for _, block := range docxbody.Blocks(bodyElem) {
		if block.Index == index {
			return doc, bodyElem, block, prefix, nil
		}
	}
	return nil, nil, docxbody.BodyBlock{}, "", fmt.Errorf("%w: %d", ErrBlockIndexOutOfRange, index)
}

func locateBody(session opc.PackageSession, documentURI string) (*etree.Document, *etree.Element, string, error) {
	if session == nil {
		return nil, nil, "", fmt.Errorf("package session is nil")
	}
	if documentURI == "" {
		return nil, nil, "", fmt.Errorf("document URI is required")
	}

	doc, err := session.ReadXMLPart(documentURI)
	if err != nil {
		return nil, nil, "", fmt.Errorf("failed to read document part %s: %w", documentURI, err)
	}
	root := doc.Root()
	bodyElem, err := docxbody.FindBody(root)
	if err != nil {
		return nil, nil, "", err
	}
	return doc, bodyElem, root.Space, nil
}

func firstDirectRunProperties(paragraph *etree.Element) *etree.Element {
	for _, run := range namespaces.FindChildren(paragraph, namespaces.NsW, "r") {
		if rPr := namespaces.FindChild(run, namespaces.NsW, "rPr"); rPr != nil {
			return rPr.Copy()
		}
	}
	return nil
}

func clearParagraphChildren(paragraph *etree.Element) bool {
	flattened := false
	for _, child := range paragraph.ChildElements() {
		if docxbody.LocalName(child.Tag) == "pPr" {
			continue
		}
		if docxbody.LocalName(child.Tag) != "r" {
			flattened = true
		}
		paragraph.RemoveChild(child)
	}
	return flattened
}

func buildParagraph(root *etree.Element, prefix, text, style string) *etree.Element {
	paragraph := newElement(prefix, "p")
	if style != "" {
		pPr := newElement(prefix, "pPr")
		pStyle := newElement(prefix, "pStyle")
		pStyle.CreateAttr(qualifiedWordAttrName(root, prefix, "val"), style)
		pPr.AddChild(pStyle)
		paragraph.AddChild(pPr)
	}
	if text != "" {
		run := newElement(prefix, "r")
		appendTextChildren(run, prefix, text)
		paragraph.AddChild(run)
	}
	return paragraph
}

func appendBodyBlock(bodyElem *etree.Element, block *etree.Element) {
	if sectPr := bodySectPr(bodyElem); sectPr != nil {
		bodyElem.InsertChildAt(sectPr.Index(), block)
		return
	}
	bodyElem.AddChild(block)
}

func firstBodyBlock(bodyElem *etree.Element) *etree.Element {
	for _, child := range bodyElem.ChildElements() {
		switch docxbody.LocalName(child.Tag) {
		case "p", "tbl":
			return child
		}
	}
	return nil
}

func bodySectPr(bodyElem *etree.Element) *etree.Element {
	for _, child := range bodyElem.ChildElements() {
		if docxbody.LocalName(child.Tag) == "sectPr" {
			return child
		}
	}
	return nil
}

func qualifiedWordAttrName(root *etree.Element, prefix, local string) string {
	if prefix != "" {
		return prefix + ":" + local
	}
	ensureNamespacePrefix(root, "w", namespaces.NsW)
	return "w:" + local
}

func ensureNamespacePrefix(root *etree.Element, prefix, uri string) {
	if root == nil || prefix == "" || uri == "" {
		return
	}
	attrName := "xmlns:" + prefix
	if attr := root.SelectAttr(attrName); attr != nil {
		return
	}
	root.CreateAttr(attrName, uri)
}

func appendTextChildren(run *etree.Element, prefix, text string) {
	lines := strings.Split(text, "\n")
	for lineIndex, line := range lines {
		if lineIndex > 0 {
			run.AddChild(newElement(prefix, "br"))
		}
		segments := strings.Split(line, "\t")
		for segmentIndex, segment := range segments {
			if segmentIndex > 0 {
				run.AddChild(newElement(prefix, "tab"))
			}
			if segment == "" {
				continue
			}
			t := newElement(prefix, "t")
			if needsSpacePreserve(segment) {
				t.CreateAttr("xml:space", "preserve")
			}
			t.SetText(segment)
			run.AddChild(t)
		}
	}
}

func needsSpacePreserve(value string) bool {
	return value != strings.Trim(value, " \t\r\n")
}

func newElement(prefix, tag string) *etree.Element {
	elem := etree.NewElement(tag)
	elem.Space = prefix
	return elem
}
