package mutate

import (
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"regexp"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/handle"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
)

var (
	ErrTextOccurrencesGuardMismatch = errors.New("text occurrences guard mismatch")
	ErrTextOccurrencesNoMatches     = errors.New("text occurrences no matches")
)

type TextOccurrencesReplaceRequest struct {
	Package      opc.PackageSession
	SlideNumbers []int
	// ShapeHandle, when non-nil, confines the replacement to the ONE shape it
	// addresses (by native cNvPr@id within its slide scope) instead of every
	// target on the slide. It is authoritative: when set, SlideNumbers is ignored
	// (the handle encodes its own slide scope) and only that shape's text nodes are
	// scanned. This is what stops a find->ops shape hit from leaking a replacement
	// into sibling shapes on the same slide.
	ShapeHandle    *handle.Handle
	MatchText      string
	NewText        string
	IgnoreCase     bool
	ExpectCount    *int
	ExpectPlanHash string
	AllowZero      bool
	FailOnZero     bool
}

type TextOccurrencesReplaceResult struct {
	Operation          string                      `json:"operation"`
	MatchText          string                      `json:"matchText"`
	NewText            string                      `json:"newText"`
	IgnoreCase         bool                        `json:"ignoreCase"`
	SlidesScanned      int                         `json:"slidesScanned"`
	TargetsScanned     int                         `json:"targetsScanned"`
	TextNodesScanned   int                         `json:"textNodesScanned"`
	ChangedTargetCount int                         `json:"changedTargetCount"`
	ReplacementCount   int                         `json:"replacementCount"`
	PlanHash           string                      `json:"planHash"`
	Scope              TextOccurrencesReplaceScope `json:"scope"`
	Matches            []TextOccurrenceMatch       `json:"matches"`
}

type TextOccurrencesReplaceScope struct {
	Slides              []int  `json:"slides"`
	Text                string `json:"text"`
	SplitRunMatches     string `json:"splitRunMatches"`
	ExcludedContent     string `json:"excludedContent"`
	TableCellsIncluded  bool   `json:"tableCellsIncluded"`
	SlideShapesIncluded bool   `json:"slideShapesIncluded"`
}

type TextOccurrenceMatch struct {
	SlideNumber     int      `json:"slideNumber"`
	PartURI         string   `json:"partUri"`
	ShapeID         int      `json:"shapeId"`
	ShapeName       string   `json:"shapeName,omitempty"`
	TargetKind      string   `json:"targetKind"`
	PrimarySelector string   `json:"primarySelector"`
	Selectors       []string `json:"selectors"`
	TextNodeIndex   int      `json:"textNodeIndex"`
	MatchCount      int      `json:"matchCount"`
	BeforeText      string   `json:"beforeText"`
	AfterText       string   `json:"afterText"`
	SourceHash      string   `json:"sourceHash"`
}

type pendingTextOccurrenceReplacement struct {
	textNode *etree.Element
	after    string
}

// ReplaceTextOccurrences replaces matching text inside slide-visible a:t nodes.
// It preserves surrounding runs and table-cell formatting when the match is
// contained within one text node. Matches split across runs are reported as out
// of scope in the result contract and are not rewritten.
func ReplaceTextOccurrences(req *TextOccurrencesReplaceRequest) (*TextOccurrencesReplaceResult, error) {
	if req == nil {
		return nil, fmt.Errorf("text occurrences request cannot be nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session cannot be nil")
	}
	if req.MatchText == "" {
		return nil, fmt.Errorf("match text cannot be empty")
	}

	graph, err := inspect.ParsePresentation(req.Package)
	if err != nil {
		return nil, fmt.Errorf("failed to parse presentation: %w", err)
	}

	// Build the ordered list of (slide, catalog, targets) scan units. A shape
	// handle confines the scan to its ONE addressed shape (the handle is
	// authoritative over SlideNumbers); otherwise every target on each requested
	// slide is scanned.
	scanUnits, slideNumbers, err := buildTextOccurrenceScanUnits(req, graph)
	if err != nil {
		return nil, err
	}

	result := &TextOccurrencesReplaceResult{
		Operation:  "pptx.replace.text-occurrences",
		MatchText:  req.MatchText,
		NewText:    req.NewText,
		IgnoreCase: req.IgnoreCase,
		Scope: TextOccurrencesReplaceScope{
			Slides:              append([]int{}, slideNumbers...),
			Text:                "slide-visible text nodes under published slide targets",
			SplitRunMatches:     "not matched; only occurrences contained within one XML text node are replaced",
			ExcludedContent:     "notes, layouts, masters, comments, charts, and non-slide parts",
			TableCellsIncluded:  true,
			SlideShapesIncluded: true,
		},
		Matches: []TextOccurrenceMatch{},
	}
	if req.ShapeHandle != nil {
		result.Scope.Text = "slide-visible text nodes under a single shape target (shape-scoped)"
	}

	var pending []pendingTextOccurrenceReplacement
	changedTargets := map[string]struct{}{}
	changedSlides := map[int]*selectors.SlideCatalog{}
	var changedSlideOrder []int

	for _, unit := range scanUnits {
		slideNumber := unit.slideNumber
		catalog := unit.catalog
		result.SlidesScanned++
		for _, target := range unit.targets {
			elem := unit.elements[target.ShapeID]
			if elem == nil {
				continue
			}
			textNodes := collectDescendantTextNodes(elem)
			if len(textNodes) == 0 {
				continue
			}
			result.TargetsScanned++
			for nodeIndex, textNode := range textNodes {
				before := textNode.Text()
				result.TextNodesScanned++
				after, count := replaceTextOccurrencesInString(before, req.MatchText, req.NewText, req.IgnoreCase)
				if count == 0 {
					continue
				}
				result.ReplacementCount += count
				changedTargets[strconv.Itoa(slideNumber)+":"+strconv.Itoa(target.ShapeID)] = struct{}{}
				if _, ok := changedSlides[slideNumber]; !ok {
					changedSlideOrder = append(changedSlideOrder, slideNumber)
					changedSlides[slideNumber] = catalog
				}
				result.Matches = append(result.Matches, TextOccurrenceMatch{
					SlideNumber:     slideNumber,
					PartURI:         catalog.SlidePartURI,
					ShapeID:         target.ShapeID,
					ShapeName:       target.ShapeName,
					TargetKind:      target.TargetKind,
					PrimarySelector: target.PrimarySelector,
					Selectors:       append([]string{}, target.Selectors...),
					TextNodeIndex:   nodeIndex + 1,
					MatchCount:      count,
					BeforeText:      before,
					AfterText:       after,
					SourceHash:      sha256String(before),
				})
				pending = append(pending, pendingTextOccurrenceReplacement{
					textNode: textNode,
					after:    after,
				})
			}
		}
	}

	result.ChangedTargetCount = len(changedTargets)
	result.PlanHash = computeTextOccurrencesPlanHash(result)

	if req.ExpectCount != nil && result.ReplacementCount != *req.ExpectCount {
		return result, fmt.Errorf("%w: --expect-count is %d but planned replacements are %d", ErrTextOccurrencesGuardMismatch, *req.ExpectCount, result.ReplacementCount)
	}
	if strings.TrimSpace(req.ExpectPlanHash) != "" && result.PlanHash != strings.TrimSpace(req.ExpectPlanHash) {
		return result, fmt.Errorf("%w: --expect-plan-hash is %s but current plan hash is %s", ErrTextOccurrencesGuardMismatch, strings.TrimSpace(req.ExpectPlanHash), result.PlanHash)
	}
	if result.ReplacementCount == 0 && req.FailOnZero && !req.AllowZero {
		return result, fmt.Errorf("%w: no occurrences of match text were found", ErrTextOccurrencesNoMatches)
	}

	for _, replacement := range pending {
		if textNeedsSpacePreserve(replacement.after) {
			replacement.textNode.CreateAttr("xml:space", "preserve")
		}
		replacement.textNode.SetText(replacement.after)
	}
	for _, slideNumber := range changedSlideOrder {
		catalog := changedSlides[slideNumber]
		if err := req.Package.ReplaceXMLPart(catalog.SlidePartURI, catalog.SlideDocument()); err != nil {
			return result, fmt.Errorf("failed to save slide %d: %w", catalog.SlideNumber, err)
		}
	}

	return result, nil
}

// textOccurrenceScanUnit is one slide's catalog plus the targets to scan on it.
// For a slide-wide request this is every target on the slide; for a shape-scoped
// request it is the single resolved shape target. elements maps each target's
// cNvPr@id to its backing shape element so the scan does not re-resolve.
type textOccurrenceScanUnit struct {
	slideNumber int
	catalog     *selectors.SlideCatalog
	targets     []selectors.SlideSelectorTarget
	elements    map[int]*etree.Element
}

// buildTextOccurrenceScanUnits resolves the scan plan. With a shape handle it
// returns exactly one unit holding the single addressed shape (the handle's slide
// scope is authoritative); otherwise it returns one unit per requested slide with
// all of that slide's targets. It also returns the slide numbers actually scanned
// (for the result scope contract).
func buildTextOccurrenceScanUnits(req *TextOccurrencesReplaceRequest, graph *inspect.PresentationGraph) ([]textOccurrenceScanUnit, []int, error) {
	if req.ShapeHandle != nil {
		h := *req.ShapeHandle
		slideNumber, err := selectors.ResolveSlideNumberForHandle(graph, h)
		if err != nil {
			return nil, nil, err
		}
		catalog, err := selectors.BuildSlideCatalogFromGraph(req.Package, graph, slideNumber)
		if err != nil {
			return nil, nil, err
		}
		target, elem, err := catalog.ResolveHandleShape(h)
		if err != nil {
			return nil, nil, err
		}
		unit := textOccurrenceScanUnit{
			slideNumber: slideNumber,
			catalog:     catalog,
			targets:     []selectors.SlideSelectorTarget{*target},
			elements:    map[int]*etree.Element{target.ShapeID: elem},
		}
		return []textOccurrenceScanUnit{unit}, []int{slideNumber}, nil
	}

	slideNumbers, err := resolveTextOccurrenceSlides(req.SlideNumbers, len(graph.Slides))
	if err != nil {
		return nil, nil, err
	}
	units := make([]textOccurrenceScanUnit, 0, len(slideNumbers))
	for _, slideNumber := range slideNumbers {
		catalog, err := selectors.BuildSlideCatalogFromGraph(req.Package, graph, slideNumber)
		if err != nil {
			return nil, nil, err
		}
		elements := make(map[int]*etree.Element, len(catalog.Targets))
		for _, target := range catalog.Targets {
			_, elem, err := catalog.ResolveTargetElement(target.PrimarySelector)
			if err != nil {
				return nil, nil, err
			}
			elements[target.ShapeID] = elem
		}
		units = append(units, textOccurrenceScanUnit{
			slideNumber: slideNumber,
			catalog:     catalog,
			targets:     append([]selectors.SlideSelectorTarget{}, catalog.Targets...),
			elements:    elements,
		})
	}
	return units, slideNumbers, nil
}

func resolveTextOccurrenceSlides(slideNumbers []int, slideCount int) ([]int, error) {
	if slideCount < 1 {
		return nil, fmt.Errorf("presentation has no slides")
	}
	if len(slideNumbers) == 0 {
		result := make([]int, slideCount)
		for i := 0; i < slideCount; i++ {
			result[i] = i + 1
		}
		return result, nil
	}
	seen := map[int]struct{}{}
	result := make([]int, 0, len(slideNumbers))
	for _, slideNumber := range slideNumbers {
		if slideNumber < 1 || slideNumber > slideCount {
			return nil, fmt.Errorf("slide %d not found (presentation has %d slides)", slideNumber, slideCount)
		}
		if _, ok := seen[slideNumber]; ok {
			continue
		}
		seen[slideNumber] = struct{}{}
		result = append(result, slideNumber)
	}
	if len(result) == 0 {
		return nil, fmt.Errorf("no valid slides specified")
	}
	return result, nil
}

func collectDescendantTextNodes(elem *etree.Element) []*etree.Element {
	if elem == nil {
		return nil
	}
	var nodes []*etree.Element
	var walk func(*etree.Element)
	walk = func(current *etree.Element) {
		if localName(current.Tag) == "t" {
			nodes = append(nodes, current)
			return
		}
		for _, child := range current.ChildElements() {
			walk(child)
		}
	}
	walk(elem)
	return nodes
}

func replaceTextOccurrencesInString(text, match, replacement string, ignoreCase bool) (string, int) {
	if match == "" || text == "" {
		return text, 0
	}
	if !ignoreCase {
		count := strings.Count(text, match)
		if count == 0 {
			return text, 0
		}
		return strings.ReplaceAll(text, match, replacement), count
	}

	ranges := regexp.MustCompile("(?i)"+regexp.QuoteMeta(match)).FindAllStringIndex(text, -1)
	if len(ranges) == 0 {
		return text, 0
	}

	var builder strings.Builder
	offset := 0
	for _, matchRange := range ranges {
		builder.WriteString(text[offset:matchRange[0]])
		builder.WriteString(replacement)
		offset = matchRange[1]
	}
	builder.WriteString(text[offset:])
	return builder.String(), len(ranges)
}

func computeTextOccurrencesPlanHash(result *TextOccurrencesReplaceResult) string {
	hasher := sha256.New()
	writeHashField := func(value string) {
		hasher.Write([]byte(value))
		hasher.Write([]byte{0})
	}
	writeHashField(result.Operation)
	writeHashField(result.MatchText)
	writeHashField(result.NewText)
	writeHashField(strconv.FormatBool(result.IgnoreCase))
	for _, slide := range result.Scope.Slides {
		writeHashField(strconv.Itoa(slide))
	}
	for _, match := range result.Matches {
		writeHashField(strconv.Itoa(match.SlideNumber))
		writeHashField(match.PartURI)
		writeHashField(strconv.Itoa(match.ShapeID))
		writeHashField(match.PrimarySelector)
		writeHashField(strconv.Itoa(match.TextNodeIndex))
		writeHashField(strconv.Itoa(match.MatchCount))
		writeHashField(match.BeforeText)
		writeHashField(match.SourceHash)
	}
	return "sha256:" + hex.EncodeToString(hasher.Sum(nil))
}

func sha256String(value string) string {
	sum := sha256.Sum256([]byte(value))
	return "sha256:" + hex.EncodeToString(sum[:])
}
