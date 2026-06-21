package inspect

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

// ListDefinedNames returns workbook defined names in workbook order.
func ListDefinedNames(session opc.PackageSession) ([]model.DefinedName, error) {
	workbook, err := ParseWorkbook(session)
	if err != nil {
		return nil, err
	}
	doc, err := session.ReadXMLPart(workbook.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read workbook part %s: %w", workbook.PartURI, err)
	}
	root := doc.Root()
	if root == nil || !isWorkbookRoot(root) {
		return nil, fmt.Errorf("workbook part %s root element not found", workbook.PartURI)
	}
	return parseDefinedNames(root, workbook.Sheets), nil
}

func parseDefinedNames(root *etree.Element, sheets []model.SheetRef) []model.DefinedName {
	definedNames := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "definedNames")
	if definedNames == nil {
		return nil
	}

	out := []model.DefinedName{}
	for idx, elem := range namespaces.FindChildren(definedNames, namespaces.NsSpreadsheetML, "definedName") {
		name := model.DefinedName{
			Number:      idx + 1,
			Name:        elem.SelectAttrValue("name", ""),
			Scope:       "workbook",
			Ref:         strings.TrimSpace(elem.Text()),
			Hidden:      boolAttr(elem.SelectAttrValue("hidden", "")),
			Comment:     elem.SelectAttrValue("comment", ""),
			Description: elem.SelectAttrValue("description", ""),
		}
		if localSheetIDText := elem.SelectAttrValue("localSheetId", ""); localSheetIDText != "" {
			if localSheetID, err := strconv.Atoi(localSheetIDText); err == nil {
				name.Scope = "sheet"
				name.LocalSheetID = &localSheetID
				name.SheetNumber = localSheetID + 1
				if localSheetID >= 0 && localSheetID < len(sheets) {
					name.SheetName = sheets[localSheetID].Name
				}
			}
		}
		out = append(out, model.WithDefinedNameSelectors(name))
	}
	return out
}

func boolAttr(value string) bool {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "1", "true":
		return true
	default:
		return false
	}
}
