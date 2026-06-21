package validate

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/diag"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/imagex"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmodel "github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

const (
	docxNsDrawingML = "http://schemas.openxmlformats.org/drawingml/2006/main"
	docxNsWP        = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
)

func validateDOCXSemantics(session opc.PackageSession) ([]result.Diagnostic, error) {
	var diags []result.Diagnostic

	partMap := make(map[string]bool)
	for _, part := range session.ListParts() {
		partMap[opc.NormalizeURI(part.URI)] = true
	}

	documentURI, err := docxinspect.FindMainDocumentPart(session)
	if err != nil {
		diags = append(diags, diag.Error(
			"DOCX_PARSE_ERROR",
			"failed to find main document part: "+err.Error(),
		))
		return diags, nil
	}
	if !partMap[documentURI] {
		diags = append(diags, diag.Error(
			"DOCX_MISSING_DOCUMENT",
			"main document part not found: "+documentURI,
		))
		return diags, nil
	}

	documentDoc, err := session.ReadXMLPart(documentURI)
	if err != nil {
		diags = append(diags, diag.Error(
			"DOCX_PARSE_ERROR",
			"failed to parse main document: "+err.Error(),
		))
		return diags, nil
	}
	root := documentDoc.Root()
	if root == nil || !xmlx.ElementMatches(root, namespaces.NsW, "document") {
		diags = append(diags, diag.Error(
			"DOCX_DOCUMENT_ROOT_ERROR",
			"main document root element not found",
		))
		return diags, nil
	}
	body := namespaces.FindChild(root, namespaces.NsW, "body")
	if body == nil {
		diags = append(diags, diag.Error(
			"DOCX_MISSING_BODY",
			"main document has no body element",
		))
	}

	document, err := docxinspect.ParseDocument(session)
	if err != nil {
		diags = append(diags, diag.Error(
			"DOCX_PARSE_ERROR",
			"failed to parse document structure: "+err.Error(),
		))
		return diags, nil
	}

	if document.StylesURI != "" && !partMap[document.StylesURI] {
		diags = append(diags, diag.Error(
			"DOCX_MISSING_STYLES",
			"styles part not found: "+document.StylesURI,
		))
	}
	if document.NumberingURI != "" && !partMap[document.NumberingURI] {
		diags = append(diags, diag.Error(
			"DOCX_MISSING_NUMBERING",
			"numbering part not found: "+document.NumberingURI,
		))
	}
	var styles []docxmodel.StyleInfo
	stylesParsed := false
	if document.StylesURI != "" && partMap[document.StylesURI] {
		styles, err = docxinspect.ParseStyles(session, document.StylesURI)
		if err != nil {
			diags = append(diags, diag.Error(
				"DOCX_PARSE_ERROR",
				"failed to parse styles part: "+err.Error(),
			))
		} else {
			stylesParsed = true
		}
	}
	if body != nil {
		if document.StylesURI == "" && len(namespaces.FindDescendants(body, namespaces.NsW, "pStyle")) > 0 {
			diags = append(diags, diag.Warning(
				"DOCX_MISSING_STYLES",
				"document uses paragraph styles but has no styles relationship",
			))
		}
		if document.NumberingURI == "" && len(namespaces.FindDescendants(body, namespaces.NsW, "numPr")) > 0 {
			diags = append(diags, diag.Warning(
				"DOCX_MISSING_NUMBERING",
				"document uses numbering but has no numbering relationship",
			))
		}
		diags = append(diags, validateDOCXTableScaffolds(body)...)
		if stylesParsed {
			diags = append(diags, validateDOCXStyleReferences(body, styles)...)
		}
		diags = append(diags, validateDOCXCommentReferences(session, documentURI, body, partMap)...)
	}

	diags = append(diags, validateDocumentRelationships(session, documentURI, partMap)...)
	diags = append(diags, validateImageReferences(documentDoc, session, documentURI, partMap)...)
	diags = append(diags, validateHeaderFooterImageReferences(session, documentURI, partMap)...)
	diags = append(diags, validateDOCXDrawingExtents(root)...)
	return diags, nil
}

func validateDOCXTableScaffolds(body *etree.Element) []result.Diagnostic {
	var diags []result.Diagnostic
	for _, child := range namespaces.FindDescendants(body, namespaces.NsW, "tbl") {
		firstRowIndex := -1
		var tblPr, tblGrid *etree.Element
		for _, tableChild := range child.ChildElements() {
			switch {
			case xmlx.ElementMatches(tableChild, namespaces.NsW, "tblPr") && tblPr == nil:
				tblPr = tableChild
			case xmlx.ElementMatches(tableChild, namespaces.NsW, "tblGrid") && tblGrid == nil:
				tblGrid = tableChild
			case xmlx.ElementMatches(tableChild, namespaces.NsW, "tr") && firstRowIndex < 0:
				firstRowIndex = tableChild.Index()
			}
		}
		if firstRowIndex < 0 {
			continue
		}
		if tblPr == nil || tblGrid == nil || tblPr.Index() > firstRowIndex || tblGrid.Index() > firstRowIndex || tblPr.Index() > tblGrid.Index() {
			diags = append(diags, diag.Error(
				"DOCX_TABLE_SCAFFOLD",
				"table must contain w:tblPr and w:tblGrid before its first w:tr row",
			))
		}
		if tableGridDiag, ok := validateDOCXTableGrid(child, tblGrid); ok {
			diags = append(diags, tableGridDiag)
		}
		diags = append(diags, validateDOCXTableCellBlocks(child)...)
	}
	return diags
}

func validateDOCXTableCellBlocks(table *etree.Element) []result.Diagnostic {
	var diags []result.Diagnostic
	for _, row := range namespaces.FindChildren(table, namespaces.NsW, "tr") {
		for _, cell := range namespaces.FindChildren(row, namespaces.NsW, "tc") {
			var hasBlock, hasNestedTable bool
			lastBlock := ""
			for _, child := range cell.ChildElements() {
				switch kind := docxTableCellBlockKind(child); kind {
				case "p":
					hasBlock = true
					lastBlock = "p"
				case "tbl":
					hasBlock = true
					hasNestedTable = true
					lastBlock = "tbl"
				case "sdt", "customXml", "altChunk":
					hasBlock = true
					lastBlock = kind
				}
			}
			if !hasBlock {
				diags = append(diags, diag.Error(
					"DOCX_TABLE_SCAFFOLD",
					"table cell must contain direct block content such as w:p or w:tbl",
				))
				continue
			}
			if hasNestedTable && lastBlock != "p" {
				diags = append(diags, diag.Error(
					"DOCX_TABLE_SCAFFOLD",
					"table cell containing a nested w:tbl must end with a direct w:p paragraph",
				))
			}
		}
	}
	return diags
}

func docxTableCellBlockKind(elem *etree.Element) string {
	for _, local := range []string{"p", "tbl", "sdt", "customXml", "altChunk"} {
		if xmlx.ElementMatches(elem, namespaces.NsW, local) {
			return local
		}
	}
	return ""
}

func validateDOCXTableGrid(table, tblGrid *etree.Element) (result.Diagnostic, bool) {
	if table == nil || tblGrid == nil {
		return result.Diagnostic{}, false
	}
	rows := namespaces.FindChildren(table, namespaces.NsW, "tr")
	if len(rows) == 0 {
		return result.Diagnostic{}, false
	}

	gridCols := len(namespaces.FindChildren(tblGrid, namespaces.NsW, "gridCol"))
	widestRow := 0
	for _, row := range rows {
		if width := docxTableRowGridWidth(row); width > widestRow {
			widestRow = width
		}
	}
	if gridCols == widestRow {
		return result.Diagnostic{}, false
	}
	return diag.Error(
		"DOCX_TABLE_GRID_MISMATCH",
		fmt.Sprintf("table w:tblGrid has %d w:gridCol entries but the widest row uses %d grid columns", gridCols, widestRow),
	), true
}

func docxTableRowGridWidth(row *etree.Element) int {
	width := 0
	if trPr := namespaces.FindChild(row, namespaces.NsW, "trPr"); trPr != nil {
		width += docxNonNegativeWordIntAttr(namespaces.FindChild(trPr, namespaces.NsW, "gridBefore"), "val", 0)
		width += docxNonNegativeWordIntAttr(namespaces.FindChild(trPr, namespaces.NsW, "gridAfter"), "val", 0)
	}
	for _, cell := range namespaces.FindChildren(row, namespaces.NsW, "tc") {
		width += docxTableCellGridSpan(cell)
	}
	return width
}

func docxTableCellGridSpan(cell *etree.Element) int {
	tcPr := namespaces.FindChild(cell, namespaces.NsW, "tcPr")
	if tcPr == nil {
		return 1
	}
	return docxPositiveWordIntAttr(namespaces.FindChild(tcPr, namespaces.NsW, "gridSpan"), "val", 1)
}

func docxPositiveWordIntAttr(elem *etree.Element, localName string, fallback int) int {
	value, ok := namespaces.Attr(elem, namespaces.NsW, localName)
	if !ok {
		return fallback
	}
	parsed, err := strconv.Atoi(strings.TrimSpace(value))
	if err != nil || parsed < 1 {
		return fallback
	}
	return parsed
}

func docxNonNegativeWordIntAttr(elem *etree.Element, localName string, fallback int) int {
	value, ok := namespaces.Attr(elem, namespaces.NsW, localName)
	if !ok {
		return fallback
	}
	parsed, err := strconv.Atoi(strings.TrimSpace(value))
	if err != nil || parsed < 0 {
		return fallback
	}
	return parsed
}

func validateDOCXStyleReferences(body *etree.Element, styles []docxmodel.StyleInfo) []result.Diagnostic {
	var diags []result.Diagnostic
	styleTypes := make(map[string]string)
	styleCounts := make(map[string]int)
	for _, style := range styles {
		if style.StyleID == "" {
			continue
		}
		styleCounts[style.StyleID]++
		if _, exists := styleTypes[style.StyleID]; !exists {
			styleTypes[style.StyleID] = style.Type
		}
	}

	reportedDuplicates := make(map[string]bool)
	for styleID, count := range styleCounts {
		if count > 1 && !reportedDuplicates[styleID] {
			reportedDuplicates[styleID] = true
			diags = append(diags, diag.Error(
				"DOCX_DUPLICATE_STYLE_ID",
				fmt.Sprintf("styles part defines w:styleId %q %d times", styleID, count),
			))
		}
	}

	type styleRef struct {
		localName    string
		expectedType string
		description  string
	}
	refs := []styleRef{
		{localName: "pStyle", expectedType: "paragraph", description: "paragraph style"},
		{localName: "rStyle", expectedType: "character", description: "run style"},
		{localName: "tblStyle", expectedType: "table", description: "table style"},
	}
	reportedMissing := make(map[string]bool)
	reportedMismatch := make(map[string]bool)
	for _, ref := range refs {
		for _, elem := range namespaces.FindDescendants(body, namespaces.NsW, ref.localName) {
			styleID, ok := namespaces.Attr(elem, namespaces.NsW, "val")
			if !ok || strings.TrimSpace(styleID) == "" {
				continue
			}
			styleID = strings.TrimSpace(styleID)
			actualType, exists := styleTypes[styleID]
			if !exists {
				key := ref.localName + "\x00" + styleID
				if !reportedMissing[key] {
					reportedMissing[key] = true
					diags = append(diags, diag.Error(
						"DOCX_MISSING_STYLE_REFERENCE",
						fmt.Sprintf("%s references missing style %q", ref.description, styleID),
					))
				}
				continue
			}
			if actualType != "" && actualType != ref.expectedType {
				key := ref.localName + "\x00" + styleID + "\x00" + actualType
				if !reportedMismatch[key] {
					reportedMismatch[key] = true
					diags = append(diags, diag.Error(
						"DOCX_STYLE_TYPE_MISMATCH",
						fmt.Sprintf("%s references style %q of type %q; expected %q", ref.description, styleID, actualType, ref.expectedType),
					))
				}
			}
		}
	}
	return diags
}

func validateDOCXCommentReferences(session opc.PackageSession, documentURI string, body *etree.Element, partMap map[string]bool) []result.Diagnostic {
	var diags []result.Diagnostic
	refs := collectDOCXCommentReferenceIDs(body)
	if len(refs) == 0 {
		return diags
	}

	commentsURI, ok := findDOCXRelationshipTarget(session, documentURI, namespaces.RelComments)
	if !ok {
		diags = append(diags, diag.Error(
			"DOCX_MISSING_COMMENTS",
			"document references comments but has no comments relationship",
		))
		return diags
	}
	if !partMap[commentsURI] {
		diags = append(diags, diag.Error(
			"DOCX_MISSING_COMMENTS",
			"comments relationship points to missing part: "+commentsURI,
		))
		return diags
	}

	commentsDoc, err := session.ReadXMLPart(commentsURI)
	if err != nil {
		diags = append(diags, diag.Error(
			"DOCX_PARSE_ERROR",
			"failed to parse comments part: "+err.Error(),
		))
		return diags
	}
	root := commentsDoc.Root()
	if root == nil || !xmlx.ElementMatches(root, namespaces.NsW, "comments") {
		diags = append(diags, diag.Error(
			"DOCX_COMMENTS_ROOT_ERROR",
			"comments part root element not found",
		))
		return diags
	}

	defined := make(map[string]bool)
	counts := make(map[string]int)
	for _, comment := range namespaces.FindChildren(root, namespaces.NsW, "comment") {
		id, ok := namespaces.Attr(comment, namespaces.NsW, "id")
		if !ok || strings.TrimSpace(id) == "" {
			diags = append(diags, diag.Error(
				"DOCX_COMMENT_ID_INVALID",
				"comments part contains a comment without w:id",
			))
			continue
		}
		id = strings.TrimSpace(id)
		if !docxIsNonNegativeInteger(id) {
			diags = append(diags, diag.Error(
				"DOCX_COMMENT_ID_INVALID",
				fmt.Sprintf("comments part contains non-numeric w:id %q", id),
			))
			continue
		}
		defined[id] = true
		counts[id]++
	}
	for id, count := range counts {
		if count > 1 {
			diags = append(diags, diag.Error(
				"DOCX_DUPLICATE_COMMENT_ID",
				fmt.Sprintf("comments part defines w:id %s %d times", id, count),
			))
		}
	}

	for id := range refs {
		if !docxIsNonNegativeInteger(id) {
			diags = append(diags, diag.Error(
				"DOCX_COMMENT_REFERENCE_ID_INVALID",
				fmt.Sprintf("document contains non-numeric comment reference id %q", id),
			))
			continue
		}
		if !defined[id] {
			diags = append(diags, diag.Error(
				"DOCX_MISSING_COMMENT_REFERENCE",
				fmt.Sprintf("document references comment id %s but comments part does not define it", id),
			))
		}
	}
	return dedupeDOCXDiagnostics(diags)
}

func collectDOCXCommentReferenceIDs(body *etree.Element) map[string]bool {
	ids := make(map[string]bool)
	for _, localName := range []string{"commentRangeStart", "commentRangeEnd", "commentReference"} {
		for _, elem := range namespaces.FindDescendants(body, namespaces.NsW, localName) {
			if id, ok := namespaces.Attr(elem, namespaces.NsW, "id"); ok && strings.TrimSpace(id) != "" {
				ids[strings.TrimSpace(id)] = true
			}
		}
	}
	return ids
}

func findDOCXRelationshipTarget(session opc.PackageSession, sourceURI, relType string) (string, bool) {
	for _, rel := range session.ListRelationships(sourceURI) {
		if rel.TargetMode == "External" || rel.Type != relType {
			continue
		}
		return opc.NormalizeURI(opc.ResolveRelationshipTarget(sourceURI, rel.Target)), true
	}
	return "", false
}

func docxIsNonNegativeInteger(value string) bool {
	if value == "" {
		return false
	}
	for _, r := range value {
		if r < '0' || r > '9' {
			return false
		}
	}
	return true
}

func validateDocumentRelationships(session opc.PackageSession, documentURI string, partMap map[string]bool) []result.Diagnostic {
	var diags []result.Diagnostic
	for _, rel := range session.ListRelationships(documentURI) {
		if rel.TargetMode == "External" {
			continue
		}
		targetURI := opc.NormalizeURI(opc.ResolveRelationshipTarget(documentURI, rel.Target))
		switch rel.Type {
		case namespaces.RelHyperlink:
			if !partMap[targetURI] {
				diags = append(diags, diag.Warning(
					"DOCX_DANGLING_HYPERLINK",
					fmt.Sprintf("internal hyperlink relationship %s points to missing target: %s", rel.ID, targetURI),
				))
			}
		case namespaces.RelImage:
			if !partMap[targetURI] {
				diags = append(diags, diag.Error(
					"DOCX_MISSING_IMAGE",
					fmt.Sprintf("image relationship %s points to missing target: %s", rel.ID, targetURI),
				))
			}
		}
	}
	return diags
}

func validateImageReferences(doc *etree.Document, session opc.PackageSession, partURI string, partMap map[string]bool) []result.Diagnostic {
	var diags []result.Diagnostic
	root := doc.Root()
	if root == nil {
		return diags
	}

	relMap := make(map[string]opc.RelationshipInfo)
	for _, rel := range session.ListRelationships(partURI) {
		relMap[rel.ID] = rel
	}
	for _, elem := range root.FindElements(".//*") {
		if !xmlx.ElementMatches(elem, docxNsDrawingML, "blip") {
			continue
		}
		if rid, ok := namespaces.Attr(elem, namespaces.NsR, "embed"); ok && strings.TrimSpace(rid) != "" {
			diags = append(diags, validateDOCXImageRelationship(session, relMap, partMap, partURI, strings.TrimSpace(rid), true)...)
		}
		if rid, ok := namespaces.Attr(elem, namespaces.NsR, "link"); ok && strings.TrimSpace(rid) != "" {
			diags = append(diags, validateDOCXImageRelationship(session, relMap, partMap, partURI, strings.TrimSpace(rid), false)...)
		}
	}
	return dedupeDOCXDiagnostics(diags)
}

func validateHeaderFooterImageReferences(session opc.PackageSession, documentURI string, partMap map[string]bool) []result.Diagnostic {
	var diags []result.Diagnostic
	for _, partURI := range docxHeaderFooterPartURIs(session, documentURI) {
		if !partMap[partURI] {
			continue
		}
		doc, err := session.ReadXMLPart(partURI)
		if err != nil {
			diags = append(diags, diag.Error(
				"DOCX_PARSE_ERROR",
				fmt.Sprintf("failed to parse header/footer part %s: %v", partURI, err),
			))
			continue
		}
		diags = append(diags, validateImageReferences(doc, session, partURI, partMap)...)
	}
	return dedupeDOCXDiagnostics(diags)
}

func docxHeaderFooterPartURIs(session opc.PackageSession, documentURI string) []string {
	seen := make(map[string]bool)
	var uris []string
	add := func(uri string) {
		uri = opc.NormalizeURI(strings.TrimSpace(uri))
		if uri == "" || uri == "/" || seen[uri] {
			return
		}
		seen[uri] = true
		uris = append(uris, uri)
	}
	for _, part := range session.ListParts() {
		switch part.ContentType {
		case namespaces.ContentTypeHeader, namespaces.ContentTypeFooter:
			add(part.URI)
		}
	}
	listing, err := docxinspect.ListHeadersFooters(session, documentURI)
	if err != nil {
		return uris
	}
	for _, ref := range docxinspect.HeaderFooterRefs(listing, "") {
		if ref != nil {
			add(ref.PartURI)
		}
	}
	return uris
}

func validateDOCXImageRelationship(session opc.PackageSession, relMap map[string]opc.RelationshipInfo, partMap map[string]bool, partURI, rid string, embed bool) []result.Diagnostic {
	rel, ok := relMap[rid]
	if !ok {
		return []result.Diagnostic{diag.Error(
			"DOCX_MISSING_IMAGE_RELATIONSHIP",
			fmt.Sprintf("image reference %s has no relationship in %s", rid, partURI),
		)}
	}
	if rel.Type != namespaces.RelImage {
		return []result.Diagnostic{diag.Error(
			"DOCX_IMAGE_RELATIONSHIP_TYPE",
			fmt.Sprintf("image reference %s uses relationship type %s; expected %s", rid, rel.Type, namespaces.RelImage),
		)}
	}
	if rel.TargetMode == "External" {
		if embed {
			return []result.Diagnostic{diag.Error(
				"DOCX_IMAGE_RELATIONSHIP_EXTERNAL",
				fmt.Sprintf("embedded image reference %s points to an external target", rid),
			)}
		}
		return nil
	}
	targetURI := opc.NormalizeURI(opc.ResolveRelationshipTarget(partURI, rel.Target))
	if !partMap[targetURI] {
		return []result.Diagnostic{diag.Error(
			"DOCX_MISSING_IMAGE",
			fmt.Sprintf("image reference %s points to missing target: %s", rid, targetURI),
		)}
	}
	contentType := strings.TrimSpace(session.GetContentType(targetURI))
	if contentType != "" && !imagex.IsContentType(contentType) {
		return []result.Diagnostic{diag.Error(
			"DOCX_IMAGE_CONTENT_TYPE",
			fmt.Sprintf("image reference %s points to %s with content type %q; expected image/*", rid, targetURI, contentType),
		)}
	}
	if !imagex.HasKnownSignature(contentType) {
		return nil
	}
	raw, err := session.ReadRawPart(targetURI)
	if err != nil {
		return []result.Diagnostic{diag.Error(
			"DOCX_IMAGE_PAYLOAD",
			fmt.Sprintf("image reference %s points to %s but image payload could not be read: %v", rid, targetURI, err),
		)}
	}
	if !imagex.PayloadMatchesContentType(contentType, raw) {
		return []result.Diagnostic{diag.Error(
			"DOCX_IMAGE_PAYLOAD",
			fmt.Sprintf("image reference %s points to %s with content type %q but payload signature does not match", rid, targetURI, strings.TrimSpace(contentType)),
		)}
	}
	return nil
}

func validateDOCXDrawingExtents(root *etree.Element) []result.Diagnostic {
	var diags []result.Diagnostic
	for _, drawing := range namespaces.FindDescendants(root, namespaces.NsW, "drawing") {
		containers := append(xmlx.FindDescendants(drawing, docxNsWP, "inline"), xmlx.FindDescendants(drawing, docxNsWP, "anchor")...)
		for _, container := range containers {
			extent := xmlx.FindChild(container, docxNsWP, "extent")
			if extent == nil {
				diags = append(diags, diag.Error(
					"DOCX_DRAWING_MISSING_EXTENT",
					"w:drawing inline/anchor is missing wp:extent",
				))
				continue
			}
			cx, cxOK := docxPositiveIntAttribute(extent, "cx")
			cy, cyOK := docxPositiveIntAttribute(extent, "cy")
			if !cxOK || !cyOK {
				diags = append(diags, diag.Error(
					"DOCX_DRAWING_INVALID_EXTENT",
					fmt.Sprintf("w:drawing wp:extent must have positive cx/cy values; got cx=%q cy=%q", cx, cy),
				))
			}
		}
	}
	return dedupeDOCXDiagnostics(diags)
}

func docxPositiveIntAttribute(elem *etree.Element, name string) (string, bool) {
	attr := elem.SelectAttr(name)
	if attr == nil {
		return "", false
	}
	value := strings.TrimSpace(attr.Value)
	parsed, err := strconv.ParseInt(value, 10, 64)
	return value, err == nil && parsed > 0
}

func dedupeDOCXDiagnostics(diags []result.Diagnostic) []result.Diagnostic {
	seen := make(map[string]bool)
	var deduped []result.Diagnostic
	for _, diagnostic := range diags {
		key := diagnostic.Code + "\x00" + strings.TrimSpace(diagnostic.Message)
		if seen[key] {
			continue
		}
		seen[key] = true
		deduped = append(deduped, diagnostic)
	}
	return deduped
}
