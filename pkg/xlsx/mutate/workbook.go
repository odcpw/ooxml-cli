package mutate

import (
	"fmt"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

// EnsureFullCalcOnLoad marks the workbook for recalculation when opened.
func EnsureFullCalcOnLoad(session opc.PackageSession, workbookURI string) error {
	if session == nil {
		return fmt.Errorf("package session is nil")
	}
	if workbookURI == "" {
		return fmt.Errorf("workbook URI cannot be empty")
	}

	doc, err := session.ReadXMLPart(workbookURI)
	if err != nil {
		return fmt.Errorf("failed to read workbook %s: %w", workbookURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "workbook") {
		return fmt.Errorf("workbook part %s root element not found", workbookURI)
	}

	calcPr := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "calcPr")
	if calcPr == nil {
		calcPr = etree.NewElement("calcPr")
		calcPr.Space = root.Space
		root.AddChild(calcPr)
	}
	calcPr.CreateAttr("fullCalcOnLoad", "1")
	calcPr.CreateAttr("forceFullCalc", "1")

	if err := session.ReplaceXMLPart(workbookURI, doc); err != nil {
		return fmt.Errorf("failed to replace workbook %s: %w", workbookURI, err)
	}
	return nil
}
