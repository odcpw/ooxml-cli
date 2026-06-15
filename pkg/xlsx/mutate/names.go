package mutate

import (
	"fmt"
	"regexp"
	"strconv"
	"strings"
	"unicode"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

var r1c1NamePattern = regexp.MustCompile(`(?i)^R[0-9]+C[0-9]+$`)

type DefinedNameScope struct {
	LocalSheetID *int
}

type AddDefinedNameRequest struct {
	Package     opc.PackageSession
	WorkbookURI string
	Name        string
	Ref         string
	Scope       DefinedNameScope
	Hidden      bool
	Comment     string
}

type UpdateDefinedNameRequest struct {
	Package     opc.PackageSession
	WorkbookURI string
	Target      model.DefinedName
	Ref         string
	ExpectRef   string
}

type RenameDefinedNameRequest struct {
	Package     opc.PackageSession
	WorkbookURI string
	Target      model.DefinedName
	NewName     string
	ExpectRef   string
}

type DeleteDefinedNameRequest struct {
	Package     opc.PackageSession
	WorkbookURI string
	Target      model.DefinedName
	ExpectRef   string
}

type DefinedNameMutationResult struct {
	Name         string `json:"name"`
	PreviousName string `json:"previousName,omitempty"`
	Ref          string `json:"ref,omitempty"`
	PreviousRef  string `json:"previousRef,omitempty"`
	Scope        string `json:"scope"`
	LocalSheetID *int   `json:"localSheetId,omitempty"`
}

func AddDefinedName(req *AddDefinedNameRequest) (*DefinedNameMutationResult, error) {
	if req == nil {
		return nil, fmt.Errorf("add defined name request is nil")
	}
	if err := validateDefinedNameRequest(req.Package, req.WorkbookURI); err != nil {
		return nil, err
	}
	name := strings.TrimSpace(req.Name)
	if err := ValidateDefinedName(name); err != nil {
		return nil, err
	}
	ref, err := NormalizeDefinedNameRef(req.Ref)
	if err != nil {
		return nil, err
	}

	doc, root, err := readWorkbookForDefinedNames(req.Package, req.WorkbookURI)
	if err != nil {
		return nil, err
	}
	definedNames := ensureWorkbookDefinedNames(root, root.Space)
	if duplicateDefinedName(definedNames, name, req.Scope.LocalSheetID, nil) {
		return nil, fmt.Errorf("defined name %q already exists in %s scope", name, definedNameScopeText(req.Scope.LocalSheetID))
	}

	elem := newSpreadsheetElement(root.Space, "definedName")
	elem.CreateAttr("name", name)
	if req.Scope.LocalSheetID != nil {
		elem.CreateAttr("localSheetId", strconv.Itoa(*req.Scope.LocalSheetID))
	}
	if req.Hidden {
		elem.CreateAttr("hidden", "1")
	}
	if strings.TrimSpace(req.Comment) != "" {
		elem.CreateAttr("comment", strings.TrimSpace(req.Comment))
	}
	elem.SetText(ref)
	definedNames.AddChild(elem)

	if err := req.Package.ReplaceXMLPart(req.WorkbookURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace workbook %s: %w", req.WorkbookURI, err)
	}
	return &DefinedNameMutationResult{
		Name:         name,
		Ref:          ref,
		Scope:        definedNameScopeText(req.Scope.LocalSheetID),
		LocalSheetID: cloneIntPtr(req.Scope.LocalSheetID),
	}, nil
}

func UpdateDefinedName(req *UpdateDefinedNameRequest) (*DefinedNameMutationResult, error) {
	if req == nil {
		return nil, fmt.Errorf("update defined name request is nil")
	}
	if err := validateDefinedNameRequest(req.Package, req.WorkbookURI); err != nil {
		return nil, err
	}
	ref, err := NormalizeDefinedNameRef(req.Ref)
	if err != nil {
		return nil, err
	}
	doc, _, elem, err := readTargetDefinedName(req.Package, req.WorkbookURI, req.Target)
	if err != nil {
		return nil, err
	}
	previousRef := strings.TrimSpace(elem.Text())
	if err := checkExpectedDefinedNameRef(previousRef, req.ExpectRef); err != nil {
		return nil, err
	}
	elem.SetText(ref)
	if err := req.Package.ReplaceXMLPart(req.WorkbookURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace workbook %s: %w", req.WorkbookURI, err)
	}
	return &DefinedNameMutationResult{
		Name:         elem.SelectAttrValue("name", ""),
		Ref:          ref,
		PreviousRef:  previousRef,
		Scope:        scopeTextFromElement(elem),
		LocalSheetID: localSheetIDFromElement(elem),
	}, nil
}

func RenameDefinedName(req *RenameDefinedNameRequest) (*DefinedNameMutationResult, error) {
	if req == nil {
		return nil, fmt.Errorf("rename defined name request is nil")
	}
	if err := validateDefinedNameRequest(req.Package, req.WorkbookURI); err != nil {
		return nil, err
	}
	newName := strings.TrimSpace(req.NewName)
	if err := ValidateDefinedName(newName); err != nil {
		return nil, err
	}
	doc, root, elem, err := readTargetDefinedName(req.Package, req.WorkbookURI, req.Target)
	if err != nil {
		return nil, err
	}
	previousRef := strings.TrimSpace(elem.Text())
	if err := checkExpectedDefinedNameRef(previousRef, req.ExpectRef); err != nil {
		return nil, err
	}
	definedNames := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "definedNames")
	if duplicateDefinedName(definedNames, newName, localSheetIDFromElement(elem), elem) {
		return nil, fmt.Errorf("defined name %q already exists in %s scope", newName, scopeTextFromElement(elem))
	}
	previousName := elem.SelectAttrValue("name", "")
	elem.CreateAttr("name", newName)
	if err := req.Package.ReplaceXMLPart(req.WorkbookURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace workbook %s: %w", req.WorkbookURI, err)
	}
	return &DefinedNameMutationResult{
		Name:         newName,
		PreviousName: previousName,
		Ref:          previousRef,
		Scope:        scopeTextFromElement(elem),
		LocalSheetID: localSheetIDFromElement(elem),
	}, nil
}

func DeleteDefinedName(req *DeleteDefinedNameRequest) (*DefinedNameMutationResult, error) {
	if req == nil {
		return nil, fmt.Errorf("delete defined name request is nil")
	}
	if err := validateDefinedNameRequest(req.Package, req.WorkbookURI); err != nil {
		return nil, err
	}
	doc, root, elem, err := readTargetDefinedName(req.Package, req.WorkbookURI, req.Target)
	if err != nil {
		return nil, err
	}
	previousRef := strings.TrimSpace(elem.Text())
	if err := checkExpectedDefinedNameRef(previousRef, req.ExpectRef); err != nil {
		return nil, err
	}
	definedNames := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "definedNames")
	result := &DefinedNameMutationResult{
		Name:         elem.SelectAttrValue("name", ""),
		Ref:          previousRef,
		Scope:        scopeTextFromElement(elem),
		LocalSheetID: localSheetIDFromElement(elem),
	}
	definedNames.RemoveChild(elem)
	if len(namespaces.FindChildren(definedNames, namespaces.NsSpreadsheetML, "definedName")) == 0 {
		root.RemoveChild(definedNames)
	}
	if err := req.Package.ReplaceXMLPart(req.WorkbookURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace workbook %s: %w", req.WorkbookURI, err)
	}
	return result, nil
}

func validateDefinedNameRequest(session opc.PackageSession, workbookURI string) error {
	if session == nil {
		return fmt.Errorf("package session is nil")
	}
	if workbookURI == "" {
		return fmt.Errorf("workbook URI cannot be empty")
	}
	return nil
}

func ValidateDefinedName(name string) error {
	if name == "" {
		return fmt.Errorf("defined name cannot be empty")
	}
	if len([]rune(name)) > 255 {
		return fmt.Errorf("defined name %q exceeds Excel's 255-character limit", name)
	}
	first := []rune(name)[0]
	if !(unicode.IsLetter(first) || first == '_' || first == '\\') {
		return fmt.Errorf("defined name %q must start with a letter, underscore, or backslash", name)
	}
	for _, r := range name {
		if unicode.IsLetter(r) || unicode.IsDigit(r) || r == '_' || r == '.' || r == '\\' {
			continue
		}
		return fmt.Errorf("defined name %q contains invalid characters", name)
	}
	if _, err := address.ParseCell(name); err == nil {
		return fmt.Errorf("defined name %q cannot be an A1 cell reference", name)
	}
	if r1c1NamePattern.MatchString(name) {
		return fmt.Errorf("defined name %q cannot be an R1C1 cell reference", name)
	}
	return nil
}

func NormalizeDefinedNameRef(ref string) (string, error) {
	ref = strings.TrimSpace(ref)
	ref = strings.TrimPrefix(ref, "=")
	ref = strings.TrimSpace(ref)
	if ref == "" {
		return "", fmt.Errorf("defined name ref cannot be empty")
	}
	return ref, nil
}

func DefinedNameRangeRef(sheetName string, rangeRef address.RangeRef) string {
	return quoteDefinedNameSheet(sheetName) + "!" + absoluteRangeRef(rangeRef)
}

func quoteDefinedNameSheet(sheetName string) string {
	escaped := strings.ReplaceAll(sheetName, "'", "''")
	return "'" + escaped + "'"
}

func absoluteRangeRef(rangeRef address.RangeRef) string {
	start := rangeRef.Start
	end := rangeRef.End
	start.AbsColumn = true
	start.AbsRow = true
	end.AbsColumn = true
	end.AbsRow = true
	if start.String() == end.String() {
		return start.String()
	}
	return start.String() + ":" + end.String()
}

func readWorkbookForDefinedNames(session opc.PackageSession, workbookURI string) (*etree.Document, *etree.Element, error) {
	doc, err := session.ReadXMLPart(workbookURI)
	if err != nil {
		return nil, nil, fmt.Errorf("failed to read workbook %s: %w", workbookURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "workbook") {
		return nil, nil, fmt.Errorf("workbook part %s root element not found", workbookURI)
	}
	return doc, root, nil
}

func readTargetDefinedName(session opc.PackageSession, workbookURI string, target model.DefinedName) (*etree.Document, *etree.Element, *etree.Element, error) {
	doc, root, err := readWorkbookForDefinedNames(session, workbookURI)
	if err != nil {
		return nil, nil, nil, err
	}
	definedNames := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "definedNames")
	if definedNames == nil {
		return nil, nil, nil, fmt.Errorf("workbook has no defined names")
	}
	children := namespaces.FindChildren(definedNames, namespaces.NsSpreadsheetML, "definedName")
	if target.Number < 1 || target.Number > len(children) {
		return nil, nil, nil, fmt.Errorf("defined name %q is no longer present", target.Name)
	}
	elem := children[target.Number-1]
	if !strings.EqualFold(elem.SelectAttrValue("name", ""), target.Name) || !sameLocalSheetID(localSheetIDFromElement(elem), target.LocalSheetID) {
		for _, candidate := range children {
			if strings.EqualFold(candidate.SelectAttrValue("name", ""), target.Name) && sameLocalSheetID(localSheetIDFromElement(candidate), target.LocalSheetID) {
				return doc, root, candidate, nil
			}
		}
		return nil, nil, nil, fmt.Errorf("defined name %q is no longer present in %s scope", target.Name, definedNameScopeText(target.LocalSheetID))
	}
	return doc, root, elem, nil
}

func ensureWorkbookDefinedNames(root *etree.Element, prefix string) *etree.Element {
	if definedNames := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "definedNames"); definedNames != nil {
		return definedNames
	}
	definedNames := newSpreadsheetElement(prefix, "definedNames")
	insertWorkbookChild(root, definedNames, "definedNames")
	return definedNames
}

func insertWorkbookChild(root, child *etree.Element, localName string) {
	targetOrder := workbookChildOrder(localName)
	for _, existing := range root.ChildElements() {
		if workbookChildOrder(existing.Tag) > targetOrder {
			root.InsertChildAt(existing.Index(), child)
			return
		}
	}
	root.AddChild(child)
}

func workbookChildOrder(localName string) int {
	switch localName {
	case "fileVersion":
		return 10
	case "fileSharing":
		return 20
	case "workbookPr":
		return 30
	case "workbookProtection":
		return 40
	case "bookViews":
		return 50
	case "sheets":
		return 60
	case "functionGroups":
		return 70
	case "externalReferences":
		return 80
	case "definedNames":
		return 90
	case "calcPr":
		return 100
	case "oleSize":
		return 110
	case "customWorkbookViews":
		return 120
	case "pivotCaches":
		return 130
	case "smartTagPr":
		return 140
	case "smartTagTypes":
		return 150
	case "webPublishing":
		return 160
	case "fileRecoveryPr":
		return 170
	case "webPublishObjects":
		return 180
	case "extLst":
		return 190
	default:
		return 1000
	}
}

func duplicateDefinedName(definedNames *etree.Element, name string, localSheetID *int, skip *etree.Element) bool {
	if definedNames == nil {
		return false
	}
	for _, elem := range namespaces.FindChildren(definedNames, namespaces.NsSpreadsheetML, "definedName") {
		if skip != nil && elem == skip {
			continue
		}
		if strings.EqualFold(elem.SelectAttrValue("name", ""), name) && sameLocalSheetID(localSheetIDFromElement(elem), localSheetID) {
			return true
		}
	}
	return false
}

func checkExpectedDefinedNameRef(actual, expected string) error {
	expected = strings.TrimSpace(expected)
	expected = strings.TrimPrefix(expected, "=")
	expected = strings.TrimSpace(expected)
	if expected == "" {
		return nil
	}
	if actual != expected {
		return fmt.Errorf("defined name ref mismatch: expected %q, found %q", expected, actual)
	}
	return nil
}

func localSheetIDFromElement(elem *etree.Element) *int {
	value := elem.SelectAttrValue("localSheetId", "")
	if value == "" {
		return nil
	}
	parsed, err := strconv.Atoi(value)
	if err != nil {
		return nil
	}
	return &parsed
}

func sameLocalSheetID(a, b *int) bool {
	if a == nil || b == nil {
		return a == nil && b == nil
	}
	return *a == *b
}

func scopeTextFromElement(elem *etree.Element) string {
	return definedNameScopeText(localSheetIDFromElement(elem))
}

func definedNameScopeText(localSheetID *int) string {
	if localSheetID == nil {
		return "workbook"
	}
	return "sheet"
}

func cloneIntPtr(value *int) *int {
	if value == nil {
		return nil
	}
	clone := *value
	return &clone
}
