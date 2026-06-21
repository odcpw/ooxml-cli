package chart

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
)

const (
	RoleName       = "name"
	RoleCategories = "categories"
	RoleValues     = "values"
	RoleXValues    = "xValues"
	RoleYValues    = "yValues"
	RoleBubbleSize = "bubbleSize"

	CacheModeAuto  = "auto"
	CacheModeClear = "clear"
	CacheModeKeep  = "keep"
)

type CachePoint struct {
	Index int
	Value string
}

type SetSeriesSourceRequest struct {
	Package           opc.PackageSession
	ChartURI          string
	SeriesNumber      int
	Role              string
	Formula           string
	CacheMode         string
	CachePoints       []CachePoint
	CacheSkipped      int
	FormatCode        string
	ExpectFormula     string
	ExpectSourceRange string
}

type SetSeriesSourceResult struct {
	PreviousFormula    string
	Formula            string
	Sheet              string
	Range              string
	RefKind            string
	CacheType          string
	CachePointCount    int
	CachePreview       []string
	CacheSkipped       int
	SiblingPointCounts map[string]int
	Warnings           []string
}

type SeriesSourceSnapshot struct {
	SeriesNumber int
	Role         string
	Formula      string
	Sheet        string
	Range        string
	RefKind      string
	CacheType    string
	PointCount   int
	Values       []string
}

type chartSourceRole struct {
	Canonical string
	Element   string
}

func NormalizeSourceRole(value string) (string, error) {
	role, ok := chartSourceRoleFor(value)
	if !ok {
		return "", fmt.Errorf("invalid chart source role %q (must be name, categories, values, xValues, yValues, or bubbleSize)", value)
	}
	return role.Canonical, nil
}

func ParseLocalRangeFormula(formula string) (string, string, bool) {
	sheet, rangeRef := splitSheetRangeFormula(formula)
	return sheet, rangeRef, sheet != "" && rangeRef != ""
}

func LocalRangeFormula(sheetName string, rangeRef address.RangeRef) string {
	return quoteFormulaSheet(sheetName) + "!" + absoluteRangeRef(rangeRef)
}

func SetSeriesSource(req *SetSeriesSourceRequest) (*SetSeriesSourceResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set chart series source request is nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if strings.TrimSpace(req.ChartURI) == "" {
		return nil, fmt.Errorf("chart URI is required")
	}
	if req.SeriesNumber < 1 {
		return nil, fmt.Errorf("series number must be >= 1")
	}
	role, ok := chartSourceRoleFor(req.Role)
	if !ok {
		return nil, fmt.Errorf("invalid chart source role %q", req.Role)
	}
	cacheMode := normalizeCacheMode(req.CacheMode)
	if cacheMode == "" {
		return nil, fmt.Errorf("invalid cache mode %q (must be auto, clear, or keep)", req.CacheMode)
	}
	formula := normalizeFormulaText(req.Formula)
	if _, _, ok := ParseLocalRangeFormula(formula); !ok {
		return nil, fmt.Errorf("chart source formula %q is not a supported local A1 range", req.Formula)
	}

	doc, err := req.Package.ReadXMLPart(req.ChartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read chart part %s: %w", req.ChartURI, err)
	}
	root := doc.Root()
	if root == nil || !isLocal(root, "chartSpace") {
		return nil, fmt.Errorf("chart part %s root element not found", req.ChartURI)
	}
	if firstDescendant(root, "pivotSource") != nil {
		return nil, fmt.Errorf("pivot-backed chart sources are not supported")
	}

	series := walkSeries(root)
	if req.SeriesNumber > len(series) {
		return nil, fmt.Errorf("series %d is out of range (1-%d)", req.SeriesNumber, len(series))
	}
	ser := series[req.SeriesNumber-1]
	roleElem := firstDirectChild(ser, role.Element)
	if roleElem == nil {
		return nil, fmt.Errorf("series %d has no %s source (available roles: %s)", req.SeriesNumber, role.Canonical, strings.Join(seriesRoles(ser), ", "))
	}
	sourceRef, refKind, err := sourceRefElement(roleElem)
	if err != nil {
		return nil, err
	}
	if refKind == "multiLvlStrRef" {
		return nil, fmt.Errorf("multi-level category sources are not supported")
	}

	formulaElem := firstDirectChild(sourceRef, "f")
	previousFormula := ""
	if formulaElem != nil {
		previousFormula = strings.TrimSpace(formulaElem.Text())
	}
	if err := checkExpectedSource(previousFormula, req.ExpectFormula, req.ExpectSourceRange); err != nil {
		return nil, err
	}
	if formulaElem == nil {
		formulaElem = newChartElement(sourceRef, "f")
		sourceRef.InsertChildAt(0, formulaElem)
	}
	formulaElem.SetText(formula)

	result := &SetSeriesSourceResult{
		PreviousFormula: previousFormula,
		Formula:         formula,
		RefKind:         refKind,
		CacheSkipped:    req.CacheSkipped,
	}
	result.Sheet, result.Range, _ = ParseLocalRangeFormula(formula)

	cacheElem := firstCacheChild(sourceRef)
	switch cacheMode {
	case CacheModeClear:
		if cacheElem != nil {
			sourceRef.RemoveChild(cacheElem)
		}
	case CacheModeKeep:
		if cacheElem != nil {
			result.CacheType = localName(cacheElem.Tag)
			result.CachePointCount = cachePointCount(cacheElem)
			result.CachePreview = cachePreview(cacheElem, 5)
		}
	case CacheModeAuto:
		if cacheElem != nil {
			sourceRef.RemoveChild(cacheElem)
		}
		cacheType := cacheTypeForRefKind(refKind)
		cacheElem = buildCacheElement(sourceRef, cacheType, req.CachePoints, req.FormatCode)
		insertAfterFormula(sourceRef, formulaElem, cacheElem)
		result.CacheType = cacheType
		result.CachePointCount = len(req.CachePoints)
		result.CachePreview = cachePointPreview(req.CachePoints, 5)
	}

	result.SiblingPointCounts = siblingPointCounts(ser)
	if editedCount, ok := result.SiblingPointCounts[role.Canonical]; ok && editedCount > 0 && comparablePointRole(role.Canonical) {
		for siblingRole, count := range result.SiblingPointCounts {
			if siblingRole == role.Canonical || !comparablePointRole(siblingRole) || count == 0 || count == editedCount {
				continue
			}
			result.Warnings = append(result.Warnings, fmt.Sprintf("%s now has %d point(s) but %s has %d; chart may misrender until related sources are updated", role.Canonical, editedCount, siblingRole, count))
		}
	}

	if err := req.Package.ReplaceXMLPart(req.ChartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace chart part %s: %w", req.ChartURI, err)
	}
	return result, nil
}

func ReadSeriesSource(session opc.PackageSession, chartURI string, seriesNumber int, roleName string) (*SeriesSourceSnapshot, error) {
	if session == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if strings.TrimSpace(chartURI) == "" {
		return nil, fmt.Errorf("chart URI is required")
	}
	if seriesNumber < 1 {
		return nil, fmt.Errorf("series number must be >= 1")
	}
	role, ok := chartSourceRoleFor(roleName)
	if !ok {
		return nil, fmt.Errorf("invalid chart source role %q", roleName)
	}
	doc, err := session.ReadXMLPart(chartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read chart part %s: %w", chartURI, err)
	}
	root := doc.Root()
	if root == nil || !isLocal(root, "chartSpace") {
		return nil, fmt.Errorf("chart part %s root element not found", chartURI)
	}
	series := walkSeries(root)
	if seriesNumber > len(series) {
		return nil, fmt.Errorf("series %d is out of range (1-%d)", seriesNumber, len(series))
	}
	ser := series[seriesNumber-1]
	roleElem := firstDirectChild(ser, role.Element)
	if roleElem == nil {
		return nil, fmt.Errorf("series %d has no %s source (available roles: %s)", seriesNumber, role.Canonical, strings.Join(seriesRoles(ser), ", "))
	}
	sourceRef, refKind, err := sourceRefElement(roleElem)
	if err != nil {
		return nil, err
	}
	if refKind == "multiLvlStrRef" {
		return nil, fmt.Errorf("multi-level category sources are not supported")
	}
	snapshot := &SeriesSourceSnapshot{
		SeriesNumber: seriesNumber,
		Role:         role.Canonical,
		RefKind:      refKind,
	}
	if formulaElem := firstDirectChild(sourceRef, "f"); formulaElem != nil {
		snapshot.Formula = strings.TrimSpace(formulaElem.Text())
		snapshot.Sheet, snapshot.Range, _ = ParseLocalRangeFormula(snapshot.Formula)
	}
	if cacheElem := firstCacheChild(sourceRef); cacheElem != nil {
		snapshot.CacheType = localName(cacheElem.Tag)
		snapshot.PointCount = cachePointCount(cacheElem)
		snapshot.Values = cacheValues(cacheElem)
	}
	return snapshot, nil
}

func chartSourceRoleFor(value string) (chartSourceRole, bool) {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case RoleName, "tx", "series-name", "seriesname":
		return chartSourceRole{Canonical: RoleName, Element: "tx"}, true
	case RoleCategories, "category", "cat", "cats":
		return chartSourceRole{Canonical: RoleCategories, Element: "cat"}, true
	case RoleValues, "value", "val", "vals":
		return chartSourceRole{Canonical: RoleValues, Element: "val"}, true
	case strings.ToLower(RoleXValues), "x", "xval", "x-val", "x-values":
		return chartSourceRole{Canonical: RoleXValues, Element: "xVal"}, true
	case strings.ToLower(RoleYValues), "y", "yval", "y-val", "y-values":
		return chartSourceRole{Canonical: RoleYValues, Element: "yVal"}, true
	case strings.ToLower(RoleBubbleSize), "bubble", "bubble-size", "bubblesize":
		return chartSourceRole{Canonical: RoleBubbleSize, Element: "bubbleSize"}, true
	default:
		return chartSourceRole{}, false
	}
}

func normalizeCacheMode(value string) string {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "", CacheModeAuto:
		return CacheModeAuto
	case CacheModeClear:
		return CacheModeClear
	case CacheModeKeep:
		return CacheModeKeep
	default:
		return ""
	}
}

func sourceRefElement(roleElem *etree.Element) (*etree.Element, string, error) {
	for _, local := range []string{"numRef", "strRef", "multiLvlStrRef"} {
		if child := firstDirectChild(roleElem, local); child != nil {
			return child, local, nil
		}
	}
	if firstDirectChild(roleElem, "v") != nil {
		return nil, "", fmt.Errorf("series source is a literal value, not a cell reference; setting literal chart sources is not supported")
	}
	return nil, "", fmt.Errorf("series source has no supported reference")
}

func checkExpectedSource(previousFormula, expectFormula, expectRange string) error {
	if strings.TrimSpace(expectFormula) != "" {
		expected := normalizeFormulaText(expectFormula)
		if normalizeFormulaText(previousFormula) != expected {
			return fmt.Errorf("chart source formula mismatch: expected %s but found %s", expected, previousFormula)
		}
	}
	if strings.TrimSpace(expectRange) != "" {
		_, currentRange, ok := ParseLocalRangeFormula(previousFormula)
		if !ok {
			return fmt.Errorf("current chart source formula %q is not a supported local A1 range", previousFormula)
		}
		expectedRange, err := address.NormalizeRange(expectRange)
		if err != nil {
			return fmt.Errorf("invalid expected source range %q: %w", expectRange, err)
		}
		if currentRange != expectedRange {
			return fmt.Errorf("chart source range mismatch: expected %s but found %s", expectedRange, currentRange)
		}
	}
	return nil
}

func normalizeFormulaText(value string) string {
	return strings.TrimSpace(strings.TrimPrefix(strings.TrimSpace(value), "="))
}

func cacheTypeForRefKind(refKind string) string {
	if refKind == "numRef" {
		return "numCache"
	}
	return "strCache"
}

func comparablePointRole(role string) bool {
	return role != RoleName
}

func buildCacheElement(parent *etree.Element, cacheType string, points []CachePoint, formatCode string) *etree.Element {
	cache := newChartElement(parent, cacheType)
	if cacheType == "numCache" {
		code := strings.TrimSpace(formatCode)
		if code == "" {
			code = "General"
		}
		format := newChartElement(cache, "formatCode")
		format.SetText(code)
		cache.AddChild(format)
	}
	ptCount := newChartElement(cache, "ptCount")
	ptCount.CreateAttr("val", strconv.Itoa(len(points)))
	cache.AddChild(ptCount)
	for idx, point := range points {
		pointIdx := point.Index
		if pointIdx < 0 {
			pointIdx = idx
		}
		pt := newChartElement(cache, "pt")
		pt.CreateAttr("idx", strconv.Itoa(pointIdx))
		v := newChartElement(pt, "v")
		v.SetText(point.Value)
		pt.AddChild(v)
		cache.AddChild(pt)
	}
	return cache
}

func insertAfterFormula(parent, formulaElem, cacheElem *etree.Element) {
	index := formulaElem.Index()
	if index < 0 {
		parent.AddChild(cacheElem)
		return
	}
	parent.InsertChildAt(index+1, cacheElem)
}

func siblingPointCounts(ser *etree.Element) map[string]int {
	result := map[string]int{}
	for _, roleName := range []string{RoleName, RoleCategories, RoleValues, RoleXValues, RoleYValues, RoleBubbleSize} {
		role, _ := chartSourceRoleFor(roleName)
		roleElem := firstDirectChild(ser, role.Element)
		if roleElem == nil {
			continue
		}
		sourceRef, _, err := sourceRefElement(roleElem)
		if err != nil {
			continue
		}
		cache := firstCacheChild(sourceRef)
		if cache == nil {
			continue
		}
		result[role.Canonical] = cachePointCount(cache)
	}
	return result
}

func cachePointCount(cache *etree.Element) int {
	if cache == nil {
		return 0
	}
	if ptCount := firstDirectChild(cache, "ptCount"); ptCount != nil {
		return parseAttrInt(ptCount, "val")
	}
	return len(descendants(cache, "pt"))
}

func cachePreview(cache *etree.Element, limit int) []string {
	if cache == nil || limit <= 0 {
		return nil
	}
	var preview []string
	for _, pt := range descendants(cache, "pt") {
		if len(preview) >= limit {
			break
		}
		if value := firstDirectChild(pt, "v"); value != nil {
			preview = append(preview, value.Text())
		}
	}
	return preview
}

func cacheValues(cache *etree.Element) []string {
	if cache == nil {
		return nil
	}
	var values []string
	for _, pt := range descendants(cache, "pt") {
		if value := firstDirectChild(pt, "v"); value != nil {
			values = append(values, value.Text())
		}
	}
	return values
}

func cachePointPreview(points []CachePoint, limit int) []string {
	if limit <= 0 {
		return nil
	}
	capacity := len(points)
	if capacity > limit {
		capacity = limit
	}
	preview := make([]string, 0, capacity)
	for _, point := range points {
		if len(preview) >= limit {
			break
		}
		preview = append(preview, point.Value)
	}
	return preview
}

func seriesRoles(ser *etree.Element) []string {
	var roles []string
	for _, roleName := range []string{RoleName, RoleCategories, RoleValues, RoleXValues, RoleYValues, RoleBubbleSize} {
		role, _ := chartSourceRoleFor(roleName)
		if firstDirectChild(ser, role.Element) != nil {
			roles = append(roles, role.Canonical)
		}
	}
	if len(roles) == 0 {
		return []string{"none"}
	}
	return roles
}

func newChartElement(parent *etree.Element, local string) *etree.Element {
	prefix := "c"
	if parent != nil && strings.TrimSpace(parent.Space) != "" {
		prefix = parent.Space
	}
	return etree.NewElement(prefix + ":" + local)
}

func quoteFormulaSheet(sheetName string) string {
	if isSimpleFormulaSheetName(sheetName) {
		return sheetName
	}
	return "'" + strings.ReplaceAll(sheetName, "'", "''") + "'"
}

func isSimpleFormulaSheetName(sheetName string) bool {
	if sheetName == "" {
		return false
	}
	for i, r := range sheetName {
		if r >= 'A' && r <= 'Z' || r >= 'a' && r <= 'z' || r == '_' {
			continue
		}
		if i > 0 && r >= '0' && r <= '9' {
			continue
		}
		return false
	}
	return true
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
