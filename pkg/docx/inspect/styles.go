package inspect

import (
	"fmt"

	"github.com/beevik/etree"
	docxhandle "github.com/ooxml-cli/ooxml-cli/pkg/docx/handle"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// FindStylesPart resolves the URI of the word/styles.xml part for a DOCX
// package. It returns an empty string (and no error) when the optional styles
// part is not present.
func FindStylesPart(session opc.PackageSession) (string, error) {
	if session == nil {
		return "", fmt.Errorf("package session is nil")
	}

	documentURI, err := FindMainDocumentPart(session)
	if err != nil {
		return "", err
	}

	for _, rel := range session.ListRelationships(documentURI) {
		if rel.TargetMode == "External" {
			continue
		}
		if rel.Type == namespaces.RelStyles {
			return resolveTargetURI(documentURI, rel.Target), nil
		}
	}

	for _, part := range session.ListParts() {
		uri := opc.NormalizeURI(part.URI)
		if isStylesPart(uri, part.ContentType) {
			return uri, nil
		}
	}

	return "", nil
}

// ParseStyles reads and parses the styles part at the given URI, returning the
// list of style definitions found.
func ParseStyles(session opc.PackageSession, stylesURI string) ([]model.StyleInfo, error) {
	doc, err := session.ReadXMLPart(stylesURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read styles part %s: %w", stylesURI, err)
	}
	root := doc.Root()
	if root == nil {
		return nil, fmt.Errorf("styles part %s has no root element", stylesURI)
	}
	if !namespaces.IsElement(root, namespaces.NsW, "styles") {
		return nil, fmt.Errorf("styles part %s root is %q, expected styles", stylesURI, root.Tag)
	}

	styleElems := namespaces.FindChildren(root, namespaces.NsW, "style")
	styleIDCounts := countStyleIDs(styleElems)
	styles := make([]model.StyleInfo, 0, len(styleElems))
	for _, elem := range styleElems {
		styles = append(styles, reportStyle(elem, styleIDCounts))
	}
	return styles, nil
}

// FindStyle returns the style with the given styleId, if present.
func FindStyle(styles []model.StyleInfo, styleID string) (model.StyleInfo, bool) {
	for _, style := range styles {
		if style.StyleID == styleID {
			return style, true
		}
	}
	return model.StyleInfo{}, false
}

func reportStyle(elem *etree.Element, styleIDCounts map[string]int) model.StyleInfo {
	info := model.StyleInfo{}
	if id, ok := namespaces.Attr(elem, namespaces.NsW, "styleId"); ok {
		info.StyleID = id
		info.PrimarySelector = id
		if id != "" {
			info.Selectors = []string{id}
		}
		if id != "" && (styleIDCounts == nil || styleIDCounts[id] == 1) {
			info.Handle = docxhandle.FormatStyle(id)
		}
	}
	if styleType, ok := namespaces.Attr(elem, namespaces.NsW, "type"); ok {
		info.Type = styleType
	}
	info.Default = boolAttr(elem, "default")
	info.Builtin = !boolAttr(elem, "customStyle")

	if name := namespaces.FindChild(elem, namespaces.NsW, "name"); name != nil {
		if val, ok := namespaces.Attr(name, namespaces.NsW, "val"); ok {
			info.Name = val
		}
	}
	if basedOn := namespaces.FindChild(elem, namespaces.NsW, "basedOn"); basedOn != nil {
		if val, ok := namespaces.Attr(basedOn, namespaces.NsW, "val"); ok {
			info.BasedOn = val
		}
	}
	if next := namespaces.FindChild(elem, namespaces.NsW, "next"); next != nil {
		if val, ok := namespaces.Attr(next, namespaces.NsW, "val"); ok {
			info.Next = val
		}
	}
	return info
}

func countStyleIDs(styles []*etree.Element) map[string]int {
	counts := make(map[string]int)
	for _, elem := range styles {
		if id, ok := namespaces.Attr(elem, namespaces.NsW, "styleId"); ok && id != "" {
			counts[id]++
		}
	}
	return counts
}

// boolAttr interprets an OOXML ST_OnOff attribute on a w:style element. A
// missing attribute is false; "1"/"true"/"on" are true; "0"/"false"/"off" are
// false; bare presence (empty value) is true.
func boolAttr(elem *etree.Element, localName string) bool {
	val, ok := namespaces.Attr(elem, namespaces.NsW, localName)
	if !ok {
		return false
	}
	switch val {
	case "0", "false", "off":
		return false
	default:
		return true
	}
}
