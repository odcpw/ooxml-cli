package cli

import (
	"strconv"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

func countPPTXSlideShapeTypes(spTree *etree.Element) (textShapes, images, tables int) {
	if spTree == nil {
		return 0, 0, 0
	}
	for _, sp := range spTree.FindElements("sp") {
		if sp.FindElement("txBody") != nil {
			textShapes++
		}
	}
	for _, shape := range inspect.EnumerateShapes(spTree) {
		switch shape.Type {
		case model.ShapeTypePic:
			images++
		case model.ShapeTypeGraphicFrame:
			if shape.TableInfo != nil {
				tables++
			}
		}
	}
	return textShapes, images, tables
}

func attachPPTXSlideText(spTree *etree.Element, shapes []model.ShapeInfo) {
	if spTree == nil {
		return
	}
	byID := make(map[int]*model.ShapeInfo, len(shapes))
	for i := range shapes {
		byID[shapes[i].ID] = &shapes[i]
	}
	for _, spElem := range spTree.FindElements("sp") {
		id, ok := pptxShapeElementID(spElem, "nvSpPr")
		if !ok {
			continue
		}
		shape := byID[id]
		if shape == nil {
			continue
		}
		txBody := spElem.FindElement("txBody")
		if txBody == nil {
			continue
		}
		textInfo := inspect.ExtractTextBody(txBody)
		if textInfo != nil {
			shape.TextContent = textInfo.PlainText
		}
	}
}

func pptxShapeElementID(elem *etree.Element, nonVisualTag string) (int, bool) {
	nv := elem.FindElement(nonVisualTag)
	if nv == nil {
		return 0, false
	}
	cNvPr := nv.FindElement("cNvPr")
	if cNvPr == nil {
		return 0, false
	}
	idStr := cNvPr.SelectAttrValue("id", "")
	if idStr == "" {
		return 0, false
	}
	id, err := strconv.Atoi(idStr)
	if err != nil {
		return 0, false
	}
	return id, true
}
