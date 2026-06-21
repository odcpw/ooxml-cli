package mutate

import (
	"strconv"
	"strings"

	"github.com/beevik/etree"
)

// appendSpTreeChild appends a shape-tree child before the schema-tail extLst,
// when present. PresentationML requires extLst to remain the final spTree child.
func appendSpTreeChild(spTree, child *etree.Element) {
	if spTree == nil || child == nil {
		return
	}
	if idx := firstDirectChildTokenIndexByLocalName(spTree, "extLst"); idx >= 0 {
		spTree.InsertChildAt(idx, child)
		return
	}
	spTree.AddChild(child)
}

// insertSpTreeChildAfterShapeID inserts child after afterID when that position
// is schema-safe; otherwise it falls back to appending before extLst.
func insertSpTreeChildAfterShapeID(spTree, child *etree.Element, afterID int) {
	if spTree == nil || child == nil {
		return
	}
	if afterID > 0 {
		extIdx := firstDirectChildTokenIndexByLocalName(spTree, "extLst")
		for _, elem := range spTree.ChildElements() {
			if spTreeShapeID(elem) != afterID {
				continue
			}
			insertIdx := elem.Index() + 1
			if extIdx >= 0 && insertIdx >= extIdx {
				insertIdx = extIdx
			}
			spTree.InsertChildAt(insertIdx, child)
			return
		}
	}
	appendSpTreeChild(spTree, child)
}

func firstDirectChildTokenIndexByLocalName(elem *etree.Element, localName string) int {
	if elem == nil {
		return -1
	}
	for _, child := range elem.ChildElements() {
		if pptxMutateLocalName(child.Tag) == localName {
			return child.Index()
		}
	}
	return -1
}

func spTreeShapeID(shape *etree.Element) int {
	for _, nv := range []string{"nvSpPr", "nvPicPr", "nvGrpSpPr", "nvGraphicFramePr"} {
		nvPr := directChildByLocalNameForSpTree(shape, nv)
		if nvPr == nil {
			continue
		}
		cNvPr := directChildByLocalNameForSpTree(nvPr, "cNvPr")
		if cNvPr == nil {
			continue
		}
		id, err := strconv.Atoi(strings.TrimSpace(cNvPr.SelectAttrValue("id", "")))
		if err == nil {
			return id
		}
	}
	return 0
}

func directChildByLocalNameForSpTree(elem *etree.Element, localName string) *etree.Element {
	if elem == nil {
		return nil
	}
	for _, child := range elem.ChildElements() {
		if pptxMutateLocalName(child.Tag) == localName {
			return child
		}
	}
	return nil
}

func pptxMutateLocalName(tag string) string {
	if idx := strings.LastIndex(tag, "}"); idx >= 0 && idx+1 < len(tag) {
		return tag[idx+1:]
	}
	if idx := strings.LastIndex(tag, ":"); idx >= 0 && idx+1 < len(tag) {
		return tag[idx+1:]
	}
	return tag
}
