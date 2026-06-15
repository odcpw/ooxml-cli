package selectors

import "fmt"

// SelectorType represents the type of selector
type SelectorType int

const (
	// SelectorTypePlaceholderKey matches a placeholder by its canonical key
	// Examples: "title", "body:0", "pic:1"
	SelectorTypePlaceholderKey SelectorType = iota

	// SelectorTypePlaceholderType matches all placeholders of a given type
	// Syntax: @type (e.g., "@title", "@body")
	SelectorTypePlaceholderType

	// SelectorTypePlaceholderIndex matches a placeholder by its index
	// Syntax: #idx (e.g., "#0", "#3")
	SelectorTypePlaceholderIndex

	// SelectorTypeShapeName matches a shape by its name
	// Syntax: ~name (e.g., "~My Shape")
	SelectorTypeShapeName

	// SelectorTypeShapeID matches a shape by its ID
	// Syntax: shape:id (e.g., "shape:5", "shape:123")
	SelectorTypeShapeID

	// SelectorTypeSlideNumber matches a specific slide by 1-based number
	// Examples: "1", "5"
	SelectorTypeSlideNumber

	// SelectorTypeSlideRange matches multiple slides by range
	// Examples: "1-3", "1,3,5-7"
	SelectorTypeSlideRange

	// SelectorTypeWildcardAllPlaceholders matches all placeholders in the context
	// Syntax: @* or @all-placeholders
	SelectorTypeWildcardAllPlaceholders

	// SelectorTypeWildcardAllShapes matches all shapes in the context
	// Syntax: @all-shapes or @all-shapes-nonph (non-placeholder shapes)
	SelectorTypeWildcardAllShapes

	// SelectorTypeWildcardAllPictures matches all pictures in the context
	// Syntax: @all-pictures
	SelectorTypeWildcardAllPictures

	// SelectorTypeWildcardAllTables matches all tables in the context
	// Syntax: @all-tables
	SelectorTypeWildcardAllTables
)

// String returns the string representation of the SelectorType
func (st SelectorType) String() string {
	switch st {
	case SelectorTypePlaceholderKey:
		return "placeholder-key"
	case SelectorTypePlaceholderType:
		return "placeholder-type"
	case SelectorTypePlaceholderIndex:
		return "placeholder-index"
	case SelectorTypeShapeName:
		return "shape-name"
	case SelectorTypeShapeID:
		return "shape-id"
	case SelectorTypeSlideNumber:
		return "slide-number"
	case SelectorTypeSlideRange:
		return "slide-range"
	case SelectorTypeWildcardAllPlaceholders:
		return "wildcard-all-placeholders"
	case SelectorTypeWildcardAllShapes:
		return "wildcard-all-shapes"
	case SelectorTypeWildcardAllPictures:
		return "wildcard-all-pictures"
	case SelectorTypeWildcardAllTables:
		return "wildcard-all-tables"
	default:
		return "unknown"
	}
}

// Selector represents a parsed targeting expression
type Selector interface {
	// Type returns the selector type
	Type() SelectorType

	// String returns the string representation of the selector
	String() string
}

// PlaceholderKeySelector matches a placeholder by its canonical key
type PlaceholderKeySelector struct {
	Key string
}

func (s *PlaceholderKeySelector) Type() SelectorType {
	return SelectorTypePlaceholderKey
}

func (s *PlaceholderKeySelector) String() string {
	return s.Key
}

// PlaceholderTypeSelector matches placeholders by their canonical type/role
type PlaceholderTypeSelector struct {
	Role string
}

func (s *PlaceholderTypeSelector) Type() SelectorType {
	return SelectorTypePlaceholderType
}

func (s *PlaceholderTypeSelector) String() string {
	return "@" + s.Role
}

// PlaceholderIndexSelector matches a placeholder by its raw index (p:ph@idx)
type PlaceholderIndexSelector struct {
	Index int
}

func (s *PlaceholderIndexSelector) Type() SelectorType {
	return SelectorTypePlaceholderIndex
}

func (s *PlaceholderIndexSelector) String() string {
	return fmt.Sprintf("#%d", s.Index)
}

// ShapeNameSelector matches a shape by its name
type ShapeNameSelector struct {
	Name string
}

func (s *ShapeNameSelector) Type() SelectorType {
	return SelectorTypeShapeName
}

func (s *ShapeNameSelector) String() string {
	return "~" + s.Name
}

// ShapeIDSelector matches a shape by its ID
type ShapeIDSelector struct {
	ID int
}

func (s *ShapeIDSelector) Type() SelectorType {
	return SelectorTypeShapeID
}

func (s *ShapeIDSelector) String() string {
	return fmt.Sprintf("shape:%d", s.ID)
}

// SlideNumberSelector matches a single slide
type SlideNumberSelector struct {
	Number int // 1-based slide number
}

func (s *SlideNumberSelector) Type() SelectorType {
	return SelectorTypeSlideNumber
}

func (s *SlideNumberSelector) String() string {
	return fmt.Sprintf("%d", s.Number)
}

// SlideRangeSelector matches multiple slides
type SlideRangeSelector struct {
	Ranges []SlideRange
}

// SlideRange represents a range or individual slide number
type SlideRange struct {
	Start int // 1-based
	End   int // 1-based, inclusive; equals Start for single slide
}

func (sr SlideRange) String() string {
	if sr.Start == sr.End {
		return fmt.Sprintf("%d", sr.Start)
	}
	return fmt.Sprintf("%d-%d", sr.Start, sr.End)
}

func (s *SlideRangeSelector) Type() SelectorType {
	return SelectorTypeSlideRange
}

func (s *SlideRangeSelector) String() string {
	if len(s.Ranges) == 0 {
		return ""
	}
	if len(s.Ranges) == 1 {
		return s.Ranges[0].String()
	}
	result := s.Ranges[0].String()
	for i := 1; i < len(s.Ranges); i++ {
		result += "," + s.Ranges[i].String()
	}
	return result
}

// WildcardAllPlaceholdersSelector matches all placeholders in the context
type WildcardAllPlaceholdersSelector struct {
	// Format indicates which syntax was used: "*" for @*, "all-placeholders" for @all-placeholders
	Format string // "*" or "all-placeholders"
}

func (s *WildcardAllPlaceholdersSelector) Type() SelectorType {
	return SelectorTypeWildcardAllPlaceholders
}

func (s *WildcardAllPlaceholdersSelector) String() string {
	if s.Format == "all-placeholders" {
		return "@all-placeholders"
	}
	return "@*"
}

// WildcardAllShapesSelector matches all shapes in the context
type WildcardAllShapesSelector struct {
	ExcludePlaceholders bool // if true, only match non-placeholder shapes
}

func (s *WildcardAllShapesSelector) Type() SelectorType {
	return SelectorTypeWildcardAllShapes
}

func (s *WildcardAllShapesSelector) String() string {
	if s.ExcludePlaceholders {
		return "@all-shapes-nonph"
	}
	return "@all-shapes"
}

// WildcardAllPicturesSelector matches all pictures in the context
type WildcardAllPicturesSelector struct{}

func (s *WildcardAllPicturesSelector) Type() SelectorType {
	return SelectorTypeWildcardAllPictures
}

func (s *WildcardAllPicturesSelector) String() string {
	return "@all-pictures"
}

// WildcardAllTablesSelector matches all tables in the context
type WildcardAllTablesSelector struct{}

func (s *WildcardAllTablesSelector) Type() SelectorType {
	return SelectorTypeWildcardAllTables
}

func (s *WildcardAllTablesSelector) String() string {
	return "@all-tables"
}

// MatchResult represents the result of resolving a selector against a presentation
type MatchResult struct {
	// Matches is the list of matched shape IDs or slide numbers
	Matches []interface{}

	// NotFoundError is a user-friendly error message if no matches found
	NotFoundError string
}

// HasMatches returns true if the result contains matches
func (mr *MatchResult) HasMatches() bool {
	return len(mr.Matches) > 0
}

// IsNotFound returns true if the selector did not match anything
func (mr *MatchResult) IsNotFound() bool {
	return !mr.HasMatches() && mr.NotFoundError != ""
}

// BatchMatch represents a single match with full context (slide + shape identity)
type BatchMatch struct {
	SlideNumber int // 1-based slide number
	ShapeID     int // Shape ID within the slide
	ShapeName   string
	ShapeType   string // sp, pic, graphicFrame, grpSp, or placeholder key
}

// BatchMatchResult represents the result of batch resolving selectors across multiple slides
type BatchMatchResult struct {
	// Matches is the list of BatchMatch results
	Matches []BatchMatch

	// NotFoundError is a user-friendly error message if no matches found
	NotFoundError string
}

// HasMatches returns true if the result contains matches
func (bmr *BatchMatchResult) HasMatches() bool {
	return len(bmr.Matches) > 0
}

// IsNotFound returns true if the selector did not match anything
func (bmr *BatchMatchResult) IsNotFound() bool {
	return !bmr.HasMatches() && bmr.NotFoundError != ""
}
