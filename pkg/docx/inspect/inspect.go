package inspect

import (
	"fmt"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func FindMainDocumentPart(session opc.PackageSession) (string, error) {
	if session == nil {
		return "", fmt.Errorf("package session is nil")
	}

	for _, rel := range session.ListRelationships("/") {
		if rel.TargetMode == "External" {
			continue
		}
		targetURI := resolveTargetURI("/", rel.Target)
		if rel.Type == namespaces.RelOfficeDocument || isDocumentCandidate(session, targetURI) {
			return targetURI, nil
		}
	}

	for _, part := range session.ListParts() {
		if isDocumentContentType(part.ContentType) {
			return opc.NormalizeURI(part.URI), nil
		}
	}

	return "", fmt.Errorf("docx main document part not found")
}

func ParseDocument(session opc.PackageSession) (*model.Document, error) {
	documentURI, err := FindMainDocumentPart(session)
	if err != nil {
		return nil, err
	}

	doc, err := session.ReadXMLPart(documentURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read document part %s: %w", documentURI, err)
	}
	root := doc.Root()
	if root == nil {
		return nil, fmt.Errorf("document part %s has no root element", documentURI)
	}
	if !isDocumentRoot(root) {
		return nil, fmt.Errorf("document part %s root is %q, expected document", documentURI, root.Tag)
	}

	document := &model.Document{PartURI: documentURI}
	for _, rel := range session.ListRelationships(documentURI) {
		if rel.TargetMode == "External" {
			continue
		}
		targetURI := resolveTargetURI(documentURI, rel.Target)
		switch rel.Type {
		case namespaces.RelStyles:
			document.StylesURI = targetURI
		case namespaces.RelNumbering:
			document.NumberingURI = targetURI
		}
	}

	return document, nil
}

func SummarizeDocument(session opc.PackageSession) (*model.DocumentSummary, error) {
	document, err := ParseDocument(session)
	if err != nil {
		return nil, err
	}

	paragraphs, tables, hyperlinks, sections, err := CountBody(session, document.PartURI)
	if err != nil {
		return nil, err
	}

	summary := &model.DocumentSummary{
		Type:            string(opc.PackageTypeDOCX),
		DocumentPartURI: document.PartURI,
		Paragraphs:      paragraphs,
		Tables:          tables,
		Hyperlinks:      hyperlinks,
		Sections:        sections,
	}

	for _, part := range session.ListParts() {
		uri := opc.NormalizeURI(part.URI)
		contentType := part.ContentType

		switch {
		case isStylesPart(uri, contentType):
			summary.Styles = true
		case isNumberingPart(uri, contentType):
			summary.Numbering = true
		case isHeaderPart(uri, contentType):
			summary.Headers++
		case isFooterPart(uri, contentType):
			summary.Footers++
		case isFootnotesPart(uri, contentType):
			summary.Footnotes = true
		case isEndnotesPart(uri, contentType):
			summary.Endnotes = true
		case isCommentsPart(uri, contentType):
			summary.Comments = true
		case isMediaPart(uri):
			summary.MediaAssets++
		case isCustomXMLPart(uri):
			summary.CustomXMLParts++
		}
	}

	return summary, nil
}

func CountBody(session opc.PackageSession, documentURI string) (paragraphs, tables, hyperlinks, sections int, err error) {
	doc, err := session.ReadXMLPart(documentURI)
	if err != nil {
		return 0, 0, 0, 0, err
	}
	root := doc.Root()
	if !isDocumentRoot(root) {
		return 0, 0, 0, 0, fmt.Errorf("document root element not found")
	}
	body := namespaces.FindChild(root, namespaces.NsW, "body")
	if body == nil {
		return 0, 0, 0, 0, fmt.Errorf("document body element not found")
	}

	for _, child := range body.ChildElements() {
		switch localName(child.Tag) {
		case "p":
			paragraphs++
			hyperlinks += len(namespaces.FindDescendants(child, namespaces.NsW, "hyperlink"))
		case "tbl":
			tables++
			hyperlinks += len(namespaces.FindDescendants(child, namespaces.NsW, "hyperlink"))
		case "sectPr":
			sections++
		}
	}
	if sections == 0 {
		sections = len(namespaces.FindDescendants(body, namespaces.NsW, "sectPr"))
	}
	return paragraphs, tables, hyperlinks, sections, nil
}

func resolveTargetURI(sourceURI, target string) string {
	if strings.HasPrefix(target, "/") {
		return opc.NormalizeURI(target)
	}
	return opc.NormalizeURI(opc.ResolveRelationshipTarget(sourceURI, target))
}

func isDocumentCandidate(session opc.PackageSession, uri string) bool {
	if uri == "" || uri == "/" {
		return false
	}
	if isDocumentContentType(session.GetContentType(uri)) {
		return true
	}
	return uri == "/word/document.xml"
}

func isDocumentRoot(root *etree.Element) bool {
	return root != nil && namespaces.IsElement(root, namespaces.NsW, "document")
}

func isDocumentContentType(contentType string) bool {
	return contentType == namespaces.ContentTypeDocument ||
		strings.Contains(contentType, "wordprocessingml.document.main+xml")
}

func isStylesPart(uri, contentType string) bool {
	return contentType == namespaces.ContentTypeStyles || uri == "/word/styles.xml"
}

func isNumberingPart(uri, contentType string) bool {
	return contentType == namespaces.ContentTypeNumbering || uri == "/word/numbering.xml"
}

func isHeaderPart(uri, contentType string) bool {
	return contentType == namespaces.ContentTypeHeader || isXMLDataPart(uri) && strings.HasPrefix(uri, "/word/header")
}

func isFooterPart(uri, contentType string) bool {
	return contentType == namespaces.ContentTypeFooter || isXMLDataPart(uri) && strings.HasPrefix(uri, "/word/footer")
}

func isFootnotesPart(uri, contentType string) bool {
	return contentType == namespaces.ContentTypeFootnotes || uri == "/word/footnotes.xml"
}

func isEndnotesPart(uri, contentType string) bool {
	return contentType == namespaces.ContentTypeEndnotes || uri == "/word/endnotes.xml"
}

func isCommentsPart(uri, contentType string) bool {
	return contentType == namespaces.ContentTypeComments || uri == "/word/comments.xml"
}

func isMediaPart(uri string) bool {
	return strings.HasPrefix(uri, "/word/media/") && !strings.Contains(uri, "/_rels/")
}

func isCustomXMLPart(uri string) bool {
	return isXMLDataPart(uri) && strings.HasPrefix(uri, "/customXml/")
}

func isXMLDataPart(uri string) bool {
	return strings.HasSuffix(uri, ".xml") && !strings.Contains(uri, "/_rels/") && !strings.HasSuffix(uri, ".rels")
}

func localName(tag string) string {
	if idx := strings.LastIndex(tag, "}"); idx >= 0 && idx+1 < len(tag) {
		return tag[idx+1:]
	}
	if idx := strings.LastIndex(tag, ":"); idx >= 0 && idx+1 < len(tag) {
		return tag[idx+1:]
	}
	return tag
}
