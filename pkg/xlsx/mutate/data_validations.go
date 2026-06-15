package mutate

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

// DataValidation describes one worksheet data validation rule.
type DataValidation struct {
	Sqref            string `json:"sqref"`
	Type             string `json:"type,omitempty"`
	Operator         string `json:"operator,omitempty"`
	Formula1         string `json:"formula1,omitempty"`
	Formula2         string `json:"formula2,omitempty"`
	AllowBlank       bool   `json:"allowBlank"`
	ShowInputMessage bool   `json:"showInputMessage"`
	ShowErrorMessage bool   `json:"showErrorMessage"`
	PromptTitle      string `json:"promptTitle,omitempty"`
	Prompt           string `json:"prompt,omitempty"`
	ErrorTitle       string `json:"errorTitle,omitempty"`
	Error            string `json:"error,omitempty"`
	ErrorStyle       string `json:"errorStyle,omitempty"`
}

// DataValidationMutationResult reports a data validation mutation outcome.
type DataValidationMutationResult struct {
	Sqref         string `json:"sqref"`
	Validation    DataValidation
	CellsAffected int
}

// DataValidationFields carries the desired attributes for a validation.
type DataValidationFields struct {
	Type             string
	Operator         string
	Formula1         string
	Formula2         string
	ListValues       string
	ListRange        string
	AllowBlank       bool
	ShowInputMessage bool
	ShowErrorMessage bool
	PromptTitle      string
	Prompt           string
	ErrorTitle       string
	Error            string
	ErrorStyle       string

	SetType             bool
	SetOperator         bool
	SetFormula1         bool
	SetFormula2         bool
	SetListValues       bool
	SetListRange        bool
	SetAllowBlank       bool
	SetShowInputMessage bool
	SetShowErrorMessage bool
	SetPromptTitle      bool
	SetPrompt           bool
	SetErrorTitle       bool
	SetError            bool
	SetErrorStyle       bool
}

// CreateDataValidationRequest creates a new data validation rule.
type CreateDataValidationRequest struct {
	Package  opc.PackageSession
	SheetRef model.SheetRef
	Range    string
	Fields   DataValidationFields
}

// UpdateDataValidationRequest mutates an existing data validation rule.
type UpdateDataValidationRequest struct {
	Package        opc.PackageSession
	SheetRef       model.SheetRef
	Range          string
	Fields         DataValidationFields
	ExpectType     string
	HasExpectType  bool
	ExpectFormula1 string
	HasExpectF1    bool
}

// DeleteDataValidationRequest removes a data validation rule by sqref.
type DeleteDataValidationRequest struct {
	Package        opc.PackageSession
	SheetRef       model.SheetRef
	Range          string
	ExpectType     string
	HasExpectType  bool
	ExpectFormula1 string
	HasExpectF1    bool
}

var validDataValidationTypes = map[string]bool{
	"list":       true,
	"whole":      true,
	"decimal":    true,
	"date":       true,
	"time":       true,
	"textLength": true,
	"custom":     true,
}

var validDataValidationOperators = map[string]bool{
	"between":            true,
	"notBetween":         true,
	"equal":              true,
	"notEqual":           true,
	"greaterThan":        true,
	"lessThan":           true,
	"greaterThanOrEqual": true,
	"lessThanOrEqual":    true,
}

var validErrorStyles = map[string]bool{
	"stop":        true,
	"warning":     true,
	"information": true,
}

// NormalizeDataValidationType maps CLI aliases to schema type names.
func NormalizeDataValidationType(t string) string {
	switch strings.TrimSpace(t) {
	case "text-length", "textLength", "textlength":
		return "textLength"
	default:
		return strings.TrimSpace(t)
	}
}

func validateDataValidationType(t string) error {
	if t == "" {
		return nil
	}
	if !validDataValidationTypes[t] {
		return fmt.Errorf("invalid data validation type %q (want list, whole, decimal, date, time, textLength, custom)", t)
	}
	return nil
}

func validateDataValidationOperator(op, t string) error {
	if op == "" {
		return nil
	}
	if !validDataValidationOperators[op] {
		return fmt.Errorf("invalid operator %q", op)
	}
	if t == "list" || t == "custom" {
		return fmt.Errorf("operator is not valid for type %q", t)
	}
	return nil
}

// NormalizeSqref canonicalizes a space-delimited ST_Sqref (cells and ranges).
func NormalizeSqref(ref string) (string, error) {
	ref = strings.TrimSpace(ref)
	if ref == "" {
		return "", fmt.Errorf("range cannot be empty")
	}
	fields := strings.Fields(ref)
	out := make([]string, 0, len(fields))
	for _, f := range fields {
		if strings.Contains(f, ":") {
			parsed, err := address.ParseRange(f)
			if err != nil {
				return "", err
			}
			out = append(out, parsed.String())
		} else {
			parsed, err := address.ParseCell(f)
			if err != nil {
				return "", err
			}
			out = append(out, parsed.String())
		}
	}
	return strings.Join(out, " "), nil
}

func sqrefCellCount(sqref string) int {
	total := 0
	for _, f := range strings.Fields(sqref) {
		if strings.Contains(f, ":") {
			if r, err := address.ParseRange(f); err == nil {
				cols := r.End.Column - r.Start.Column + 1
				rows := r.End.Row - r.Start.Row + 1
				if cols > 0 && rows > 0 {
					total += cols * rows
				}
				continue
			}
		}
		total++
	}
	return total
}

// ListDataValidations returns all data validations on a worksheet.
func ListDataValidations(session opc.PackageSession, sheet model.SheetRef) ([]DataValidation, error) {
	_, root, err := readWorksheetRoot(session, sheet)
	if err != nil {
		return nil, err
	}
	var out []DataValidation
	container := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "dataValidations")
	if container == nil {
		return out, nil
	}
	for _, dv := range namespaces.FindChildren(container, namespaces.NsSpreadsheetML, "dataValidation") {
		out = append(out, dataValidationFromElem(dv))
	}
	return out, nil
}

func dataValidationFromElem(dv *etree.Element) DataValidation {
	entry := DataValidation{
		Sqref:            dv.SelectAttrValue("sqref", ""),
		Type:             dv.SelectAttrValue("type", ""),
		Operator:         dv.SelectAttrValue("operator", ""),
		AllowBlank:       attrIsTrue(dv, "allowBlank"),
		ShowInputMessage: attrIsTrue(dv, "showInputMessage"),
		ShowErrorMessage: attrIsTrue(dv, "showErrorMessage"),
		PromptTitle:      dv.SelectAttrValue("promptTitle", ""),
		Prompt:           dv.SelectAttrValue("prompt", ""),
		ErrorTitle:       dv.SelectAttrValue("errorTitle", ""),
		Error:            dv.SelectAttrValue("error", ""),
		ErrorStyle:       dv.SelectAttrValue("errorStyle", ""),
	}
	if f1 := namespaces.FindChild(dv, namespaces.NsSpreadsheetML, "formula1"); f1 != nil {
		entry.Formula1 = f1.Text()
	}
	if f2 := namespaces.FindChild(dv, namespaces.NsSpreadsheetML, "formula2"); f2 != nil {
		entry.Formula2 = f2.Text()
	}
	return entry
}

func attrIsTrue(elem *etree.Element, key string) bool {
	v := strings.TrimSpace(elem.SelectAttrValue(key, ""))
	return v == "1" || v == "true"
}

func findDataValidationElem(container *etree.Element, normSqref string) *etree.Element {
	for _, dv := range namespaces.FindChildren(container, namespaces.NsSpreadsheetML, "dataValidation") {
		if existing, err := NormalizeSqref(dv.SelectAttrValue("sqref", "")); err == nil && existing == normSqref {
			return dv
		}
	}
	return nil
}

func updateDataValidationCount(container *etree.Element) {
	n := len(namespaces.FindChildren(container, namespaces.NsSpreadsheetML, "dataValidation"))
	container.CreateAttr("count", strconv.Itoa(n))
}

// resolveListFormula1 builds the formula1 value for a list-type validation.
func resolveListFormula1(values, listRange string) (string, error) {
	values = strings.TrimSpace(values)
	listRange = strings.TrimSpace(listRange)
	if (values == "") == (listRange == "") {
		return "", fmt.Errorf("list type requires exactly one of list-values or list-range")
	}
	if listRange != "" {
		return listRange, nil
	}
	// Inline values are wrapped in literal quotes; embedded quotes escaped.
	return `"` + strings.ReplaceAll(values, `"`, `""`) + `"`, nil
}

func setBoolAttr(elem *etree.Element, key string, val bool) {
	if val {
		elem.CreateAttr(key, "1")
	} else {
		elem.RemoveAttr(key)
	}
}

func setStringAttr(elem *etree.Element, key, val string) {
	if val == "" {
		elem.RemoveAttr(key)
	} else {
		elem.CreateAttr(key, val)
	}
}

// setFormulaChild sets or removes a formula1/formula2 child in schema order.
func setFormulaChild(dv *etree.Element, prefix, name, value string) {
	existing := namespaces.FindChild(dv, namespaces.NsSpreadsheetML, name)
	if value == "" {
		if existing != nil {
			dv.RemoveChild(existing)
		}
		return
	}
	if existing != nil {
		existing.SetText(value)
		return
	}
	child := newElement(prefix, name)
	child.SetText(value)
	if name == "formula1" {
		// formula1 must come before formula2.
		if f2 := namespaces.FindChild(dv, namespaces.NsSpreadsheetML, "formula2"); f2 != nil {
			dv.InsertChildAt(f2.Index(), child)
			return
		}
	}
	dv.AddChild(child)
}

// applyFields applies the desired fields to a dataValidation element. When
// create is true, unset toggle fields use their zero defaults.
func applyDataValidationFields(dv *etree.Element, prefix string, f DataValidationFields, create bool) error {
	dvType := NormalizeDataValidationType(f.Type)
	if create {
		setStringAttr(dv, "type", dvType)
	} else if f.SetType {
		setStringAttr(dv, "type", dvType)
	}
	effectiveType := NormalizeDataValidationType(dv.SelectAttrValue("type", ""))

	if create || f.SetOperator {
		setStringAttr(dv, "operator", f.Operator)
	}
	effectiveOperator := dv.SelectAttrValue("operator", "")

	if err := validateDataValidationType(effectiveType); err != nil {
		return err
	}
	if err := validateDataValidationOperator(effectiveOperator, effectiveType); err != nil {
		return err
	}
	if f.ErrorStyle != "" && !validErrorStyles[f.ErrorStyle] {
		return fmt.Errorf("invalid error-style %q (want stop, warning, information)", f.ErrorStyle)
	}

	// Formula handling.
	if effectiveType == "list" && (f.SetListValues || f.SetListRange || (create && (f.ListValues != "" || f.ListRange != ""))) {
		formula1, err := resolveListFormula1(f.ListValues, f.ListRange)
		if err != nil {
			return err
		}
		setFormulaChild(dv, prefix, "formula1", formula1)
		setFormulaChild(dv, prefix, "formula2", "")
	} else {
		if create || f.SetFormula1 {
			setFormulaChild(dv, prefix, "formula1", f.Formula1)
		}
		if create || f.SetFormula2 {
			setFormulaChild(dv, prefix, "formula2", f.Formula2)
		}
	}

	// Operator/formula consistency for between/notBetween.
	if effectiveOperator == "between" || effectiveOperator == "notBetween" {
		if namespaces.FindChild(dv, namespaces.NsSpreadsheetML, "formula2") == nil {
			return fmt.Errorf("operator %q requires formula2", effectiveOperator)
		}
	}
	if effectiveType != "" && namespaces.FindChild(dv, namespaces.NsSpreadsheetML, "formula1") == nil {
		if effectiveType == "list" {
			return fmt.Errorf("type %q requires --list-values or --list-range", effectiveType)
		}
		return fmt.Errorf("type %q requires formula1", effectiveType)
	}

	if create || f.SetAllowBlank {
		setBoolAttr(dv, "allowBlank", f.AllowBlank)
	}
	if create || f.SetShowInputMessage {
		setBoolAttr(dv, "showInputMessage", f.ShowInputMessage)
	}
	if create || f.SetShowErrorMessage {
		setBoolAttr(dv, "showErrorMessage", f.ShowErrorMessage)
	}
	if create || f.SetPromptTitle {
		setStringAttr(dv, "promptTitle", f.PromptTitle)
	}
	if create || f.SetPrompt {
		setStringAttr(dv, "prompt", f.Prompt)
	}
	if create || f.SetErrorTitle {
		setStringAttr(dv, "errorTitle", f.ErrorTitle)
	}
	if create || f.SetError {
		setStringAttr(dv, "error", f.Error)
	}
	if create || f.SetErrorStyle {
		setStringAttr(dv, "errorStyle", f.ErrorStyle)
	}
	return nil
}

// CreateDataValidation adds a new data validation rule on a sqref.
func CreateDataValidation(req *CreateDataValidationRequest) (*DataValidationMutationResult, error) {
	if req == nil {
		return nil, fmt.Errorf("create data validation request is nil")
	}
	normSqref, err := NormalizeSqref(req.Range)
	if err != nil {
		return nil, err
	}
	dvType := NormalizeDataValidationType(req.Fields.Type)
	if dvType == "" {
		return nil, fmt.Errorf("--type is required")
	}
	doc, root, err := readWorksheetRoot(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	prefix := root.Space
	container := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "dataValidations")
	if container == nil {
		container = newElement(prefix, "dataValidations")
		insertWorksheetChild(root, container, "dataValidations")
	}
	if existing := findDataValidationElem(container, normSqref); existing != nil {
		return nil, fmt.Errorf("a data validation already exists on %s (use update)", normSqref)
	}
	dv := newElement(prefix, "dataValidation")
	dv.CreateAttr("sqref", normSqref)
	// Attach before applying fields so the worksheet's default namespace is in
	// scope for child-element lookups during validation.
	container.AddChild(dv)
	if err := applyDataValidationFields(dv, prefix, req.Fields, true); err != nil {
		container.RemoveChild(dv)
		if len(namespaces.FindChildren(container, namespaces.NsSpreadsheetML, "dataValidation")) == 0 {
			root.RemoveChild(container)
		}
		return nil, err
	}
	updateDataValidationCount(container)
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return &DataValidationMutationResult{Sqref: normSqref, Validation: dataValidationFromElem(dv), CellsAffected: sqrefCellCount(normSqref)}, nil
}

// UpdateDataValidation mutates an existing data validation rule.
func UpdateDataValidation(req *UpdateDataValidationRequest) (*DataValidationMutationResult, error) {
	if req == nil {
		return nil, fmt.Errorf("update data validation request is nil")
	}
	normSqref, err := NormalizeSqref(req.Range)
	if err != nil {
		return nil, err
	}
	doc, root, err := readWorksheetRoot(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	container := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "dataValidations")
	var dv *etree.Element
	if container != nil {
		dv = findDataValidationElem(container, normSqref)
	}
	if dv == nil {
		return nil, fmt.Errorf("no data validation found on %s", normSqref)
	}
	current := dataValidationFromElem(dv)
	if err := checkDataValidationGuards(current, req.HasExpectType, req.ExpectType, req.HasExpectF1, req.ExpectFormula1); err != nil {
		return nil, err
	}
	if err := applyDataValidationFields(dv, root.Space, req.Fields, false); err != nil {
		return nil, err
	}
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return &DataValidationMutationResult{Sqref: normSqref, Validation: dataValidationFromElem(dv), CellsAffected: sqrefCellCount(normSqref)}, nil
}

// DeleteDataValidation removes a data validation rule by sqref.
func DeleteDataValidation(req *DeleteDataValidationRequest) (*DataValidationMutationResult, error) {
	if req == nil {
		return nil, fmt.Errorf("delete data validation request is nil")
	}
	normSqref, err := NormalizeSqref(req.Range)
	if err != nil {
		return nil, err
	}
	doc, root, err := readWorksheetRoot(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	container := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "dataValidations")
	var dv *etree.Element
	if container != nil {
		dv = findDataValidationElem(container, normSqref)
	}
	if dv == nil {
		return nil, fmt.Errorf("no data validation found on %s", normSqref)
	}
	current := dataValidationFromElem(dv)
	if err := checkDataValidationGuards(current, req.HasExpectType, req.ExpectType, req.HasExpectF1, req.ExpectFormula1); err != nil {
		return nil, err
	}
	container.RemoveChild(dv)
	if len(namespaces.FindChildren(container, namespaces.NsSpreadsheetML, "dataValidation")) == 0 {
		root.RemoveChild(container)
	} else {
		updateDataValidationCount(container)
	}
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return &DataValidationMutationResult{Sqref: normSqref, Validation: current, CellsAffected: sqrefCellCount(normSqref)}, nil
}

func checkDataValidationGuards(current DataValidation, hasType bool, expectType string, hasF1 bool, expectF1 string) error {
	if hasType {
		want := NormalizeDataValidationType(expectType)
		if current.Type != want {
			return fmt.Errorf("expected type %q but found %q", want, current.Type)
		}
	}
	if hasF1 && current.Formula1 != expectF1 {
		return fmt.Errorf("expected formula1 %q but found %q", expectF1, current.Formula1)
	}
	return nil
}
