package selectors

import (
	"fmt"
	"sort"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/handle"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/normalize"
)

// SlideSelectorPlaceholder describes placeholder metadata for a targetable slide shape.
type SlideSelectorPlaceholder struct {
	Key          string `json:"key"`
	Role         string `json:"role,omitempty"`
	Index        *int   `json:"index,omitempty"`
	LiteralType  string `json:"literalType,omitempty"`
	ResolvedType string `json:"resolvedType,omitempty"`
	TypeSource   string `json:"typeSource,omitempty"`
}

// SlideSelectorTarget describes one targetable shape on a slide and the selectors that resolve to it.
type SlideSelectorTarget struct {
	Order           int                       `json:"order"`
	ShapeID         int                       `json:"shapeId"`
	ShapeName       string                    `json:"shapeName,omitempty"`
	ShapeType       model.ShapeType           `json:"shapeType"`
	TargetKind      string                    `json:"targetKind"`
	TextCapable     bool                      `json:"textCapable"`
	TextPreview     string                    `json:"textPreview,omitempty"`
	PrimarySelector string                    `json:"primarySelector"`
	Selectors       []string                  `json:"selectors"`
	Placeholder     *SlideSelectorPlaceholder `json:"placeholder,omitempty"`
}

type placeholderSelectorMeta struct {
	info       model.PlaceholderInfo
	typeSource string
}

// SlideCatalog contains the selector surface for a specific slide.
type SlideCatalog struct {
	SlideNumber   int                   `json:"slideNumber"`
	SlideID       uint32                `json:"slideId,omitempty"`
	SlidePartURI  string                `json:"slidePartUri"`
	LayoutName    string                `json:"layoutName,omitempty"`
	LayoutPartURI string                `json:"layoutPartUri,omitempty"`
	Targets       []SlideSelectorTarget `json:"targets"`

	slideDoc           *etree.Document
	targetByShapeID    map[int]*SlideSelectorTarget
	elementByShapeID   map[int]*etree.Element
	uniqueSelectors    map[string]int
	ambiguousSelectors map[string][]int
	// slideIDCount is how many presentation slides share this catalog's native
	// p:sldId@id. A shape handle scoped to a non-unique slide id would later
	// resolve as HANDLE_AMBIGUOUS, so read surfaces must omit it.
	slideIDCount       int
	availableSelectors []string
	// ambiguousShapeIDs holds cNvPr@ids that occur MORE THAN ONCE among the
	// slide's top-level shapes. A handle naming such an id cannot be resolved to
	// a single shape, so resolution refuses (CodeAmbiguous) and surfacing omits
	// the handle. The value is the count of shapes sharing that id.
	ambiguousShapeIDs map[int]int
}

// IsShapeIDAmbiguous reports whether a cNvPr@id occurs more than once among the
// slide's top-level shapes. Surfacing uses this to avoid minting a handle that
// would mis-resolve.
func (c *SlideCatalog) IsShapeIDAmbiguous(shapeID int) bool {
	if c == nil {
		return false
	}
	return c.ambiguousShapeIDs[shapeID] > 1
}

// IsSlideIDAmbiguous reports whether this catalog's native p:sldId@id is
// duplicated in the presentation. Surfacing uses this to avoid minting shape
// handles whose slide scope cannot resolve to a single slide.
func (c *SlideCatalog) IsSlideIDAmbiguous() bool {
	if c == nil || c.SlideID == 0 {
		return false
	}
	return c.slideIDCount > 1
}

// BuildSlideCatalog parses a presentation and builds the published selector catalog for one slide.
func BuildSlideCatalog(pkg opc.PackageSession, slideNumber int) (*SlideCatalog, error) {
	if pkg == nil {
		return nil, fmt.Errorf("package session cannot be nil")
	}
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		return nil, fmt.Errorf("failed to parse presentation: %w", err)
	}
	return BuildSlideCatalogFromGraph(pkg, graph, slideNumber)
}

// BuildSlideCatalogFromGraph builds a selector catalog for one slide using a pre-parsed presentation graph.
func BuildSlideCatalogFromGraph(pkg opc.PackageSession, graph *inspect.PresentationGraph, slideNumber int) (*SlideCatalog, error) {
	if pkg == nil {
		return nil, fmt.Errorf("package session cannot be nil")
	}
	if graph == nil {
		return nil, fmt.Errorf("presentation graph cannot be nil")
	}
	if slideNumber < 1 || slideNumber > len(graph.Slides) {
		return nil, fmt.Errorf("slide %d not found (presentation has %d slides)", slideNumber, len(graph.Slides))
	}

	slideRef := graph.Slides[slideNumber-1]
	slideDoc, err := pkg.ReadXMLPart(slideRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read slide: %w", err)
	}

	var (
		layoutRoot *etree.Element
		masterRoot *etree.Element
		layoutName string
	)
	if slideRef.LayoutPartURI != "" {
		layoutDoc, err := pkg.ReadXMLPart(slideRef.LayoutPartURI)
		if err != nil {
			return nil, fmt.Errorf("failed to read layout: %w", err)
		}
		layoutRoot = layoutDoc.Root()
		for _, layout := range graph.Layouts {
			if layout.PartURI != slideRef.LayoutPartURI {
				continue
			}
			layoutName = layout.Name
			if layout.MasterPartURI != "" {
				masterDoc, err := pkg.ReadXMLPart(layout.MasterPartURI)
				if err != nil {
					return nil, fmt.Errorf("failed to read layout master: %w", err)
				}
				masterRoot = masterDoc.Root()
			}
			break
		}
	}

	slideRoot := slideDoc.Root()
	if slideRoot == nil {
		return nil, fmt.Errorf("slide root element not found")
	}
	slideSpTree := findShapeTree(slideRoot)
	if slideSpTree == nil {
		return nil, fmt.Errorf("no shape tree found on slide")
	}

	shapeInfos := inspect.EnumerateShapes(slideSpTree)
	shapeInfoByID := make(map[int]model.ShapeInfo, len(shapeInfos))
	nameCounts := make(map[string]int)
	for _, shapeInfo := range shapeInfos {
		shapeInfoByID[shapeInfo.ID] = shapeInfo
		if strings.TrimSpace(shapeInfo.Name) != "" {
			nameCounts[shapeInfo.Name]++
		}
	}

	placeholderByShapeID := buildSlidePlaceholderMetadata(slideRoot, layoutRoot, masterRoot)
	indexCounts := make(map[int]int)
	for _, meta := range placeholderByShapeID {
		if meta.info.Index >= 0 {
			indexCounts[meta.info.Index]++
		}
	}

	catalog := &SlideCatalog{
		SlideNumber:        slideNumber,
		SlideID:            slideRef.SlideID,
		SlidePartURI:       slideRef.PartURI,
		LayoutName:         layoutName,
		LayoutPartURI:      slideRef.LayoutPartURI,
		Targets:            []SlideSelectorTarget{},
		slideDoc:           slideDoc,
		targetByShapeID:    map[int]*SlideSelectorTarget{},
		elementByShapeID:   map[int]*etree.Element{},
		uniqueSelectors:    map[string]int{},
		ambiguousSelectors: map[string][]int{},
		slideIDCount:       slideIDCount(graph, slideRef.SlideID),
		ambiguousShapeIDs:  map[int]int{},
	}

	// Count cNvPr@ids across all top-level shapes BEFORE building targets, so a
	// duplicated id is known to be ambiguous regardless of which shape is visited
	// first. The per-id count is recorded; ids with count > 1 are ambiguous.
	shapeIDCounts := make(map[int]int)
	for _, shapeElem := range listTopLevelShapeElements(slideSpTree) {
		if shapeID, _ := extractShapeIdentity(shapeElem); shapeID != 0 {
			shapeIDCounts[shapeID]++
		}
	}
	for shapeID, count := range shapeIDCounts {
		if count > 1 {
			catalog.ambiguousShapeIDs[shapeID] = count
		}
	}

	candidateSelectors := make(map[int][]string)
	candidateToShapeIDs := make(map[string][]int)
	tableIndex := 0

	for order, shapeElem := range listTopLevelShapeElements(slideSpTree) {
		shapeID, shapeName := extractShapeIdentity(shapeElem)
		if shapeID == 0 {
			continue
		}
		shapeInfo, ok := shapeInfoByID[shapeID]
		if !ok {
			shapeInfo = model.ShapeInfo{ID: shapeID, Name: shapeName, Type: model.ShapeType(xmlLocalName(shapeElem.Tag))}
		}
		if shapeName == "" {
			shapeName = shapeInfo.Name
		}

		textCapable, textPreview := extractTextPreview(shapeElem)
		placeholderMeta, hasPlaceholder := placeholderByShapeID[shapeID]
		placeholder := buildSlideSelectorPlaceholder(placeholderMeta, hasPlaceholder)
		primarySelector := fmt.Sprintf("shape:%d", shapeID)
		if placeholder != nil && strings.TrimSpace(placeholder.Key) != "" {
			primarySelector = placeholder.Key
		}
		tableSelector := ""
		if shapeInfo.TableInfo != nil {
			tableIndex++
			tableSelector = fmt.Sprintf("table:%d", tableIndex)
			primarySelector = tableSelector
		}

		target := SlideSelectorTarget{
			Order:           order + 1,
			ShapeID:         shapeID,
			ShapeName:       shapeName,
			ShapeType:       shapeInfo.Type,
			TargetKind:      inferTargetKind(shapeInfo, placeholder, textCapable),
			TextCapable:     textCapable,
			TextPreview:     textPreview,
			PrimarySelector: primarySelector,
			Selectors:       []string{},
			Placeholder:     placeholder,
		}
		catalog.Targets = append(catalog.Targets, target)
		catalog.elementByShapeID[shapeID] = shapeElem

		orderedCandidates := []string{}
		addCandidate := func(value string) {
			value = strings.TrimSpace(value)
			if value == "" {
				return
			}
			for _, existing := range orderedCandidates {
				if existing == value {
					return
				}
			}
			orderedCandidates = append(orderedCandidates, value)
			candidateToShapeIDs[value] = append(candidateToShapeIDs[value], shapeID)
		}

		if placeholder != nil {
			addCandidate(placeholder.Key)
			if placeholder.Role != "" {
				addCandidate("@" + placeholder.Role)
				addCandidate(placeholder.Role)
				if placeholder.Index != nil {
					addCandidate(fmt.Sprintf("%s:%d", placeholder.Role, *placeholder.Index))
				}
			}
			if placeholder.Index != nil && indexCounts[*placeholder.Index] == 1 {
				addCandidate(fmt.Sprintf("#%d", *placeholder.Index))
			}
		}
		addCandidate(fmt.Sprintf("shape:%d", shapeID))
		if tableSelector != "" {
			addCandidate(tableSelector)
		}
		if shapeName != "" && nameCounts[shapeName] == 1 {
			addCandidate("~" + shapeName)
		}
		candidateSelectors[shapeID] = orderedCandidates
	}

	catalog.targetByShapeID = map[int]*SlideSelectorTarget{}
	for i := range catalog.Targets {
		catalog.targetByShapeID[catalog.Targets[i].ShapeID] = &catalog.Targets[i]
	}

	allPlaceholderShapeIDs := collectShapeIDsByPredicate(catalog.Targets, func(target SlideSelectorTarget) bool {
		return target.Placeholder != nil
	})
	allShapeIDs := collectShapeIDsByPredicate(catalog.Targets, func(target SlideSelectorTarget) bool {
		return true
	})
	allNonPlaceholderShapeIDs := collectShapeIDsByPredicate(catalog.Targets, func(target SlideSelectorTarget) bool {
		return target.Placeholder == nil
	})
	allPictureShapeIDs := collectShapeIDsByPredicate(catalog.Targets, func(target SlideSelectorTarget) bool {
		return target.ShapeType == model.ShapeTypePic
	})
	allTableShapeIDs := collectShapeIDsByPredicate(catalog.Targets, func(target SlideSelectorTarget) bool {
		return target.TargetKind == "table"
	})
	addSelectorGroup(candidateToShapeIDs, "@*", allPlaceholderShapeIDs)
	addSelectorGroup(candidateToShapeIDs, "@all-placeholders", allPlaceholderShapeIDs)
	addSelectorGroup(candidateToShapeIDs, "@all-shapes", allShapeIDs)
	addSelectorGroup(candidateToShapeIDs, "@all-shapes-nonph", allNonPlaceholderShapeIDs)
	addSelectorGroup(candidateToShapeIDs, "@all-pictures", allPictureShapeIDs)
	addSelectorGroup(candidateToShapeIDs, "@all-tables", allTableShapeIDs)

	for selector, shapeIDs := range candidateToShapeIDs {
		shapeIDs = uniqueInts(shapeIDs)
		if len(shapeIDs) == 1 {
			catalog.uniqueSelectors[selector] = shapeIDs[0]
			catalog.availableSelectors = append(catalog.availableSelectors, selector)
		} else if len(shapeIDs) > 1 {
			catalog.ambiguousSelectors[selector] = shapeIDs
		}
	}
	sort.Strings(catalog.availableSelectors)

	for shapeID, target := range catalog.targetByShapeID {
		for _, selector := range candidateSelectors[shapeID] {
			if catalog.uniqueSelectors[selector] == shapeID {
				target.Selectors = append(target.Selectors, selector)
			}
		}
	}

	return catalog, nil
}

func slideIDCount(graph *inspect.PresentationGraph, slideID uint32) int {
	if graph == nil || slideID == 0 {
		return 0
	}
	count := 0
	for _, slideRef := range graph.Slides {
		if slideRef.SlideID == slideID {
			count++
		}
	}
	return count
}

// ResolveHandleShape resolves a parsed shape handle against this catalog. The
// handle's slide scope MUST already have been verified to match this catalog
// (callers use ResolvePPTXShapeHandle, which selects the catalog by sldId).
// Resolution SEARCHES for the native cNvPr@id within the slide, so it survives
// structural edits (insert/delete/reorder of other shapes). A missing shape id
// yields a typed CodeStale error and NEVER a positional fallback.
func (c *SlideCatalog) ResolveHandleShape(h handle.Handle) (*SlideSelectorTarget, *etree.Element, error) {
	if c == nil {
		return nil, nil, fmt.Errorf("slide catalog cannot be nil")
	}
	if h.Kind != handle.KindShape {
		return nil, nil, &handle.Error{Code: handle.CodeMalformed, Handle: handle.Format(h), Message: "expected a shape handle"}
	}
	// Refuse a duplicated cNvPr@id rather than letting the last-wins map below
	// silently mis-target one of the colliding shapes.
	if count := c.ambiguousShapeIDs[h.ShapeID]; count > 1 {
		return nil, nil, &handle.Error{
			Code:    handle.CodeAmbiguous,
			Handle:  handle.Format(h),
			Message: fmt.Sprintf("cNvPr id %d is not unique on slide sldId %d (%d shapes share it); cannot resolve to a single shape", h.ShapeID, c.SlideID, count),
		}
	}
	target, ok := c.targetByShapeID[h.ShapeID]
	if !ok || target == nil {
		return nil, nil, &handle.Error{
			Code:    handle.CodeStale,
			Handle:  handle.Format(h),
			Message: fmt.Sprintf("shape cNvPr id %d not found on slide sldId %d", h.ShapeID, c.SlideID),
		}
	}
	elem := c.elementByShapeID[h.ShapeID]
	if elem == nil {
		return nil, nil, &handle.Error{
			Code:    handle.CodeStale,
			Handle:  handle.Format(h),
			Message: fmt.Sprintf("shape cNvPr id %d resolved to a missing element", h.ShapeID),
		}
	}
	return target, elem, nil
}

// ResolveTarget resolves a selector string to exactly one published slide target.
func (c *SlideCatalog) ResolveTarget(selector string) (*SlideSelectorTarget, error) {
	if c == nil {
		return nil, fmt.Errorf("slide catalog cannot be nil")
	}
	parsed, err := Parse(selector)
	if err != nil {
		return nil, fmt.Errorf("invalid selector: %w", err)
	}
	if parsed == nil {
		return nil, fmt.Errorf("invalid selector")
	}
	switch parsed.(type) {
	case *SlideNumberSelector, *SlideRangeSelector:
		return nil, fmt.Errorf("selector %q is a slide selector; use a shape or placeholder selector", strings.TrimSpace(selector))
	}
	canonical := parsed.String()
	if shapeID, ok := c.uniqueSelectors[canonical]; ok {
		return c.targetByShapeID[shapeID], nil
	}
	if shapeIDs, ok := c.ambiguousSelectors[canonical]; ok {
		return nil, fmt.Errorf("ambiguous target: %s matches %s", canonical, c.describeTargets(shapeIDs))
	}
	return nil, fmt.Errorf("target not found: %s (available selectors: %s)", canonical, strings.Join(c.availableSelectors, ", "))
}

// ResolveTargetElement resolves a selector string to both target metadata and the backing XML element.
func (c *SlideCatalog) ResolveTargetElement(selector string) (*SlideSelectorTarget, *etree.Element, error) {
	target, err := c.ResolveTarget(selector)
	if err != nil {
		return nil, nil, err
	}
	elem := c.elementByShapeID[target.ShapeID]
	if elem == nil {
		return nil, nil, fmt.Errorf("resolved target %s is missing its shape element", target.PrimarySelector)
	}
	return target, elem, nil
}

// SlideDocument returns the mutable slide XML document used to build the catalog.
func (c *SlideCatalog) SlideDocument() *etree.Document {
	if c == nil {
		return nil
	}
	return c.slideDoc
}

func (c *SlideCatalog) describeTargets(shapeIDs []int) string {
	labels := make([]string, 0, len(shapeIDs))
	for _, shapeID := range uniqueInts(shapeIDs) {
		target := c.targetByShapeID[shapeID]
		if target == nil {
			continue
		}
		labels = append(labels, target.PrimarySelector)
	}
	sort.Strings(labels)
	return strings.Join(labels, ", ")
}

func buildSlideSelectorPlaceholder(meta placeholderSelectorMeta, ok bool) *SlideSelectorPlaceholder {
	if !ok {
		return nil
	}
	placeholder := &SlideSelectorPlaceholder{
		Key:          meta.info.Key,
		Role:         meta.info.Role,
		LiteralType:  meta.info.LiteralType,
		ResolvedType: meta.info.ResolvedType,
		TypeSource:   meta.typeSource,
	}
	if meta.info.Index >= 0 {
		idx := meta.info.Index
		placeholder.Index = &idx
	}
	return placeholder
}

func inferTargetKind(shapeInfo model.ShapeInfo, placeholder *SlideSelectorPlaceholder, textCapable bool) string {
	if placeholder != nil && placeholder.Role != "" {
		return placeholder.Role
	}
	switch shapeInfo.Type {
	case model.ShapeTypePic:
		return "picture"
	case model.ShapeTypeGraphicFrame:
		if shapeInfo.TableInfo != nil {
			return "table"
		}
		return "graphicFrame"
	case model.ShapeTypeGroup:
		return "group"
	case model.ShapeTypeSP:
		if placeholder != nil {
			return "placeholder"
		}
		if textCapable {
			return "textbox"
		}
		return "shape"
	default:
		return string(shapeInfo.Type)
	}
}

func extractTextPreview(shapeElem *etree.Element) (bool, string) {
	if xmlLocalName(shapeElem.Tag) != "sp" {
		return false, ""
	}
	txBody := findDirectChildByLocalName(shapeElem, "txBody")
	if txBody == nil {
		return false, ""
	}
	textInfo := inspect.ExtractTextBody(txBody)
	if textInfo == nil {
		return true, ""
	}
	preview := strings.TrimSpace(strings.Join(strings.Fields(textInfo.PlainText), " "))
	if len(preview) > 140 {
		preview = preview[:137] + "..."
	}
	return true, preview
}

func buildSlidePlaceholderMetadata(slideRoot, layoutRoot, masterRoot *etree.Element) map[int]placeholderSelectorMeta {
	slideShapes := slidePlaceholderShapes(slideRoot)
	layoutShapes := placeholderShapes(layoutRoot)
	masterShapes := placeholderShapes(masterRoot)
	layoutCtx := buildResolvedLayoutContext(layoutShapes, masterShapes)
	layoutPlaceholders := parseRawPlaceholders(layoutShapes)
	masterPlaceholders := parseRawPlaceholders(masterShapes)

	result := make(map[int]placeholderSelectorMeta, len(slideShapes))
	for _, slideShape := range slideShapes {
		raw := normalize.ParsePlaceholder(slideShape)
		if raw == nil {
			continue
		}
		shapeID, shapeName := extractShapeIdentity(slideShape)
		if shapeID == 0 {
			continue
		}

		resolvedType := raw.Type
		typeSource := ""
		switch {
		case resolvedType != "":
			typeSource = "slide"
		case raw.Idx >= 0:
			if inherited := findInheritedPlaceholderType(layoutPlaceholders, raw.Idx); inherited != "" {
				resolvedType = inherited
				typeSource = "layout"
			} else if inherited := findInheritedPlaceholderType(masterPlaceholders, raw.Idx); inherited != "" {
				resolvedType = inherited
				typeSource = "master"
			}
		}

		resolved := model.ResolvedPlaceholder{
			Raw: model.RawPlaceholder{
				Type:   resolvedType,
				Idx:    raw.Idx,
				Sz:     raw.Sz,
				Orient: raw.Orient,
			},
			Role:      normalize.CanonicalRole(resolvedType),
			ShapeID:   shapeID,
			ShapeName: shapeName,
		}
		info := model.PlaceholderInfo{
			Key:          normalize.GenerateKey(resolved, layoutCtx),
			Role:         resolved.Role,
			Index:        raw.Idx,
			ShapeName:    shapeName,
			LiteralType:  raw.Type,
			ResolvedType: resolvedType,
		}
		result[shapeID] = placeholderSelectorMeta{info: info, typeSource: typeSource}
	}
	return result
}

func buildResolvedLayoutContext(layoutShapes, masterShapes []*etree.Element) normalize.LayoutContext {
	layoutPlaceholders := parseRawPlaceholders(layoutShapes)
	masterPlaceholders := parseRawPlaceholders(masterShapes)
	roleCounts := make(map[string]int)
	for _, ph := range layoutPlaceholders {
		resolvedType := ph.Type
		if resolvedType == "" && ph.Idx >= 0 {
			resolvedType = findInheritedPlaceholderType(masterPlaceholders, ph.Idx)
		}
		role := normalize.CanonicalRole(resolvedType)
		if role != "" {
			roleCounts[role]++
		}
	}
	return normalize.NewSimpleLayoutContext(roleCounts)
}

func parseRawPlaceholders(shapes []*etree.Element) []*model.RawPlaceholder {
	result := make([]*model.RawPlaceholder, 0, len(shapes))
	for _, shape := range shapes {
		if ph := normalize.ParsePlaceholder(shape); ph != nil {
			result = append(result, ph)
		}
	}
	return result
}

func findInheritedPlaceholderType(placeholders []*model.RawPlaceholder, idx int) string {
	for _, ph := range placeholders {
		if ph != nil && ph.Idx == idx && ph.Type != "" {
			return ph.Type
		}
	}
	return ""
}

func slidePlaceholderShapes(slideRoot *etree.Element) []*etree.Element {
	spTree := findShapeTree(slideRoot)
	if spTree == nil {
		return nil
	}
	shapes := []*etree.Element{}
	for _, shape := range listTopLevelShapeElements(spTree) {
		if xmlLocalName(shape.Tag) != "sp" {
			continue
		}
		if normalize.ParsePlaceholder(shape) != nil {
			shapes = append(shapes, shape)
		}
	}
	return shapes
}

func placeholderShapes(root *etree.Element) []*etree.Element {
	spTree := findShapeTree(root)
	if spTree == nil {
		return nil
	}
	shapes := []*etree.Element{}
	for _, shape := range listTopLevelShapeElements(spTree) {
		if xmlLocalName(shape.Tag) != "sp" {
			continue
		}
		if normalize.ParsePlaceholder(shape) != nil {
			shapes = append(shapes, shape)
		}
	}
	return shapes
}

func findShapeTree(root *etree.Element) *etree.Element {
	if root == nil {
		return nil
	}
	if cSld := namespaces.FindChild(root, namespaces.NsP, "cSld"); cSld != nil {
		if spTree := namespaces.FindChild(cSld, namespaces.NsP, "spTree"); spTree != nil {
			return spTree
		}
	}
	for _, child := range root.ChildElements() {
		if xmlLocalName(child.Tag) == "cSld" {
			for _, grandChild := range child.ChildElements() {
				if xmlLocalName(grandChild.Tag) == "spTree" {
					return grandChild
				}
			}
		}
	}
	return root.FindElement("//p:spTree")
}

func listTopLevelShapeElements(spTree *etree.Element) []*etree.Element {
	if spTree == nil {
		return nil
	}
	result := []*etree.Element{}
	for _, child := range spTree.ChildElements() {
		switch xmlLocalName(child.Tag) {
		case "sp", "pic", "graphicFrame", "grpSp":
			result = append(result, child)
		}
	}
	return result
}

func extractShapeIdentity(shape *etree.Element) (int, string) {
	if shape == nil {
		return 0, ""
	}
	var nvPr *etree.Element
	switch xmlLocalName(shape.Tag) {
	case "sp":
		nvPr = findDirectChildByLocalName(shape, "nvSpPr")
	case "pic":
		nvPr = findDirectChildByLocalName(shape, "nvPicPr")
	case "graphicFrame":
		nvPr = findDirectChildByLocalName(shape, "nvGraphicFramePr")
	case "grpSp":
		nvPr = findDirectChildByLocalName(shape, "nvGrpSpPr")
	}
	if nvPr == nil {
		return 0, ""
	}
	cNvPr := findDirectChildByLocalName(nvPr, "cNvPr")
	if cNvPr == nil {
		return 0, ""
	}
	shapeID, _ := strconv.Atoi(cNvPr.SelectAttrValue("id", "0"))
	return shapeID, cNvPr.SelectAttrValue("name", "")
}

func findDirectChildByLocalName(elem *etree.Element, localName string) *etree.Element {
	if elem == nil {
		return nil
	}
	for _, child := range elem.ChildElements() {
		if xmlLocalName(child.Tag) == localName {
			return child
		}
	}
	return nil
}

func xmlLocalName(tag string) string {
	if tag == "" {
		return ""
	}
	if idx := strings.LastIndex(tag, "}"); idx >= 0 {
		return tag[idx+1:]
	}
	if idx := strings.Index(tag, ":"); idx >= 0 {
		return tag[idx+1:]
	}
	return tag
}

func addSelectorGroup(selectorMap map[string][]int, selector string, shapeIDs []int) {
	shapeIDs = uniqueInts(shapeIDs)
	if len(shapeIDs) == 0 {
		return
	}
	selectorMap[selector] = append(selectorMap[selector], shapeIDs...)
}

func collectShapeIDsByPredicate(targets []SlideSelectorTarget, predicate func(SlideSelectorTarget) bool) []int {
	shapeIDs := make([]int, 0, len(targets))
	for _, target := range targets {
		if predicate(target) {
			shapeIDs = append(shapeIDs, target.ShapeID)
		}
	}
	return shapeIDs
}

func uniqueInts(values []int) []int {
	seen := make(map[int]struct{}, len(values))
	result := make([]int, 0, len(values))
	for _, value := range values {
		if _, ok := seen[value]; ok {
			continue
		}
		seen[value] = struct{}{}
		result = append(result, value)
	}
	return result
}
