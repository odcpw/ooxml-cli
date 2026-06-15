package selectors

import (
	"fmt"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// ShapeContext provides information about shapes in a slide or layout
type ShapeContext interface {
	// GetShapesByName returns all shapes with the given name
	GetShapesByName(name string) []model.ShapeInfo

	// GetShapeByID returns the shape with the given ID
	GetShapeByID(id int) *model.ShapeInfo

	// GetAllShapes returns all shapes in this context
	GetAllShapes() []model.ShapeInfo

	// GetAllPictures returns all picture shapes (p:pic)
	GetAllPictures() []model.ShapeInfo

	// GetAllTables returns all table shapes (graphicFrame containing a:tbl)
	GetAllTables() []model.ShapeInfo
}

// PlaceholderContext provides information about placeholders in a slide or layout
type PlaceholderContext interface {
	// GetPlaceholderByKey returns the placeholder with the given key
	GetPlaceholderByKey(key string) *model.ShapeInfo

	// GetPlaceholdersByType returns all placeholders with the given canonical type
	GetPlaceholdersByType(role string) []model.ShapeInfo

	// GetPlaceholderByIndex returns the placeholder with the given raw index
	GetPlaceholderByIndex(idx int) *model.ShapeInfo

	// ListAllPlaceholderKeys returns all available placeholder keys in this context
	ListAllPlaceholderKeys() []string

	// GetAllPlaceholders returns all placeholders in this context
	GetAllPlaceholders() []model.ShapeInfo
}

// SlideContext provides information about a presentation's slides
type SlideContext interface {
	// GetTotalSlides returns the total number of slides in the presentation
	GetTotalSlides() int
}

// ResolutionContext combines all contexts needed for resolving selectors
type ResolutionContext struct {
	Shape       ShapeContext
	Placeholder PlaceholderContext
	Slide       SlideContext
}

// ResolveForShape resolves a selector to matching shapes within a single slide/layout
func ResolveForShape(selector Selector, ctx *ResolutionContext) *MatchResult {
	result := &MatchResult{}

	switch s := selector.(type) {
	case *PlaceholderKeySelector:
		return resolvePlaceholderKey(s, ctx)
	case *PlaceholderTypeSelector:
		return resolvePlaceholderType(s, ctx)
	case *PlaceholderIndexSelector:
		return resolvePlaceholderIndex(s, ctx)
	case *ShapeNameSelector:
		return resolveShapeName(s, ctx)
	case *ShapeIDSelector:
		return resolveShapeID(s, ctx)
	case *WildcardAllPlaceholdersSelector:
		return resolveWildcardAllPlaceholders(s, ctx)
	case *WildcardAllShapesSelector:
		return resolveWildcardAllShapes(s, ctx)
	case *WildcardAllPicturesSelector:
		return resolveWildcardAllPictures(s, ctx)
	case *WildcardAllTablesSelector:
		return resolveWildcardAllTables(s, ctx)
	default:
		result.NotFoundError = fmt.Sprintf("unsupported selector type: %T", selector)
		return result
	}
}

// PresentationContext provides information about the entire presentation
type PresentationContext interface {
	// GetSlideShapeContext returns the ShapeContext for a specific slide (1-based slide number)
	GetSlideShapeContext(slideNum int) ShapeContext

	// GetSlidePlaceholderContext returns the PlaceholderContext for a specific slide
	GetSlidePlaceholderContext(slideNum int) PlaceholderContext

	// GetTotalSlides returns the total number of slides
	GetTotalSlides() int
}

// ResolveForSlides resolves a selector to matching slide numbers
func ResolveForSlides(selector Selector, ctx *ResolutionContext) *MatchResult {
	result := &MatchResult{}

	switch s := selector.(type) {
	case *SlideNumberSelector:
		if s.Number < 1 || s.Number > ctx.Slide.GetTotalSlides() {
			result.NotFoundError = fmt.Sprintf("slide %d does not exist (total: %d)", s.Number, ctx.Slide.GetTotalSlides())
			return result
		}
		result.Matches = append(result.Matches, s.Number)
		return result

	case *SlideRangeSelector:
		totalSlides := ctx.Slide.GetTotalSlides()
		for _, r := range s.Ranges {
			start := r.Start
			if start < 1 {
				start = 1
			}
			if start > totalSlides {
				result.NotFoundError = fmt.Sprintf("slide %d does not exist (total: %d)", r.Start, totalSlides)
				return result
			}

			end := r.End
			if end > totalSlides {
				end = totalSlides
			}

			for slideNum := start; slideNum <= end; slideNum++ {
				result.Matches = append(result.Matches, slideNum)
			}
		}

		if !result.HasMatches() {
			result.NotFoundError = fmt.Sprintf("no slides match the range %s", selector.String())
		}
		return result

	default:
		// Other selector types don't match slides
		result.NotFoundError = fmt.Sprintf("selector type %s cannot be used for slide selection", selector.Type())
		return result
	}
}

// resolvePlaceholderKey resolves a placeholder by its canonical key
func resolvePlaceholderKey(s *PlaceholderKeySelector, ctx *ResolutionContext) *MatchResult {
	result := &MatchResult{}
	shape := ctx.Placeholder.GetPlaceholderByKey(s.Key)
	if shape == nil {
		availableKeys := ctx.Placeholder.ListAllPlaceholderKeys()
		result.NotFoundError = formatNotFoundError("placeholder", s.Key, availableKeys)
		return result
	}
	result.Matches = append(result.Matches, shape.ID)
	return result
}

// resolvePlaceholderType resolves all placeholders of a given canonical type
func resolvePlaceholderType(s *PlaceholderTypeSelector, ctx *ResolutionContext) *MatchResult {
	result := &MatchResult{}
	shapes := ctx.Placeholder.GetPlaceholdersByType(s.Role)
	if len(shapes) == 0 {
		result.NotFoundError = fmt.Sprintf("no placeholders of type %q found", s.Role)
		return result
	}
	for _, shape := range shapes {
		result.Matches = append(result.Matches, shape.ID)
	}
	return result
}

// resolvePlaceholderIndex resolves a placeholder by its raw index
func resolvePlaceholderIndex(s *PlaceholderIndexSelector, ctx *ResolutionContext) *MatchResult {
	result := &MatchResult{}
	shape := ctx.Placeholder.GetPlaceholderByIndex(s.Index)
	if shape == nil {
		result.NotFoundError = fmt.Sprintf("no placeholder with index %d found", s.Index)
		return result
	}
	result.Matches = append(result.Matches, shape.ID)
	return result
}

// resolveShapeName resolves a shape by its name
func resolveShapeName(s *ShapeNameSelector, ctx *ResolutionContext) *MatchResult {
	result := &MatchResult{}
	shapes := ctx.Shape.GetShapesByName(s.Name)
	if len(shapes) == 0 {
		result.NotFoundError = fmt.Sprintf("no shape with name %q found", s.Name)
		return result
	}
	for _, shape := range shapes {
		result.Matches = append(result.Matches, shape.ID)
	}
	return result
}

// resolveShapeID resolves a shape by its ID
func resolveShapeID(s *ShapeIDSelector, ctx *ResolutionContext) *MatchResult {
	result := &MatchResult{}
	shape := ctx.Shape.GetShapeByID(s.ID)
	if shape == nil {
		result.NotFoundError = fmt.Sprintf("no shape with ID %d found", s.ID)
		return result
	}
	result.Matches = append(result.Matches, shape.ID)
	return result
}

// formatNotFoundError creates a user-friendly error message when a placeholder key is not found
func formatNotFoundError(targetType, key string, availableKeys []string) string {
	msg := fmt.Sprintf("no %s found with key %q", targetType, key)

	if len(availableKeys) > 0 {
		msg += ". Available keys: "
		for i, k := range availableKeys {
			if i > 0 {
				msg += ", "
			}
			msg += fmt.Sprintf("%q", k)
		}
	}

	return msg
}

// resolveWildcardAllPlaceholders resolves @* or @all-placeholders selector
func resolveWildcardAllPlaceholders(s *WildcardAllPlaceholdersSelector, ctx *ResolutionContext) *MatchResult {
	result := &MatchResult{}
	if ctx.Placeholder == nil {
		result.NotFoundError = "no placeholder context available"
		return result
	}

	keys := ctx.Placeholder.ListAllPlaceholderKeys()
	if len(keys) == 0 {
		result.NotFoundError = "no placeholders found in this slide"
		return result
	}

	for _, key := range keys {
		ph := ctx.Placeholder.GetPlaceholderByKey(key)
		if ph != nil {
			result.Matches = append(result.Matches, ph.ID)
		}
	}

	return result
}

// resolveWildcardAllShapes resolves @all-shapes or @all-shapes-nonph selector
func resolveWildcardAllShapes(s *WildcardAllShapesSelector, ctx *ResolutionContext) *MatchResult {
	result := &MatchResult{}
	if ctx.Shape == nil {
		result.NotFoundError = "no shape context available"
		return result
	}

	allShapes := ctx.Shape.GetAllShapes()
	if len(allShapes) == 0 {
		result.NotFoundError = "no shapes found in this slide"
		return result
	}

	for _, shape := range allShapes {
		if s.ExcludePlaceholders && shape.IsPlaceholder {
			continue
		}
		result.Matches = append(result.Matches, shape.ID)
	}

	if len(result.Matches) == 0 {
		result.NotFoundError = "no shapes matching the criteria found in this slide"
	}

	return result
}

// resolveWildcardAllPictures resolves @all-pictures selector
func resolveWildcardAllPictures(s *WildcardAllPicturesSelector, ctx *ResolutionContext) *MatchResult {
	result := &MatchResult{}
	if ctx.Shape == nil {
		result.NotFoundError = "no shape context available"
		return result
	}

	pictures := ctx.Shape.GetAllPictures()
	if len(pictures) == 0 {
		result.NotFoundError = "no pictures found in this slide"
		return result
	}

	for _, picture := range pictures {
		result.Matches = append(result.Matches, picture.ID)
	}

	return result
}

// resolveWildcardAllTables resolves @all-tables selector
func resolveWildcardAllTables(s *WildcardAllTablesSelector, ctx *ResolutionContext) *MatchResult {
	result := &MatchResult{}
	if ctx.Shape == nil {
		result.NotFoundError = "no shape context available"
		return result
	}

	tables := ctx.Shape.GetAllTables()
	if len(tables) == 0 {
		result.NotFoundError = "no tables found in this slide"
		return result
	}

	for _, table := range tables {
		result.Matches = append(result.Matches, table.ID)
	}

	return result
}

// ResolveBatch resolves a selector across multiple slides and returns batch matches with full context
func ResolveBatch(selector Selector, slideNums []int, presentationCtx PresentationContext) *BatchMatchResult {
	result := &BatchMatchResult{}

	// Handle slide range selectors first
	_, isSlideNum := selector.(*SlideNumberSelector)
	_, isSlideRange := selector.(*SlideRangeSelector)
	if isSlideNum || isSlideRange {
		// These are slide selectors, not shape selectors
		result.NotFoundError = "selector is a slide selector, not a shape selector. Use ResolveForSlides instead."
		return result
	}

	// Resolve selector against each slide
	for _, slideNum := range slideNums {
		if slideNum < 1 || slideNum > presentationCtx.GetTotalSlides() {
			continue
		}

		shapeCtx := presentationCtx.GetSlideShapeContext(slideNum)
		placeholderCtx := presentationCtx.GetSlidePlaceholderContext(slideNum)

		if shapeCtx == nil || placeholderCtx == nil {
			continue
		}

		// Create resolution context for this slide
		ctx := &ResolutionContext{
			Shape:       shapeCtx,
			Placeholder: placeholderCtx,
			Slide:       NewSimpleSlideContext(presentationCtx.GetTotalSlides()),
		}

		// Resolve the selector for this slide
		matchResult := ResolveForShape(selector, ctx)
		if matchResult.HasMatches() {
			for _, match := range matchResult.Matches {
				if shapeID, ok := match.(int); ok {
					// Get shape info to determine shape type
					shapeInfo := shapeCtx.GetShapeByID(shapeID)
					if shapeInfo != nil {
						shapeType := string(shapeInfo.Type)
						batchMatch := BatchMatch{
							SlideNumber: slideNum,
							ShapeID:     shapeID,
							ShapeName:   shapeInfo.Name,
							ShapeType:   shapeType,
						}
						result.Matches = append(result.Matches, batchMatch)
					}
				}
			}
		}
	}

	if !result.HasMatches() {
		result.NotFoundError = fmt.Sprintf("no matches found for selector %q across specified slides", selector.String())
	}

	return result
}

// SimpleShapeContext is a minimal implementation of ShapeContext for testing
type SimpleShapeContext struct {
	shapes   map[int]*model.ShapeInfo
	byName   map[string][]*model.ShapeInfo
	allList  []model.ShapeInfo // all shapes for GetAllShapes
	pictures []model.ShapeInfo // cached pictures
	tables   []model.ShapeInfo // cached tables
}

// NewSimpleShapeContext creates a new SimpleShapeContext
func NewSimpleShapeContext(shapes []model.ShapeInfo) *SimpleShapeContext {
	ctx := &SimpleShapeContext{
		shapes:  make(map[int]*model.ShapeInfo),
		byName:  make(map[string][]*model.ShapeInfo),
		allList: shapes,
	}

	for i := range shapes {
		shape := &shapes[i]
		ctx.shapes[shape.ID] = shape
		ctx.byName[shape.Name] = append(ctx.byName[shape.Name], shape)

		// Categorize shapes
		if shape.Type == model.ShapeTypePic {
			ctx.pictures = append(ctx.pictures, shapes[i])
		} else if shape.Type == model.ShapeTypeGraphicFrame && shape.TableInfo != nil {
			ctx.tables = append(ctx.tables, shapes[i])
		}
	}

	return ctx
}

// GetShapesByName returns all shapes with the given name
func (c *SimpleShapeContext) GetShapesByName(name string) []model.ShapeInfo {
	var result []model.ShapeInfo
	if shapes, ok := c.byName[name]; ok {
		for _, shape := range shapes {
			result = append(result, *shape)
		}
	}
	return result
}

// GetShapeByID returns the shape with the given ID
func (c *SimpleShapeContext) GetShapeByID(id int) *model.ShapeInfo {
	return c.shapes[id]
}

// GetAllShapes returns all shapes
func (c *SimpleShapeContext) GetAllShapes() []model.ShapeInfo {
	return c.allList
}

// GetAllPictures returns all picture shapes
func (c *SimpleShapeContext) GetAllPictures() []model.ShapeInfo {
	return c.pictures
}

// GetAllTables returns all table shapes
func (c *SimpleShapeContext) GetAllTables() []model.ShapeInfo {
	return c.tables
}

// SimplePlaceholderContext is a minimal implementation of PlaceholderContext for testing
type SimplePlaceholderContext struct {
	byKey   map[string]*model.ShapeInfo
	byType  map[string][]*model.ShapeInfo
	byIndex map[int]*model.ShapeInfo
	allKeys []string
	allList []model.ShapeInfo // all placeholders for GetAllPlaceholders
}

// NewSimplePlaceholderContext creates a new SimplePlaceholderContext from a list of placeholders
// and their resolved metadata
func NewSimplePlaceholderContext(
	placeholders []model.ShapeInfo,
	placeholderKeys map[int]string, // maps shape ID to placeholder key
	placeholderRoles map[int]string, // maps shape ID to canonical role
) *SimplePlaceholderContext {
	ctx := &SimplePlaceholderContext{
		byKey:   make(map[string]*model.ShapeInfo),
		byType:  make(map[string][]*model.ShapeInfo),
		byIndex: make(map[int]*model.ShapeInfo),
		allList: placeholders,
	}

	for i := range placeholders {
		ph := &placeholders[i]

		// Index by key
		if key, ok := placeholderKeys[ph.ID]; ok {
			ctx.byKey[key] = ph
			ctx.allKeys = append(ctx.allKeys, key)
		}

		// Index by type/role
		if role, ok := placeholderRoles[ph.ID]; ok {
			ctx.byType[role] = append(ctx.byType[role], ph)
		}

		// Index by raw index if it's a placeholder
		// This would require extracting the raw index from XML during resolution,
		// so for now we'll skip this in the simple context
	}

	return ctx
}

// GetPlaceholderByKey returns the placeholder with the given key
func (c *SimplePlaceholderContext) GetPlaceholderByKey(key string) *model.ShapeInfo {
	return c.byKey[key]
}

// GetPlaceholdersByType returns all placeholders with the given canonical type
func (c *SimplePlaceholderContext) GetPlaceholdersByType(role string) []model.ShapeInfo {
	var result []model.ShapeInfo
	if shapes, ok := c.byType[role]; ok {
		for _, shape := range shapes {
			result = append(result, *shape)
		}
	}
	return result
}

// GetPlaceholderByIndex returns the placeholder with the given raw index
func (c *SimplePlaceholderContext) GetPlaceholderByIndex(idx int) *model.ShapeInfo {
	return c.byIndex[idx]
}

// ListAllPlaceholderKeys returns all available placeholder keys
func (c *SimplePlaceholderContext) ListAllPlaceholderKeys() []string {
	return c.allKeys
}

// GetAllPlaceholders returns all placeholders
func (c *SimplePlaceholderContext) GetAllPlaceholders() []model.ShapeInfo {
	return c.allList
}

// SimpleSlideContext is a minimal implementation of SlideContext for testing
type SimpleSlideContext struct {
	totalSlides int
}

// NewSimpleSlideContext creates a new SimpleSlideContext
func NewSimpleSlideContext(totalSlides int) *SimpleSlideContext {
	return &SimpleSlideContext{totalSlides: totalSlides}
}

// GetTotalSlides returns the total number of slides
func (c *SimpleSlideContext) GetTotalSlides() int {
	return c.totalSlides
}

// NewResolutionContext creates a ResolutionContext from simple contexts
func NewResolutionContext(
	shapes []model.ShapeInfo,
	placeholders []model.ShapeInfo,
	placeholderKeys map[int]string,
	placeholderRoles map[int]string,
	totalSlides int,
) *ResolutionContext {
	return &ResolutionContext{
		Shape:       NewSimpleShapeContext(shapes),
		Placeholder: NewSimplePlaceholderContext(placeholders, placeholderKeys, placeholderRoles),
		Slide:       NewSimpleSlideContext(totalSlides),
	}
}
