package mutate

import (
	"fmt"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

const (
	contentTypeCoreProps = "application/vnd.openxmlformats-package.core-properties+xml"
	contentTypeAppProps  = "application/vnd.openxmlformats-officedocument.extended-properties+xml"
)

// WorkbookMetadataUpdate describes the metadata fields to change. A nil pointer
// means "leave unchanged"; a non-nil pointer applies the value (empty clears).
type WorkbookMetadataUpdate struct {
	Title          *string
	Subject        *string
	Creator        *string
	Keywords       *string
	Description    *string
	LastModifiedBy *string
	Category       *string
	Company        *string
	Manager        *string
	CalcMode       *string
	FullCalcOnLoad *bool
}

// UpdateWorkbookMetadataRequest bundles inputs for a metadata mutation.
type UpdateWorkbookMetadataRequest struct {
	Package      opc.PackageSession
	WorkbookURI  string
	CorePropsURI string
	AppPropsURI  string
	Updates      WorkbookMetadataUpdate
	// ExpectValues maps a field name (title, subject, creator, keywords,
	// description, lastModifiedBy, category, company, manager) to the value the
	// current document must hold before the mutation is allowed to proceed.
	ExpectValues map[string]string
}

// WorkbookMetadataResult reports the outcome of a metadata mutation.
type WorkbookMetadataResult struct {
	UpdatedCount   int               `json:"updatedCount"`
	UpdatedFields  []string          `json:"updatedFields"`
	PreviousValues map[string]string `json:"previousValues"`
}

// UpdateWorkbookMetadata applies the requested metadata and calc changes.
func UpdateWorkbookMetadata(req *UpdateWorkbookMetadataRequest) (*WorkbookMetadataResult, error) {
	if req == nil {
		return nil, fmt.Errorf("workbook metadata request is nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	coreURI := req.CorePropsURI
	if coreURI == "" {
		coreURI = xlsxinspect.CorePropsURI(req.Package)
	}
	appURI := req.AppPropsURI
	if appURI == "" {
		appURI = xlsxinspect.AppPropsURI(req.Package)
	}

	result := &WorkbookMetadataResult{
		UpdatedFields:  []string{},
		PreviousValues: map[string]string{},
	}

	// Read the current flattened metadata once for guard validation and
	// previous-value reporting.
	current, err := xlsxinspect.ReadWorkbookMetadata(req.Package)
	if err != nil {
		return nil, fmt.Errorf("failed to read current metadata: %w", err)
	}
	if err := checkMetadataGuards(req.ExpectValues, current); err != nil {
		return nil, err
	}

	u := req.Updates

	// --- core.xml properties ---
	type coreSpec struct {
		name  string
		ns    string
		local string
		value *string
		prev  string
	}
	coreSpecs := []coreSpec{
		{"title", xlsxinspect.NsDublinCore, "title", u.Title, current.Title},
		{"subject", xlsxinspect.NsDublinCore, "subject", u.Subject, current.Subject},
		{"creator", xlsxinspect.NsDublinCore, "creator", u.Creator, current.Creator},
		{"description", xlsxinspect.NsDublinCore, "description", u.Description, current.Description},
		{"keywords", xlsxinspect.NsCoreProperties, "keywords", u.Keywords, current.Keywords},
		{"lastModifiedBy", xlsxinspect.NsCoreProperties, "lastModifiedBy", u.LastModifiedBy, current.LastModifiedBy},
		{"category", xlsxinspect.NsCoreProperties, "category", u.Category, current.Category},
	}
	coreNeeded := false
	for _, s := range coreSpecs {
		if s.value != nil {
			coreNeeded = true
			break
		}
	}
	if coreNeeded {
		doc, root, created, err := readOrCreateCoreProps(req.Package, coreURI)
		if err != nil {
			return nil, err
		}
		for _, s := range coreSpecs {
			if s.value == nil {
				continue
			}
			setNamespacedChildText(root, s.ns, s.local, *s.value)
			result.UpdatedFields = append(result.UpdatedFields, s.name)
			result.PreviousValues[s.name] = s.prev
		}
		if err := writeMetadataPart(req.Package, coreURI, doc, created, contentTypeCoreProps, xlsxinspect.RelCoreProperties); err != nil {
			return nil, err
		}
	}

	// --- app.xml properties ---
	appSpecs := []coreSpec{
		{"company", xlsxinspect.NsExtendedProperties, "Company", u.Company, current.Company},
		{"manager", xlsxinspect.NsExtendedProperties, "Manager", u.Manager, current.Manager},
	}
	appNeeded := false
	for _, s := range appSpecs {
		if s.value != nil {
			appNeeded = true
			break
		}
	}
	if appNeeded {
		doc, root, created, err := readOrCreateAppProps(req.Package, appURI)
		if err != nil {
			return nil, err
		}
		for _, s := range appSpecs {
			if s.value == nil {
				continue
			}
			setNamespacedChildText(root, s.ns, s.local, *s.value)
			result.UpdatedFields = append(result.UpdatedFields, s.name)
			result.PreviousValues[s.name] = s.prev
		}
		if err := writeMetadataPart(req.Package, appURI, doc, created, contentTypeAppProps, xlsxinspect.RelExtendedProperties); err != nil {
			return nil, err
		}
	}

	// --- workbook.xml calcPr ---
	if u.CalcMode != nil || u.FullCalcOnLoad != nil {
		workbookURI := req.WorkbookURI
		if workbookURI == "" {
			workbookURI, err = xlsxinspect.FindWorkbookPart(req.Package)
			if err != nil {
				return nil, fmt.Errorf("failed to find workbook part: %w", err)
			}
		}
		doc, err := req.Package.ReadXMLPart(workbookURI)
		if err != nil {
			return nil, fmt.Errorf("failed to read workbook %s: %w", workbookURI, err)
		}
		root := doc.Root()
		if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "workbook") {
			return nil, fmt.Errorf("workbook part %s root element not found", workbookURI)
		}
		calcPr := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "calcPr")
		if calcPr == nil {
			calcPr = newElement(root.Space, "calcPr")
			insertWorkbookChild(root, calcPr, "calcPr")
		}
		if u.CalcMode != nil {
			if !validCalcMode(*u.CalcMode) {
				return nil, fmt.Errorf("invalid calcMode %q (must be auto, manual, or autoNoTable)", *u.CalcMode)
			}
			calcPr.CreateAttr("calcMode", *u.CalcMode)
			result.UpdatedFields = append(result.UpdatedFields, "calcMode")
			result.PreviousValues["calcMode"] = current.CalcMode
		}
		if u.FullCalcOnLoad != nil {
			if *u.FullCalcOnLoad {
				calcPr.CreateAttr("fullCalcOnLoad", "1")
				calcPr.CreateAttr("forceFullCalc", "1")
			} else {
				calcPr.RemoveAttr("fullCalcOnLoad")
				calcPr.RemoveAttr("forceFullCalc")
			}
			result.UpdatedFields = append(result.UpdatedFields, "fullCalcOnLoad")
			result.PreviousValues["fullCalcOnLoad"] = boolStr(current.FullCalcOnLoad)
		}
		if err := req.Package.ReplaceXMLPart(workbookURI, doc); err != nil {
			return nil, fmt.Errorf("failed to replace workbook %s: %w", workbookURI, err)
		}
	}

	result.UpdatedCount = len(result.UpdatedFields)
	return result, nil
}

func checkMetadataGuards(expect map[string]string, current *xlsxinspect.WorkbookMetadata) error {
	if len(expect) == 0 {
		return nil
	}
	values := map[string]string{
		"title":          current.Title,
		"subject":        current.Subject,
		"creator":        current.Creator,
		"keywords":       current.Keywords,
		"description":    current.Description,
		"lastModifiedBy": current.LastModifiedBy,
		"category":       current.Category,
		"company":        current.Company,
		"manager":        current.Manager,
	}
	for field, want := range expect {
		got, ok := values[field]
		if !ok {
			return fmt.Errorf("unknown guard field %q", field)
		}
		if got != want {
			return fmt.Errorf("expected %s to be %q but found %q", field, want, got)
		}
	}
	return nil
}

// readOrCreateCoreProps returns the core.xml document, creating a fresh one with
// the required namespace declarations when the part is missing.
func readOrCreateCoreProps(session opc.PackageSession, uri string) (*etree.Document, *etree.Element, bool, error) {
	if doc, err := session.ReadXMLPart(uri); err == nil {
		if root := doc.Root(); root != nil {
			ensureCorePropsNamespaces(root)
			return doc, root, false, nil
		}
	}
	doc := etree.NewDocument()
	doc.CreateProcInst("xml", `version="1.0" encoding="UTF-8" standalone="yes"`)
	root := doc.CreateElement("cp:coreProperties")
	root.Space = "cp"
	ensureCorePropsNamespaces(root)
	return doc, root, true, nil
}

func ensureCorePropsNamespaces(root *etree.Element) {
	if root.SelectAttr("xmlns:cp") == nil {
		root.CreateAttr("xmlns:cp", xlsxinspect.NsCoreProperties)
	}
	if root.SelectAttr("xmlns:dc") == nil {
		root.CreateAttr("xmlns:dc", xlsxinspect.NsDublinCore)
	}
	if root.SelectAttr("xmlns:dcterms") == nil {
		root.CreateAttr("xmlns:dcterms", xlsxinspect.NsDublinCoreTerms)
	}
	if root.SelectAttr("xmlns:dcmitype") == nil {
		root.CreateAttr("xmlns:dcmitype", "http://purl.org/dc/dcmitype/")
	}
	if root.SelectAttr("xmlns:xsi") == nil {
		root.CreateAttr("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance")
	}
}

// readOrCreateAppProps returns the app.xml document, creating a fresh one with
// the extended-properties default namespace when missing.
func readOrCreateAppProps(session opc.PackageSession, uri string) (*etree.Document, *etree.Element, bool, error) {
	if doc, err := session.ReadXMLPart(uri); err == nil {
		if root := doc.Root(); root != nil {
			if root.SelectAttr("xmlns") == nil {
				root.CreateAttr("xmlns", xlsxinspect.NsExtendedProperties)
			}
			return doc, root, false, nil
		}
	}
	doc := etree.NewDocument()
	doc.CreateProcInst("xml", `version="1.0" encoding="UTF-8" standalone="yes"`)
	root := doc.CreateElement("Properties")
	root.CreateAttr("xmlns", xlsxinspect.NsExtendedProperties)
	return doc, root, true, nil
}

// validCalcMode reports whether v is a valid CT_CalcPr calcMode enumeration.
func validCalcMode(v string) bool {
	switch v {
	case "auto", "manual", "autoNoTable":
		return true
	default:
		return false
	}
}

// setNamespacedChildText sets (or creates) the text of a namespaced child. The
// child is matched by namespace URI regardless of prefix. New children adopt the
// canonical prefix for the namespace and, for app.xml (an xsd:sequence), are
// inserted at their schema-mandated position.
func setNamespacedChildText(root *etree.Element, ns, local, value string) {
	child := namespaces.FindChild(root, ns, local)
	// An empty value clears the field, per the WorkbookMetadataUpdate contract:
	// remove the element rather than leaving a meaningless empty node.
	if value == "" {
		if child != nil {
			root.RemoveChild(child)
		}
		return
	}
	if child == nil {
		child = etree.NewElement(local)
		child.Space = prefixForMetadataNS(ns)
		if ns == xlsxinspect.NsExtendedProperties {
			insertAppPropertyChild(root, child, local)
		} else {
			// core.xml is an xsd:all; order is irrelevant.
			root.AddChild(child)
		}
	}
	child.SetText(value)
}

// insertAppPropertyChild inserts child into the extended-properties root,
// preserving the CT_Properties (xsd:sequence) element order.
func insertAppPropertyChild(root, child *etree.Element, localName string) {
	targetOrder := appPropertyOrder(localName)
	for _, existing := range root.ChildElements() {
		if appPropertyOrder(existing.Tag) > targetOrder {
			root.InsertChildAt(existing.Index(), child)
			return
		}
	}
	root.AddChild(child)
}

// appPropertyOrder returns the CT_Properties sequence position of an element.
// Unknown elements sort last (so we never insert before a known later element).
func appPropertyOrder(localName string) int {
	switch localName {
	case "Template":
		return 10
	case "Manager":
		return 20
	case "Company":
		return 30
	case "Pages":
		return 40
	case "Words":
		return 50
	case "Characters":
		return 60
	case "PresentationFormat":
		return 70
	case "Lines":
		return 80
	case "Paragraphs":
		return 90
	case "Slides":
		return 100
	case "Notes":
		return 110
	case "TotalTime":
		return 120
	case "HiddenSlides":
		return 130
	case "MMClips":
		return 140
	case "ScaleCrop":
		return 150
	case "HeadingPairs":
		return 160
	case "TitlesOfParts":
		return 170
	case "LinksUpToDate":
		return 180
	case "CharactersWithSpaces":
		return 190
	case "SharedDoc":
		return 200
	case "HyperlinkBase":
		return 210
	case "HLinks":
		return 220
	case "HyperlinksChanged":
		return 230
	case "DigSig":
		return 240
	case "Application":
		return 250
	case "AppVersion":
		return 260
	case "DocSecurity":
		return 270
	default:
		return 10000
	}
}

func prefixForMetadataNS(ns string) string {
	switch ns {
	case xlsxinspect.NsDublinCore:
		return "dc"
	case xlsxinspect.NsDublinCoreTerms:
		return "dcterms"
	case xlsxinspect.NsCoreProperties:
		return "cp"
	default:
		return ""
	}
}

// writeMetadataPart writes the document back, registering a new part with its
// content type and package relationship when it did not previously exist.
func writeMetadataPart(session opc.PackageSession, uri string, doc *etree.Document, created bool, contentType, relType string) error {
	if created {
		data, err := doc.WriteToBytes()
		if err != nil {
			return fmt.Errorf("failed to serialize %s: %w", uri, err)
		}
		if err := session.AddPart(uri, data, contentType, nil); err != nil {
			return fmt.Errorf("failed to add part %s: %w", uri, err)
		}
		if err := ensurePackageRelationship(session, relType, uri); err != nil {
			return err
		}
		return nil
	}
	if err := session.ReplaceXMLPart(uri, doc); err != nil {
		return fmt.Errorf("failed to replace part %s: %w", uri, err)
	}
	return nil
}

// ensurePackageRelationship adds a package-root relationship of relType pointing
// at the given part URI if one does not already exist.
func ensurePackageRelationship(session opc.PackageSession, relType, targetURI string) error {
	rels := session.ListRelationships("/")
	for _, rel := range rels {
		if rel.Type == relType {
			return nil
		}
	}
	target := opc.RelationshipTarget("/", targetURI)
	rels = append(rels, opc.RelationshipInfo{
		SourceURI: "/",
		ID:        opc.AllocateRelationshipID(rels),
		Type:      relType,
		Target:    target,
	})
	if err := opc.WriteRelationships(session, "/", rels); err != nil {
		return fmt.Errorf("failed to write package relationships: %w", err)
	}
	return nil
}

func boolStr(b bool) string {
	if b {
		return "true"
	}
	return "false"
}
