package body

import (
	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
)

// RunStyle reads the w:rStyle/@w:val of the first run in a paragraph that
// carries a run style. It returns an empty string when no run has a run style.
func RunStyle(paragraph *etree.Element) string {
	for _, run := range namespaces.FindChildren(paragraph, namespaces.NsW, "r") {
		rPr := namespaces.FindChild(run, namespaces.NsW, "rPr")
		if rPr == nil {
			continue
		}
		rStyle := namespaces.FindChild(rPr, namespaces.NsW, "rStyle")
		if rStyle == nil {
			continue
		}
		value, _ := namespaces.Attr(rStyle, namespaces.NsW, "val")
		if value != "" {
			return value
		}
	}
	return ""
}

// TableStyle reads the w:tblPr/w:tblStyle/@w:val of a table. It returns an
// empty string when the table has no table style.
func TableStyle(table *etree.Element) string {
	tblPr := namespaces.FindChild(table, namespaces.NsW, "tblPr")
	if tblPr == nil {
		return ""
	}
	tblStyle := namespaces.FindChild(tblPr, namespaces.NsW, "tblStyle")
	if tblStyle == nil {
		return ""
	}
	value, _ := namespaces.Attr(tblStyle, namespaces.NsW, "val")
	return value
}
