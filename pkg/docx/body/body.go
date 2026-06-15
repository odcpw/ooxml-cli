// Package body enumerates and reads the main body of DOCX documents.
package body

import (
	"fmt"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
)

type BodyBlock struct {
	Index   int
	Kind    model.BlockKind
	Element *etree.Element
}

func FindBody(root *etree.Element) (*etree.Element, error) {
	if root == nil || !namespaces.IsElement(root, namespaces.NsW, "document") {
		return nil, fmt.Errorf("document root element not found")
	}
	body := namespaces.FindChild(root, namespaces.NsW, "body")
	if body == nil {
		return nil, fmt.Errorf("document body element not found")
	}
	return body, nil
}

func Blocks(body *etree.Element) []BodyBlock {
	if body == nil {
		return nil
	}
	var blocks []BodyBlock
	for _, child := range body.ChildElements() {
		switch LocalName(child.Tag) {
		case "p":
			blocks = append(blocks, BodyBlock{
				Index:   len(blocks) + 1,
				Kind:    model.BlockKindParagraph,
				Element: child,
			})
		case "tbl":
			blocks = append(blocks, BodyBlock{
				Index:   len(blocks) + 1,
				Kind:    model.BlockKindTable,
				Element: child,
			})
		}
	}
	return blocks
}

func ParagraphText(elem *etree.Element) string {
	var builder strings.Builder
	collectText(elem, &builder)
	return builder.String()
}

func ParagraphStyle(paragraph *etree.Element) string {
	pPr := namespaces.FindChild(paragraph, namespaces.NsW, "pPr")
	if pPr == nil {
		return ""
	}
	pStyle := namespaces.FindChild(pPr, namespaces.NsW, "pStyle")
	if pStyle == nil {
		return ""
	}
	value, _ := namespaces.Attr(pStyle, namespaces.NsW, "val")
	return value
}

func collectText(elem *etree.Element, builder *strings.Builder) {
	for _, child := range elem.ChildElements() {
		switch LocalName(child.Tag) {
		case "t":
			builder.WriteString(child.Text())
		case "tab":
			builder.WriteString("\t")
		case "br", "cr":
			builder.WriteString("\n")
		case "noBreakHyphen":
			builder.WriteString("-")
		case "delText", "instrText":
			continue
		default:
			collectText(child, builder)
		}
	}
}

func LocalName(tag string) string {
	if idx := strings.LastIndex(tag, "}"); idx >= 0 && idx+1 < len(tag) {
		return tag[idx+1:]
	}
	if idx := strings.LastIndex(tag, ":"); idx >= 0 && idx+1 < len(tag) {
		return tag[idx+1:]
	}
	return tag
}
