package mutate

import (
	"fmt"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

// Legacy Excel cell notes are only VISIBLE when the worksheet also carries a VML
// drawing (xl/drawings/vmlDrawingN.vml) referenced by a <legacyDrawing r:id>
// child. The comment text itself lives in xl/commentsN.xml; the VML only
// provides the (empty) note box and its cell anchor. We rebuild the VML
// wholesale from the current commentList on every add/remove so it always has
// exactly one v:shape per comment, in commentList order.

// syncCommentsVml writes (or rewrites) the VML drawing part for a worksheet so
// it has one note shape per comment in commentList, and ensures the worksheet
// <legacyDrawing r:id> element + relationship + content-type exist. It returns
// the VML part URI. When the comment list is empty it removes the VML part,
// relationship, and worksheet element instead.
func syncCommentsVml(session opc.PackageSession, sheet model.SheetRef, commentList *etree.Element) (string, error) {
	cells := commentCellRefs(commentList)

	vmlURI, vmlExists := findVmlDrawingPart(session, sheet.PartURI)

	if len(cells) == 0 {
		if vmlExists {
			if err := session.RemovePart(vmlURI); err != nil {
				return "", fmt.Errorf("failed to remove vml drawing part %s: %w", vmlURI, err)
			}
		}
		if err := removeVmlDrawingRel(session, sheet.PartURI); err != nil {
			return "", err
		}
		if err := removeWorksheetLegacyDrawing(session, sheet); err != nil {
			return "", err
		}
		return "", nil
	}

	if vmlURI == "" {
		vmlURI = allocateNumberedPart(session, "/xl/drawings/vmlDrawing", ".vml")
	}

	// VML is not an +xml part, so write it as raw bytes (ReplaceXMLPart would
	// coerce its content type to application/xml). AddPart re-adds in place when
	// the part already exists, which handles the extend/shrink case.
	if err := session.AddPart(vmlURI, buildCommentsVml(cells), namespaces.ContentTypeVml, nil); err != nil {
		return "", fmt.Errorf("failed to add vml drawing part %s: %w", vmlURI, err)
	}

	rid, err := ensureVmlDrawingRel(session, sheet.PartURI, vmlURI)
	if err != nil {
		return "", err
	}
	if err := addWorksheetLegacyDrawingRef(session, sheet, rid); err != nil {
		return "", err
	}
	return vmlURI, nil
}

// commentCellRefs returns the normalized cell anchors of every comment, in
// commentList order.
func commentCellRefs(commentList *etree.Element) []address.CellRef {
	if commentList == nil {
		return nil
	}
	var refs []address.CellRef
	for _, c := range namespaces.FindChildren(commentList, namespaces.NsSpreadsheetML, "comment") {
		ref, err := address.ParseCell(c.SelectAttrValue("ref", ""))
		if err != nil {
			continue
		}
		refs = append(refs, ref)
	}
	return refs
}

// buildCommentsVml renders the legacy VML drawing for a set of note anchors.
// Each note carries an empty text box anchored to its cell via 0-based
// Row/Column in the ClientData block.
func buildCommentsVml(cells []address.CellRef) []byte {
	var b strings.Builder
	b.WriteString(`<xml xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:x="urn:schemas-microsoft-com:office:excel">`)
	b.WriteString(`<o:shapelayout v:ext="edit"><o:idmap v:ext="edit" data="1"/></o:shapelayout>`)
	b.WriteString(`<v:shapetype id="_x0000_t202" coordsize="21600,21600" o:spt="202" path="m,l,21600r21600,l21600,xe">`)
	b.WriteString(`<v:stroke joinstyle="miter"/><v:path gradientshapeok="t" o:connecttype="rect"/></v:shapetype>`)
	for i, cell := range cells {
		shapeID := fmt.Sprintf("_x0000_s%d", 1025+i)
		row0 := cell.Row - 1
		col0 := cell.Column - 1
		b.WriteString(fmt.Sprintf(`<v:shape id="%s" type="#_x0000_t202" style="position:absolute;visibility:hidden" fillcolor="#ffffe1" o:insetmode="auto">`, shapeID))
		b.WriteString(`<v:fill color2="#ffffe1"/><v:shadow color="black" obscured="t"/>`)
		b.WriteString(`<v:path o:connecttype="none"/><v:textbox style="mso-direction-alt:auto"><div style="text-align:left"/></v:textbox>`)
		b.WriteString(`<x:ClientData ObjectType="Note">`)
		b.WriteString(`<x:MoveWithCells/><x:SizeWithCells/>`)
		// Anchor: from-col, from-col-offset, from-row, from-row-offset,
		// to-col, to-col-offset, to-row, to-row-offset (all 0-based cells).
		b.WriteString(fmt.Sprintf(`<x:Anchor>%d, 15, %d, 2, %d, 31, %d, 4</x:Anchor>`, col0+1, row0, col0+3, row0+4))
		b.WriteString(`<x:AutoFill>False</x:AutoFill>`)
		b.WriteString(fmt.Sprintf(`<x:Row>%d</x:Row>`, row0))
		b.WriteString(fmt.Sprintf(`<x:Column>%d</x:Column>`, col0))
		b.WriteString(`</x:ClientData></v:shape>`)
	}
	b.WriteString(`</xml>`)
	return []byte(b.String())
}

// findVmlDrawingPart resolves the worksheet's VML drawing part via its
// vmlDrawing relationship. The boolean reports whether the part exists.
func findVmlDrawingPart(session opc.PackageSession, worksheetURI string) (string, bool) {
	uri := ""
	for _, rel := range session.ListRelationships(worksheetURI) {
		if rel.TargetMode == "External" {
			continue
		}
		if rel.Type == namespaces.RelVmlDrawing {
			uri = opc.ResolveRelationshipTarget(worksheetURI, rel.Target)
			break
		}
	}
	if uri == "" {
		return "", false
	}
	for _, part := range session.ListParts() {
		if opc.NormalizeURI(part.URI) == uri {
			return uri, true
		}
	}
	return uri, false
}

// ensureVmlDrawingRel returns the existing worksheet->vmlDrawing relationship id,
// creating the relationship when absent.
func ensureVmlDrawingRel(session opc.PackageSession, worksheetURI, vmlURI string) (string, error) {
	rels := session.ListRelationships(worksheetURI)
	for _, rel := range rels {
		if rel.Type == namespaces.RelVmlDrawing && rel.TargetMode != "External" {
			return rel.ID, nil
		}
	}
	relID := opc.AllocateRelationshipID(rels)
	rels = append(rels, opc.RelationshipInfo{
		SourceURI: worksheetURI,
		ID:        relID,
		Type:      namespaces.RelVmlDrawing,
		Target:    opc.RelationshipTarget(worksheetURI, vmlURI),
	})
	if err := opc.WriteRelationships(session, worksheetURI, rels); err != nil {
		return "", fmt.Errorf("failed to write vml drawing relationship: %w", err)
	}
	return relID, nil
}

// removeVmlDrawingRel drops the worksheet's vmlDrawing relationship if present.
func removeVmlDrawingRel(session opc.PackageSession, worksheetURI string) error {
	rels := session.ListRelationships(worksheetURI)
	var kept []opc.RelationshipInfo
	for _, rel := range rels {
		if rel.Type == namespaces.RelVmlDrawing {
			continue
		}
		kept = append(kept, rel)
	}
	if len(kept) == len(rels) {
		return nil
	}
	if err := opc.WriteRelationships(session, worksheetURI, kept); err != nil {
		return fmt.Errorf("failed to write worksheet relationships: %w", err)
	}
	return nil
}

// addWorksheetLegacyDrawingRef inserts a <legacyDrawing r:id> child in correct
// worksheet child order, leaving an existing one with the same id untouched.
func addWorksheetLegacyDrawingRef(session opc.PackageSession, sheet model.SheetRef, rid string) error {
	doc, root, err := readWorksheetRoot(session, sheet)
	if err != nil {
		return err
	}
	if existing := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "legacyDrawing"); existing != nil {
		existingRID, _ := namespaces.Attr(existing, namespaces.NsR, "id")
		if existingRID == rid {
			return nil
		}
		existing.CreateAttr("r:id", rid)
		ensureRelationshipsNamespace(root)
		if err := session.ReplaceXMLPart(sheet.PartURI, doc); err != nil {
			return fmt.Errorf("failed to replace worksheet %s: %w", sheet.PartURI, err)
		}
		return nil
	}
	legacy := newElement(root.Space, "legacyDrawing")
	legacy.CreateAttr("r:id", rid)
	ensureRelationshipsNamespace(root)
	insertWorksheetChild(root, legacy, "legacyDrawing")
	if err := session.ReplaceXMLPart(sheet.PartURI, doc); err != nil {
		return fmt.Errorf("failed to replace worksheet %s: %w", sheet.PartURI, err)
	}
	return nil
}

// removeWorksheetLegacyDrawing strips the <legacyDrawing> element from the
// worksheet, if present.
func removeWorksheetLegacyDrawing(session opc.PackageSession, sheet model.SheetRef) error {
	doc, root, err := readWorksheetRoot(session, sheet)
	if err != nil {
		return err
	}
	existing := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "legacyDrawing")
	if existing == nil {
		return nil
	}
	root.RemoveChild(existing)
	if err := session.ReplaceXMLPart(sheet.PartURI, doc); err != nil {
		return fmt.Errorf("failed to replace worksheet %s: %w", sheet.PartURI, err)
	}
	return nil
}
