package mutate

import (
	"crypto/rand"
	"fmt"
	"math/big"
	"regexp"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

var worksheetPartPattern = regexp.MustCompile(`^/xl/worksheets/sheet([0-9]+)\.xml$`)

type AddSheetRequest struct {
	Package        opc.PackageSession
	WorkbookURI    string
	ExistingSheets []model.SheetRef
	Name           string
	AfterPosition  int
}

type AddSheetResult struct {
	Number         int    `json:"number"`
	Name           string `json:"name"`
	SheetID        string `json:"sheetId"`
	RelationshipID string `json:"relationshipId"`
	PartURI        string `json:"partUri"`
}

type RenameSheetRequest struct {
	Package        opc.PackageSession
	WorkbookURI    string
	ExistingSheets []model.SheetRef
	SheetRef       model.SheetRef
	Name           string
}

type RenameSheetResult struct {
	Number       int    `json:"number"`
	Name         string `json:"name"`
	PreviousName string `json:"previousName"`
	SheetID      string `json:"sheetId"`
	PartURI      string `json:"partUri"`
}

type MoveSheetRequest struct {
	Package        opc.PackageSession
	WorkbookURI    string
	ExistingSheets []model.SheetRef
	SheetRef       model.SheetRef
	TargetPosition int
}

type MoveSheetResult struct {
	Number         int    `json:"number"`
	Name           string `json:"name"`
	SheetID        string `json:"sheetId"`
	RelationshipID string `json:"relationshipId"`
	PartURI        string `json:"partUri"`
	OldPosition    int    `json:"oldPosition"`
	NewPosition    int    `json:"newPosition"`
	IsNoOp         bool   `json:"isNoOp"`
}

type DeleteSheetRequest struct {
	Package        opc.PackageSession
	WorkbookURI    string
	ExistingSheets []model.SheetRef
	SheetRef       model.SheetRef
}

type DeleteSheetResult struct {
	Number                int      `json:"number"`
	Name                  string   `json:"name"`
	SheetID               string   `json:"sheetId"`
	RelationshipID        string   `json:"relationshipId"`
	PartURI               string   `json:"partUri"`
	RemovedRelationshipID string   `json:"removedRelationshipId"`
	RemovedParts          []string `json:"removedParts"`
	RemainingSheets       int      `json:"remainingSheets"`
}

func AddSheet(req *AddSheetRequest) (*AddSheetResult, error) {
	if req == nil {
		return nil, fmt.Errorf("add sheet request is nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if req.WorkbookURI == "" {
		return nil, fmt.Errorf("workbook URI cannot be empty")
	}
	if err := validateNewSheetName(req.Name, req.ExistingSheets, ""); err != nil {
		return nil, err
	}
	if req.AfterPosition < 0 || req.AfterPosition > len(req.ExistingSheets) {
		return nil, fmt.Errorf("after position %d out of range", req.AfterPosition)
	}

	workbookDoc, err := req.Package.ReadXMLPart(req.WorkbookURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read workbook %s: %w", req.WorkbookURI, err)
	}
	workbookRoot := workbookDoc.Root()
	if workbookRoot == nil || !namespaces.IsElement(workbookRoot, namespaces.NsSpreadsheetML, "workbook") {
		return nil, fmt.Errorf("workbook part %s root element not found", req.WorkbookURI)
	}
	prefix := workbookRoot.Space
	ensureNamespacePrefix(workbookRoot, "r", namespaces.NsR)

	sheetsElem := ensureWorkbookSheets(workbookRoot, prefix)
	relationshipID := nextRelationshipID(req.Package.ListRelationships(req.WorkbookURI))
	sheetID, err := nextSheetID(req.ExistingSheets)
	if err != nil {
		return nil, err
	}
	partURI := nextWorksheetPartURI(req.Package)
	worksheetDoc := newWorksheetDocument(prefix)
	worksheetBytes, err := worksheetDoc.WriteToBytes()
	if err != nil {
		return nil, fmt.Errorf("failed to serialize new worksheet: %w", err)
	}

	sheetElem := newSpreadsheetElement(prefix, "sheet")
	sheetElem.CreateAttr("name", req.Name)
	sheetElem.CreateAttr("sheetId", strconv.Itoa(sheetID))
	sheetElem.CreateAttr("r:id", relationshipID)
	insertSheetElement(sheetsElem, sheetElem, req.AfterPosition)

	relsDoc, err := readOrCreateWorkbookRels(req.Package, req.WorkbookURI)
	if err != nil {
		return nil, err
	}
	relElem := etree.NewElement("Relationship")
	relElem.CreateAttr("Id", relationshipID)
	relElem.CreateAttr("Type", namespaces.RelWorksheet)
	relElem.CreateAttr("Target", relationshipTarget(req.WorkbookURI, partURI))
	relsDoc.Root().AddChild(relElem)

	if err := req.Package.AddPart(partURI, worksheetBytes, namespaces.ContentTypeWorksheet, nil); err != nil {
		return nil, fmt.Errorf("failed to add worksheet %s: %w", partURI, err)
	}
	if err := req.Package.ReplaceXMLPart(workbookRelsURI(req.WorkbookURI), relsDoc); err != nil {
		return nil, fmt.Errorf("failed to replace workbook relationships: %w", err)
	}
	if err := req.Package.ReplaceXMLPart(req.WorkbookURI, workbookDoc); err != nil {
		return nil, fmt.Errorf("failed to replace workbook %s: %w", req.WorkbookURI, err)
	}

	return &AddSheetResult{
		Number:         insertedSheetNumber(len(req.ExistingSheets), req.AfterPosition),
		Name:           req.Name,
		SheetID:        strconv.Itoa(sheetID),
		RelationshipID: relationshipID,
		PartURI:        partURI,
	}, nil
}

func RenameSheet(req *RenameSheetRequest) (*RenameSheetResult, error) {
	if req == nil {
		return nil, fmt.Errorf("rename sheet request is nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if req.WorkbookURI == "" {
		return nil, fmt.Errorf("workbook URI cannot be empty")
	}
	if req.SheetRef.RelationshipID == "" {
		return nil, fmt.Errorf("sheet relationship ID cannot be empty")
	}
	if err := validateNewSheetName(req.Name, req.ExistingSheets, req.SheetRef.Name); err != nil {
		return nil, err
	}

	workbookDoc, err := req.Package.ReadXMLPart(req.WorkbookURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read workbook %s: %w", req.WorkbookURI, err)
	}
	root := workbookDoc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "workbook") {
		return nil, fmt.Errorf("workbook part %s root element not found", req.WorkbookURI)
	}
	sheetsElem := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheets")
	if sheetsElem == nil {
		return nil, fmt.Errorf("workbook has no sheets")
	}

	var target *etree.Element
	for _, sheetElem := range namespaces.FindChildren(sheetsElem, namespaces.NsSpreadsheetML, "sheet") {
		rid, _ := namespaces.Attr(sheetElem, namespaces.NsR, "id")
		if rid == req.SheetRef.RelationshipID {
			target = sheetElem
			break
		}
	}
	if target == nil {
		return nil, fmt.Errorf("sheet %q not found in workbook", req.SheetRef.Name)
	}

	target.CreateAttr("name", req.Name)
	if err := req.Package.ReplaceXMLPart(req.WorkbookURI, workbookDoc); err != nil {
		return nil, fmt.Errorf("failed to replace workbook %s: %w", req.WorkbookURI, err)
	}
	return &RenameSheetResult{
		Number:       req.SheetRef.Number,
		Name:         req.Name,
		PreviousName: req.SheetRef.Name,
		SheetID:      req.SheetRef.SheetID,
		PartURI:      req.SheetRef.PartURI,
	}, nil
}

func MoveSheet(req *MoveSheetRequest) (*MoveSheetResult, error) {
	if req == nil {
		return nil, fmt.Errorf("move sheet request is nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if req.WorkbookURI == "" {
		return nil, fmt.Errorf("workbook URI cannot be empty")
	}
	if req.SheetRef.RelationshipID == "" {
		return nil, fmt.Errorf("sheet relationship ID cannot be empty")
	}
	if len(req.ExistingSheets) == 0 {
		return nil, fmt.Errorf("workbook has no sheets")
	}
	if req.TargetPosition < 1 || req.TargetPosition > len(req.ExistingSheets) {
		return nil, fmt.Errorf("target position %d out of range", req.TargetPosition)
	}

	workbookDoc, err := req.Package.ReadXMLPart(req.WorkbookURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read workbook %s: %w", req.WorkbookURI, err)
	}
	root := workbookDoc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "workbook") {
		return nil, fmt.Errorf("workbook part %s root element not found", req.WorkbookURI)
	}
	sheetsElem := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheets")
	if sheetsElem == nil {
		return nil, fmt.Errorf("workbook has no sheets")
	}
	sheetElems := namespaces.FindChildren(sheetsElem, namespaces.NsSpreadsheetML, "sheet")
	if len(sheetElems) != len(req.ExistingSheets) {
		return nil, fmt.Errorf("workbook sheet count changed")
	}
	targetElem, oldIndex := findWorkbookSheetElementByRelID(sheetElems, req.SheetRef.RelationshipID)
	if targetElem == nil {
		return nil, fmt.Errorf("sheet %q not found in workbook", req.SheetRef.Name)
	}
	newIndex := req.TargetPosition - 1
	remap := moveSheetPositionRemap(len(sheetElems), oldIndex, newIndex)

	if oldIndex != newIndex {
		copyElem := targetElem.Copy()
		sheetsElem.RemoveChild(targetElem)
		remaining := namespaces.FindChildren(sheetsElem, namespaces.NsSpreadsheetML, "sheet")
		if newIndex >= len(remaining) {
			sheetsElem.AddChild(copyElem)
		} else {
			sheetsElem.InsertChildAt(remaining[newIndex].Index(), copyElem)
		}
	}
	applySheetPositionRemap(root, remap)

	if err := req.Package.ReplaceXMLPart(req.WorkbookURI, workbookDoc); err != nil {
		return nil, fmt.Errorf("failed to replace workbook %s: %w", req.WorkbookURI, err)
	}
	return &MoveSheetResult{
		Number:         req.TargetPosition,
		Name:           req.SheetRef.Name,
		SheetID:        req.SheetRef.SheetID,
		RelationshipID: req.SheetRef.RelationshipID,
		PartURI:        req.SheetRef.PartURI,
		OldPosition:    oldIndex + 1,
		NewPosition:    req.TargetPosition,
		IsNoOp:         oldIndex == newIndex,
	}, nil
}

func DeleteSheet(req *DeleteSheetRequest) (*DeleteSheetResult, error) {
	if req == nil {
		return nil, fmt.Errorf("delete sheet request is nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if req.WorkbookURI == "" {
		return nil, fmt.Errorf("workbook URI cannot be empty")
	}
	if req.SheetRef.RelationshipID == "" {
		return nil, fmt.Errorf("sheet relationship ID cannot be empty")
	}
	if len(req.ExistingSheets) <= 1 {
		return nil, fmt.Errorf("cannot delete the last sheet")
	}
	if !isWorksheetSheetRef(req.SheetRef) {
		return nil, fmt.Errorf("sheet %q is not a worksheet", req.SheetRef.Name)
	}
	if visibleSheetCount(req.ExistingSheets) <= 1 && sheetState(req.SheetRef) == model.SheetStateVisible {
		return nil, fmt.Errorf("cannot delete the last visible sheet")
	}

	workbookDoc, err := req.Package.ReadXMLPart(req.WorkbookURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read workbook %s: %w", req.WorkbookURI, err)
	}
	root := workbookDoc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "workbook") {
		return nil, fmt.Errorf("workbook part %s root element not found", req.WorkbookURI)
	}
	sheetsElem := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheets")
	if sheetsElem == nil {
		return nil, fmt.Errorf("workbook has no sheets")
	}
	sheetElems := namespaces.FindChildren(sheetsElem, namespaces.NsSpreadsheetML, "sheet")
	targetElem, deleteIndex := findWorkbookSheetElementByRelID(sheetElems, req.SheetRef.RelationshipID)
	if targetElem == nil {
		return nil, fmt.Errorf("sheet %q not found in workbook", req.SheetRef.Name)
	}

	sheetsElem.RemoveChild(targetElem)
	applySheetPositionRemap(root, deleteSheetPositionRemap(len(sheetElems), deleteIndex))

	relsDoc, err := readOrCreateWorkbookRels(req.Package, req.WorkbookURI)
	if err != nil {
		return nil, err
	}
	calcChainParts := removeWorkbookRelationships(relsDoc.Root(), req.WorkbookURI, func(rel *etree.Element) bool {
		id := rel.SelectAttrValue("Id", "")
		typ := rel.SelectAttrValue("Type", "")
		if id == req.SheetRef.RelationshipID {
			return true
		}
		return typ == namespaces.RelCalcChain
	})

	removedParts := []string{opc.NormalizeURI(req.SheetRef.PartURI)}
	if err := req.Package.RemovePart(req.SheetRef.PartURI); err != nil {
		return nil, fmt.Errorf("failed to remove worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	sheetRelsURI := relsURIForPart(req.SheetRef.PartURI)
	if partExists(req.Package, sheetRelsURI) {
		if err := req.Package.RemovePart(sheetRelsURI); err != nil {
			return nil, fmt.Errorf("failed to remove worksheet relationships %s: %w", sheetRelsURI, err)
		}
		removedParts = append(removedParts, sheetRelsURI)
	}
	for _, calcPart := range calcChainParts {
		if calcPart == "" || !partExists(req.Package, calcPart) {
			continue
		}
		if err := req.Package.RemovePart(calcPart); err != nil {
			return nil, fmt.Errorf("failed to remove calcChain %s: %w", calcPart, err)
		}
		removedParts = append(removedParts, calcPart)
	}

	if err := req.Package.ReplaceXMLPart(workbookRelsURI(req.WorkbookURI), relsDoc); err != nil {
		return nil, fmt.Errorf("failed to replace workbook relationships: %w", err)
	}
	if err := req.Package.ReplaceXMLPart(req.WorkbookURI, workbookDoc); err != nil {
		return nil, fmt.Errorf("failed to replace workbook %s: %w", req.WorkbookURI, err)
	}
	return &DeleteSheetResult{
		Number:                req.SheetRef.Number,
		Name:                  req.SheetRef.Name,
		SheetID:               req.SheetRef.SheetID,
		RelationshipID:        req.SheetRef.RelationshipID,
		PartURI:               req.SheetRef.PartURI,
		RemovedRelationshipID: req.SheetRef.RelationshipID,
		RemovedParts:          removedParts,
		RemainingSheets:       len(req.ExistingSheets) - 1,
	}, nil
}

func validateNewSheetName(name string, existing []model.SheetRef, currentName string) error {
	if strings.TrimSpace(name) == "" {
		return fmt.Errorf("sheet name cannot be empty")
	}
	if len([]rune(name)) > 31 {
		return fmt.Errorf("sheet name %q exceeds Excel's 31-character limit", name)
	}
	if strings.EqualFold(name, "History") {
		return fmt.Errorf("sheet name %q is reserved by Excel", name)
	}
	if strings.HasPrefix(name, "'") || strings.HasSuffix(name, "'") {
		return fmt.Errorf("sheet name cannot begin or end with apostrophe")
	}
	if strings.ContainsAny(name, `[]:*?/\`) {
		return fmt.Errorf("sheet name %q contains invalid Excel sheet-name characters", name)
	}
	for _, sheet := range existing {
		if currentName != "" && strings.EqualFold(sheet.Name, currentName) {
			continue
		}
		if strings.EqualFold(sheet.Name, name) {
			return fmt.Errorf("sheet name %q already exists", name)
		}
	}
	return nil
}

// sheetIDRandomCeiling bounds the random sheetId draw. Open XML SDK enforces
// SpreadsheetML's ST_SheetId maxInclusive value of 65534.
// We draw from a wide range so collisions with the (typically tiny) set of
// existing sheetIds are astronomically rare, and a freed id is effectively never
// reused — which is the property that keeps a pre-issued handle to a DELETED
// sheet resolving to zero matches (CodeScopeStale) instead of silently
// re-pointing at a newly added sheet that happened to inherit maxID+1.
const sheetIDRandomCeiling = 65534

// nextSheetID allocates a fresh, unique <sheet sheetId=> by drawing a random
// xsd:unsignedInt and retrying on the (vanishingly rare) collision with an
// existing sheetId. It deliberately does NOT return max(existing)+1: that scheme
// reuses a freed id after the highest-id sheet is deleted, so a handle minted
// before the delete would silently resolve to a DIFFERENT, newly added sheet
// (the cardinal wrong-target bug). Random allocation makes a deleted sheet's id
// stay gone, so the stale handle correctly finds nothing.
//
// sheetId is not required to be sequential or contiguous within the SDK-enforced
// range; find, selectors, and handles treat it as an opaque string, and the
// definedName localSheetId attribute is a sheet POSITION, an unrelated concept.
// Random allocation is therefore sound and needs no persisted high-water mark.
func nextSheetID(sheets []model.SheetRef) (int, error) {
	existing := make(map[int]bool, len(sheets))
	for _, sheet := range sheets {
		value, err := strconv.Atoi(sheet.SheetID)
		if err != nil {
			return 0, fmt.Errorf("invalid existing sheetId %q: %w", sheet.SheetID, err)
		}
		if value < 1 || value > sheetIDRandomCeiling {
			return 0, fmt.Errorf("invalid existing sheetId %q: must be between 1 and %d", sheet.SheetID, sheetIDRandomCeiling)
		}
		existing[value] = true
	}
	if len(existing) >= sheetIDRandomCeiling {
		return 0, fmt.Errorf("no available sheetId values remain")
	}
	for {
		n, err := rand.Int(rand.Reader, big.NewInt(sheetIDRandomCeiling))
		if err != nil {
			return 0, fmt.Errorf("failed to allocate sheetId: %w", err)
		}
		// Draw from [1, sheetIDRandomCeiling]; sheetId is 1-based by Excel
		// convention and 0 is avoided.
		candidate := int(n.Int64()) + 1
		if !existing[candidate] {
			return candidate, nil
		}
	}
}

func nextRelationshipID(rels []opc.RelationshipInfo) string {
	maxID := 0
	for _, rel := range rels {
		if !strings.HasPrefix(rel.ID, "rId") {
			continue
		}
		value, err := strconv.Atoi(strings.TrimPrefix(rel.ID, "rId"))
		if err == nil && value > maxID {
			maxID = value
		}
	}
	return fmt.Sprintf("rId%d", maxID+1)
}

func nextWorksheetPartURI(session opc.PackageSession) string {
	used := make(map[string]bool)
	maxIndex := 0
	for _, part := range session.ListParts() {
		uri := opc.NormalizeURI(part.URI)
		used[uri] = true
		if matches := worksheetPartPattern.FindStringSubmatch(uri); len(matches) == 2 {
			if value, err := strconv.Atoi(matches[1]); err == nil && value > maxIndex {
				maxIndex = value
			}
		}
	}
	for index := maxIndex + 1; ; index++ {
		uri := fmt.Sprintf("/xl/worksheets/sheet%d.xml", index)
		if !used[uri] {
			return uri
		}
	}
}

func newWorksheetDocument(prefix string) *etree.Document {
	doc := etree.NewDocument()
	root := newSpreadsheetElement(prefix, "worksheet")
	if prefix == "" {
		root.CreateAttr("xmlns", namespaces.NsSpreadsheetML)
	} else {
		root.CreateAttr("xmlns:"+prefix, namespaces.NsSpreadsheetML)
	}
	root.AddChild(newSpreadsheetElement(prefix, "sheetData"))
	doc.SetRoot(root)
	return doc
}

func ensureWorkbookSheets(root *etree.Element, prefix string) *etree.Element {
	if sheetsElem := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheets"); sheetsElem != nil {
		return sheetsElem
	}
	sheetsElem := newSpreadsheetElement(prefix, "sheets")
	if calcPr := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "calcPr"); calcPr != nil {
		root.InsertChildAt(calcPr.Index(), sheetsElem)
	} else {
		root.AddChild(sheetsElem)
	}
	return sheetsElem
}

func insertSheetElement(sheetsElem, sheetElem *etree.Element, afterPosition int) {
	children := namespaces.FindChildren(sheetsElem, namespaces.NsSpreadsheetML, "sheet")
	if afterPosition <= 0 || afterPosition >= len(children) {
		sheetsElem.AddChild(sheetElem)
		return
	}
	target := children[afterPosition-1]
	sheetsElem.InsertChildAt(target.Index()+1, sheetElem)
}

func insertedSheetNumber(existingCount, afterPosition int) int {
	if existingCount == 0 {
		return 1
	}
	if afterPosition <= 0 || afterPosition >= existingCount {
		return existingCount + 1
	}
	return afterPosition + 1
}

func findWorkbookSheetElementByRelID(sheetElems []*etree.Element, relationshipID string) (*etree.Element, int) {
	for i, sheetElem := range sheetElems {
		rid, _ := namespaces.Attr(sheetElem, namespaces.NsR, "id")
		if rid == relationshipID {
			return sheetElem, i
		}
	}
	return nil, -1
}

func moveSheetPositionRemap(count, oldIndex, newIndex int) []int {
	order := make([]int, 0, count)
	for i := 0; i < count; i++ {
		if i != oldIndex {
			order = append(order, i)
		}
	}
	if newIndex >= len(order) {
		order = append(order, oldIndex)
	} else {
		order = append(order[:newIndex], append([]int{oldIndex}, order[newIndex:]...)...)
	}
	remap := make([]int, count)
	for newPos, oldPos := range order {
		remap[oldPos] = newPos
	}
	return remap
}

func deleteSheetPositionRemap(count, deletedIndex int) []int {
	remap := make([]int, count)
	for i := 0; i < count; i++ {
		switch {
		case i == deletedIndex:
			remap[i] = -1
		case i < deletedIndex:
			remap[i] = i
		default:
			remap[i] = i - 1
		}
	}
	return remap
}

func applySheetPositionRemap(workbookRoot *etree.Element, remap []int) {
	if workbookRoot == nil || len(remap) == 0 {
		return
	}
	if bookViews := namespaces.FindChild(workbookRoot, namespaces.NsSpreadsheetML, "bookViews"); bookViews != nil {
		for _, workbookView := range namespaces.FindChildren(bookViews, namespaces.NsSpreadsheetML, "workbookView") {
			remapWorkbookViewPosition(workbookView, "activeTab", remap)
			remapWorkbookViewPosition(workbookView, "firstSheet", remap)
		}
	}
	if definedNames := namespaces.FindChild(workbookRoot, namespaces.NsSpreadsheetML, "definedNames"); definedNames != nil {
		for _, definedName := range append([]*etree.Element(nil), namespaces.FindChildren(definedNames, namespaces.NsSpreadsheetML, "definedName")...) {
			value, ok := intAttr(definedName, "localSheetId")
			if !ok {
				continue
			}
			if value < 0 || value >= len(remap) {
				continue
			}
			mapped := remap[value]
			if mapped < 0 {
				definedNames.RemoveChild(definedName)
				continue
			}
			definedName.CreateAttr("localSheetId", strconv.Itoa(mapped))
		}
		if len(namespaces.FindChildren(definedNames, namespaces.NsSpreadsheetML, "definedName")) == 0 {
			workbookRoot.RemoveChild(definedNames)
		}
	}
}

func remapWorkbookViewPosition(workbookView *etree.Element, attrName string, remap []int) {
	value, ok := intAttr(workbookView, attrName)
	if !ok {
		return
	}
	mapped := remapWorkbookViewIndex(value, remap)
	workbookView.CreateAttr(attrName, strconv.Itoa(mapped))
}

func remapWorkbookViewIndex(value int, remap []int) int {
	if len(remap) == 0 {
		return 0
	}
	if value < 0 {
		return 0
	}
	if value >= len(remap) {
		value = len(remap) - 1
	}
	mapped := remap[value]
	if mapped >= 0 {
		return mapped
	}
	for i := value; i < len(remap); i++ {
		if remap[i] >= 0 {
			return remap[i]
		}
	}
	for i := value - 1; i >= 0; i-- {
		if remap[i] >= 0 {
			return remap[i]
		}
	}
	return 0
}

func intAttr(elem *etree.Element, attrName string) (int, bool) {
	value := elem.SelectAttrValue(attrName, "")
	if value == "" {
		return 0, false
	}
	parsed, err := strconv.Atoi(value)
	if err != nil {
		return 0, false
	}
	return parsed, true
}

func isWorksheetSheetRef(sheet model.SheetRef) bool {
	if sheet.RelationshipType == namespaces.RelWorksheet {
		return true
	}
	if sheet.RelationshipType != "" {
		return false
	}
	return strings.HasPrefix(sheet.PartURI, "/xl/worksheets/")
}

func sheetState(sheet model.SheetRef) string {
	if sheet.State == "" {
		return model.SheetStateVisible
	}
	return sheet.State
}

func visibleSheetCount(sheets []model.SheetRef) int {
	count := 0
	for _, sheet := range sheets {
		if sheetState(sheet) == model.SheetStateVisible {
			count++
		}
	}
	return count
}

func removeWorkbookRelationships(root *etree.Element, workbookURI string, shouldRemove func(*etree.Element) bool) []string {
	if root == nil {
		return nil
	}
	removedTargets := []string{}
	for _, rel := range append([]*etree.Element(nil), root.ChildElements()...) {
		if localName(rel.Tag) != "Relationship" || !shouldRemove(rel) {
			continue
		}
		if target := rel.SelectAttrValue("Target", ""); target != "" && rel.SelectAttrValue("TargetMode", "") != "External" {
			removedTargets = append(removedTargets, opc.NormalizeURI(opc.ResolveRelationshipTarget(workbookURI, target)))
		}
		root.RemoveChild(rel)
	}
	return removedTargets
}

func relsURIForPart(partURI string) string {
	dir := opc.GetDirectory(partURI)
	return opc.JoinPaths(dir, "_rels/"+opc.GetFileName(partURI)+".rels")
}

func partExists(session opc.PackageSession, uri string) bool {
	if uri == "" {
		return false
	}
	_, err := session.ReadRawPart(uri)
	return err == nil
}

func localName(tag string) string {
	if idx := strings.LastIndex(tag, "}"); idx >= 0 && idx+1 < len(tag) {
		return tag[idx+1:]
	}
	if idx := strings.LastIndex(tag, ":"); idx >= 0 && idx+1 < len(tag) {
		return tag[idx+1:]
	}
	return tag
}

func readOrCreateWorkbookRels(session opc.PackageSession, workbookURI string) (*etree.Document, error) {
	relsURI := workbookRelsURI(workbookURI)
	doc, err := session.ReadXMLPart(relsURI)
	if err == nil {
		return doc, nil
	}
	doc = etree.NewDocument()
	root := etree.NewElement("Relationships")
	root.CreateAttr("xmlns", "http://schemas.openxmlformats.org/package/2006/relationships")
	doc.SetRoot(root)
	return doc, nil
}

func workbookRelsURI(workbookURI string) string {
	dir := opc.GetDirectory(workbookURI)
	return opc.JoinPaths(dir, "_rels/"+opc.GetFileName(workbookURI)+".rels")
}

func relationshipTarget(workbookURI, partURI string) string {
	workbookDir := opc.GetDirectory(workbookURI)
	prefix := workbookDir
	if prefix != "/" {
		prefix += "/"
	}
	if strings.HasPrefix(partURI, prefix) {
		return strings.TrimPrefix(partURI, prefix)
	}
	return partURI
}

func newSpreadsheetElement(prefix, tag string) *etree.Element {
	elem := etree.NewElement(tag)
	elem.Space = prefix
	return elem
}

func ensureNamespacePrefix(root *etree.Element, prefix, uri string) {
	if root == nil || prefix == "" || uri == "" {
		return
	}
	attrName := "xmlns:" + prefix
	if attr := root.SelectAttr(attrName); attr != nil {
		return
	}
	root.CreateAttr(attrName, uri)
}
