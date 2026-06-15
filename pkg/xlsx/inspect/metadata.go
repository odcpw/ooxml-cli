package inspect

import (
	"strconv"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

// Namespace URIs and relationship/content types for workbook metadata parts.
const (
	// NsCoreProperties is the OPC core-properties namespace (root cp:coreProperties).
	NsCoreProperties = "http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
	// NsDublinCore is the Dublin Core elements namespace (dc:).
	NsDublinCore = "http://purl.org/dc/elements/1.1/"
	// NsDublinCoreTerms is the Dublin Core terms namespace (dcterms:).
	NsDublinCoreTerms = "http://purl.org/dc/terms/"
	// NsExtendedProperties is the extended (app) properties namespace.
	NsExtendedProperties = "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"

	// RelCoreProperties is the relationship type for docProps/core.xml.
	RelCoreProperties = "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties"
	// RelExtendedProperties is the relationship type for docProps/app.xml.
	RelExtendedProperties = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties"

	// DefaultCorePropsURI is the conventional package path for core properties.
	DefaultCorePropsURI = "/docProps/core.xml"
	// DefaultAppPropsURI is the conventional package path for extended properties.
	DefaultAppPropsURI = "/docProps/app.xml"
)

// WorkbookMetadata holds flattened workbook-level metadata and calc settings.
type WorkbookMetadata struct {
	Title          string `json:"title"`
	Subject        string `json:"subject"`
	Creator        string `json:"creator"`
	Keywords       string `json:"keywords"`
	Description    string `json:"description"`
	LastModifiedBy string `json:"lastModifiedBy"`
	Category       string `json:"category"`
	Company        string `json:"company"`
	Manager        string `json:"manager"`

	CalcMode       string  `json:"calcMode"`
	FullCalcOnLoad bool    `json:"fullCalcOnLoad"`
	ForceFullCalc  bool    `json:"forceFullCalc"`
	CalcID         string  `json:"calcId"`
	Iterate        bool    `json:"iterate"`
	IterateCount   int     `json:"iterateCount"`
	IterateDelta   float64 `json:"iterateDelta"`
}

// CorePropsURI returns the package URI of the core properties part, resolving
// from package root relationships and falling back to the conventional path.
func CorePropsURI(session opc.PackageSession) string {
	return propsURIByRelType(session, RelCoreProperties, DefaultCorePropsURI)
}

// AppPropsURI returns the package URI of the extended properties part.
func AppPropsURI(session opc.PackageSession) string {
	return propsURIByRelType(session, RelExtendedProperties, DefaultAppPropsURI)
}

func propsURIByRelType(session opc.PackageSession, relType, fallback string) string {
	if session == nil {
		return fallback
	}
	for _, rel := range session.ListRelationships("/") {
		if rel.Type == relType && rel.TargetMode != "External" {
			return resolveTargetURI("/", rel.Target)
		}
	}
	return fallback
}

// ReadWorkbookMetadata reads core, app, and calc settings into a flattened struct.
// Missing parts are treated as empty (not an error).
func ReadWorkbookMetadata(session opc.PackageSession) (*WorkbookMetadata, error) {
	out := &WorkbookMetadata{
		CalcMode:     "auto",
		IterateCount: 100,
		IterateDelta: 0.001,
	}

	if doc, err := session.ReadXMLPart(CorePropsURI(session)); err == nil {
		if root := doc.Root(); root != nil {
			out.Title = childText(root, NsDublinCore, "title")
			out.Subject = childText(root, NsDublinCore, "subject")
			out.Creator = childText(root, NsDublinCore, "creator")
			out.Description = childText(root, NsDublinCore, "description")
			out.Keywords = childText(root, NsCoreProperties, "keywords")
			out.LastModifiedBy = childText(root, NsCoreProperties, "lastModifiedBy")
			out.Category = childText(root, NsCoreProperties, "category")
		}
	}

	if doc, err := session.ReadXMLPart(AppPropsURI(session)); err == nil {
		if root := doc.Root(); root != nil {
			out.Company = childText(root, NsExtendedProperties, "Company")
			out.Manager = childText(root, NsExtendedProperties, "Manager")
		}
	}

	if workbookURI, err := FindWorkbookPart(session); err == nil {
		if doc, err := session.ReadXMLPart(workbookURI); err == nil {
			if root := doc.Root(); root != nil {
				if calcPr := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "calcPr"); calcPr != nil {
					if v := calcPr.SelectAttrValue("calcMode", ""); v != "" {
						out.CalcMode = v
					}
					out.FullCalcOnLoad = calcPr.SelectAttrValue("fullCalcOnLoad", "") == "1"
					out.ForceFullCalc = calcPr.SelectAttrValue("forceFullCalc", "") == "1"
					out.CalcID = calcPr.SelectAttrValue("calcId", "")
					out.Iterate = calcPr.SelectAttrValue("iterate", "") == "1"
					if v := calcPr.SelectAttrValue("iterateCount", ""); v != "" {
						if n, err := strconv.Atoi(v); err == nil {
							out.IterateCount = n
						}
					}
					if v := calcPr.SelectAttrValue("iterateDelta", ""); v != "" {
						if f, err := strconv.ParseFloat(v, 64); err == nil {
							out.IterateDelta = f
						}
					}
				}
			}
		}
	}

	return out, nil
}

func childText(root *etree.Element, ns, localName string) string {
	if child := namespaces.FindChild(root, ns, localName); child != nil {
		return child.Text()
	}
	return ""
}
