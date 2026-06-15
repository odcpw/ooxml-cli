package diff

import (
	"fmt"
	"sort"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

// Report is the structured semantic diff result.
type Report struct {
	SlideCountA     int            `json:"slideCountA"`
	SlideCountB     int            `json:"slideCountB"`
	SlideCountEqual bool           `json:"slideCountEqual"`
	ChangedSlides   []int          `json:"changedSlides"`
	LayoutDiffs     []LayoutDiff   `json:"layoutDiffs"`
	TextDiffs       []TextDiff     `json:"textDiffs"`
	ImageDiffs      []ImageDiff    `json:"imageDiffs"`
	FormatDiffs     []FormatDiff   `json:"formatDiffs,omitempty"`   // M12-2: Rich formatting changes
	GeometryDiffs   []GeometryDiff `json:"geometryDiffs,omitempty"` // M12-2: Shape geometry changes
}

// LayoutDiff records a per-slide layout reference change.
type LayoutDiff struct {
	Slide  int    `json:"slide"`
	Before string `json:"before"`
	After  string `json:"after"`
}

// TextDiff records a per-shape text change.
type TextDiff struct {
	Slide     int    `json:"slide"`
	ShapeKey  string `json:"shapeKey"`
	ShapeName string `json:"shapeName,omitempty"`
	Before    string `json:"before"`
	After     string `json:"after"`
}

// ImageDiff records a per-picture target/content-type change.
type ImageDiff struct {
	Slide             int    `json:"slide"`
	ShapeKey          string `json:"shapeKey"`
	ShapeName         string `json:"shapeName,omitempty"`
	BeforeTargetURI   string `json:"beforeTargetUri"`
	AfterTargetURI    string `json:"afterTargetUri"`
	BeforeContentType string `json:"beforeContentType"`
	AfterContentType  string `json:"afterContentType"`
}

// FormatDiff records a per-run rich formatting change (M12-2).
type FormatDiff struct {
	Slide      int    `json:"slide"`
	ShapeKey   string `json:"shapeKey"`
	ShapeName  string `json:"shapeName,omitempty"`
	ParagraphN int    `json:"paragraphN"` // Paragraph index (0-based)
	RunN       int    `json:"runN"`       // Run index (0-based)
	Property   string `json:"property"`   // Property changed (bold, italic, color, fontSize, alignment, bulletMode, etc.)
	Before     string `json:"before"`     // Before value (as string)
	After      string `json:"after"`      // After value (as string)
}

// GeometryDiff records a per-shape geometry change (M12-2).
type GeometryDiff struct {
	Slide     int    `json:"slide"`
	ShapeKey  string `json:"shapeKey"`
	ShapeName string `json:"shapeName,omitempty"`
	Property  string `json:"property"` // Property changed (x, y, cx, cy, rotation, flipH, flipV)
	Before    string `json:"before"`   // Before value (as string)
	After     string `json:"after"`    // After value (as string)
}

// SemanticDiff compares two PPTX packages without rendering.
func SemanticDiff(a, b opc.PackageSession) (*Report, error) {
	if a == nil || b == nil {
		return nil, fmt.Errorf("semantic diff requires two package sessions")
	}

	graphA, err := inspect.ParsePresentation(a)
	if err != nil {
		return nil, fmt.Errorf("failed to parse baseline presentation: %w", err)
	}
	graphB, err := inspect.ParsePresentation(b)
	if err != nil {
		return nil, fmt.Errorf("failed to parse candidate presentation: %w", err)
	}

	report := &Report{
		SlideCountA:     len(graphA.Slides),
		SlideCountB:     len(graphB.Slides),
		SlideCountEqual: len(graphA.Slides) == len(graphB.Slides),
		ChangedSlides:   []int{},
		LayoutDiffs:     []LayoutDiff{},
		TextDiffs:       []TextDiff{},
		ImageDiffs:      []ImageDiff{},
	}

	textA, err := extract.ExtractText(&extract.ExtractTextRequest{Session: a, Graph: graphA})
	if err != nil {
		return nil, fmt.Errorf("failed to extract baseline text: %w", err)
	}
	textB, err := extract.ExtractText(&extract.ExtractTextRequest{Session: b, Graph: graphB})
	if err != nil {
		return nil, fmt.Errorf("failed to extract candidate text: %w", err)
	}
	textIndexA := indexTextSlides(textA)
	textIndexB := indexTextSlides(textB)
	imageIndexA := indexImageSlides(a, graphA)
	imageIndexB := indexImageSlides(b, graphB)

	// M12-2: Extract formatting and geometry information
	formatIndexA := indexFormattingSlides(textA)
	formatIndexB := indexFormattingSlides(textB)
	geometryIndexA := indexGeometrySlides(a, graphA)
	geometryIndexB := indexGeometrySlides(b, graphB)

	changed := map[int]struct{}{}
	maxSlides := len(graphA.Slides)
	if len(graphB.Slides) > maxSlides {
		maxSlides = len(graphB.Slides)
	}

	for slide := 1; slide <= maxSlides; slide++ {
		if slide > len(graphA.Slides) || slide > len(graphB.Slides) {
			changed[slide] = struct{}{}
			continue
		}

		slideA := graphA.Slides[slide-1]
		slideB := graphB.Slides[slide-1]
		if slideA.LayoutPartURI != slideB.LayoutPartURI {
			report.LayoutDiffs = append(report.LayoutDiffs, LayoutDiff{Slide: slide, Before: slideA.LayoutPartURI, After: slideB.LayoutPartURI})
			changed[slide] = struct{}{}
		}

		for _, diff := range compareTextSlide(slide, textIndexA[slide], textIndexB[slide]) {
			report.TextDiffs = append(report.TextDiffs, diff)
			changed[slide] = struct{}{}
		}
		for _, diff := range compareImageSlide(slide, imageIndexA[slide], imageIndexB[slide]) {
			report.ImageDiffs = append(report.ImageDiffs, diff)
			changed[slide] = struct{}{}
		}

		// M12-2: Compare formatting and geometry
		for _, diff := range compareFormattingSlide(slide, formatIndexA[slide], formatIndexB[slide]) {
			report.FormatDiffs = append(report.FormatDiffs, diff)
			changed[slide] = struct{}{}
		}
		for _, diff := range compareGeometrySlide(slide, geometryIndexA[slide], geometryIndexB[slide]) {
			report.GeometryDiffs = append(report.GeometryDiffs, diff)
			changed[slide] = struct{}{}
		}
	}

	for slide := range changed {
		report.ChangedSlides = append(report.ChangedSlides, slide)
	}
	sort.Ints(report.ChangedSlides)
	return report, nil
}

type textShape struct {
	Key  string
	Name string
	Text string
}

type imageShape struct {
	Key         string
	Name        string
	TargetURI   string
	ContentType string
}

func indexTextSlides(result *extract.TextExtractionResult) map[int]map[string]textShape {
	indexed := map[int]map[string]textShape{}
	if result == nil {
		return indexed
	}
	for _, slide := range result.Slides {
		shapeMap := map[string]textShape{}
		for _, shape := range slide.Shapes {
			key := shape.Key
			if key == "" {
				key = fmt.Sprintf("shape:%d", shape.ID)
			}
			text := ""
			if shape.Text != nil {
				text = shape.Text.PlainText
			}
			shapeMap[key] = textShape{Key: key, Name: shape.Name, Text: text}
		}
		indexed[slide.Slide] = shapeMap
	}
	return indexed
}

func indexImageSlides(session opc.PackageSession, graph *inspect.PresentationGraph) map[int]map[string]imageShape {
	indexed := map[int]map[string]imageShape{}
	if session == nil || graph == nil {
		return indexed
	}
	for _, slideRef := range graph.Slides {
		doc, err := session.ReadXMLPart(slideRef.PartURI)
		if err != nil || doc == nil || doc.Root() == nil {
			continue
		}
		spTree := doc.Root().FindElement("{" + namespaces.NsP + "}cSld/{" + namespaces.NsP + "}spTree")
		if spTree == nil {
			spTree = doc.Root().FindElement("cSld/spTree")
		}
		images := inspect.EnumerateImageRelationships(slideRef.PartURI, session, spTree)
		shapeMap := map[string]imageShape{}
		for _, image := range images {
			key := imageKey(image)
			shapeMap[key] = imageShape{Key: key, Name: image.ShapeName, TargetURI: image.TargetURI, ContentType: image.ContentType}
		}
		indexed[slideRef.SlideNumber] = shapeMap
	}
	return indexed
}

func compareTextSlide(slide int, before, after map[string]textShape) []TextDiff {
	keys := mapKeys(before, after)
	diffs := make([]TextDiff, 0)
	for _, key := range keys {
		left := before[key]
		right := after[key]
		if left.Text == right.Text {
			continue
		}
		name := left.Name
		if name == "" {
			name = right.Name
		}
		diffs = append(diffs, TextDiff{Slide: slide, ShapeKey: key, ShapeName: name, Before: left.Text, After: right.Text})
	}
	return diffs
}

func compareImageSlide(slide int, before, after map[string]imageShape) []ImageDiff {
	keys := mapKeys(before, after)
	diffs := make([]ImageDiff, 0)
	for _, key := range keys {
		left := before[key]
		right := after[key]
		if left.TargetURI == right.TargetURI && left.ContentType == right.ContentType {
			continue
		}
		name := left.Name
		if name == "" {
			name = right.Name
		}
		diffs = append(diffs, ImageDiff{
			Slide:             slide,
			ShapeKey:          key,
			ShapeName:         name,
			BeforeTargetURI:   left.TargetURI,
			AfterTargetURI:    right.TargetURI,
			BeforeContentType: left.ContentType,
			AfterContentType:  right.ContentType,
		})
	}
	return diffs
}

func mapKeys[T any](a, b map[string]T) []string {
	set := map[string]struct{}{}
	for key := range a {
		set[key] = struct{}{}
	}
	for key := range b {
		set[key] = struct{}{}
	}
	keys := make([]string, 0, len(set))
	for key := range set {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	return keys
}

func imageKey(image model.ExtractedImageInfo) string {
	if image.ShapeID > 0 {
		return fmt.Sprintf("shape:%d", image.ShapeID)
	}
	if image.ShapeName != "" {
		return image.ShapeName
	}
	if image.RelationshipID != "" {
		return image.RelationshipID
	}
	return image.TargetURI
}

// M12-2: Formatting comparison helpers

type formatShape struct {
	Key        string
	Name       string
	Paragraphs []formatParagraph
}

type formatParagraph struct {
	Alignment  string
	BulletMode string
	Runs       []formatRun
	Properties *model.ParagraphProperties
}

type formatRun struct {
	Text       string
	Properties *model.RunProperties
}

func indexFormattingSlides(result *extract.TextExtractionResult) map[int]map[string]formatShape {
	indexed := map[int]map[string]formatShape{}
	if result == nil {
		return indexed
	}
	for _, slide := range result.Slides {
		shapeMap := map[string]formatShape{}
		for _, shape := range slide.Shapes {
			key := shape.Key
			if key == "" {
				key = fmt.Sprintf("shape:%d", shape.ID)
			}

			formatShp := formatShape{Key: key, Name: shape.Name}
			if shape.Text != nil {
				for _, para := range shape.Text.Paragraphs {
					formatPara := formatParagraph{
						Properties: para.Properties,
					}
					if para.Properties != nil {
						formatPara.Alignment = para.Properties.Alignment
						formatPara.BulletMode = para.Properties.BulletMode
					}

					// Extract formatting from runs
					for _, run := range para.Runs {
						switch r := run.(type) {
						case *model.TextRun:
							formatPara.Runs = append(formatPara.Runs, formatRun{
								Text:       r.Text,
								Properties: r.Properties,
							})
						case *model.Break:
							formatPara.Runs = append(formatPara.Runs, formatRun{
								Text:       "[break]",
								Properties: r.Properties,
							})
						case *model.Tab:
							formatPara.Runs = append(formatPara.Runs, formatRun{
								Text:       "[tab]",
								Properties: r.Properties,
							})
						case *model.Field:
							formatPara.Runs = append(formatPara.Runs, formatRun{
								Text:       r.Text,
								Properties: r.Properties,
							})
						}
					}
					formatShp.Paragraphs = append(formatShp.Paragraphs, formatPara)
				}
			}
			shapeMap[key] = formatShp
		}
		indexed[slide.Slide] = shapeMap
	}
	return indexed
}

func compareFormattingSlide(slide int, before, after map[string]formatShape) []FormatDiff {
	keys := mapKeys(before, after)
	diffs := make([]FormatDiff, 0)

	for _, key := range keys {
		leftShape := before[key]
		rightShape := after[key]

		shapeName := leftShape.Name
		if shapeName == "" {
			shapeName = rightShape.Name
		}

		// Compare paragraph-level formatting
		maxParas := len(leftShape.Paragraphs)
		if len(rightShape.Paragraphs) > maxParas {
			maxParas = len(rightShape.Paragraphs)
		}

		for pIdx := 0; pIdx < maxParas; pIdx++ {
			var leftPara, rightPara formatParagraph
			if pIdx < len(leftShape.Paragraphs) {
				leftPara = leftShape.Paragraphs[pIdx]
			}
			if pIdx < len(rightShape.Paragraphs) {
				rightPara = rightShape.Paragraphs[pIdx]
			}

			// Check paragraph alignment
			if leftPara.Alignment != rightPara.Alignment {
				diffs = append(diffs, FormatDiff{
					Slide:      slide,
					ShapeKey:   key,
					ShapeName:  shapeName,
					ParagraphN: pIdx,
					RunN:       -1,
					Property:   "alignment",
					Before:     leftPara.Alignment,
					After:      rightPara.Alignment,
				})
			}

			// Check bullet mode
			if leftPara.BulletMode != rightPara.BulletMode {
				diffs = append(diffs, FormatDiff{
					Slide:      slide,
					ShapeKey:   key,
					ShapeName:  shapeName,
					ParagraphN: pIdx,
					RunN:       -1,
					Property:   "bulletMode",
					Before:     leftPara.BulletMode,
					After:      rightPara.BulletMode,
				})
			}

			// Compare run-level formatting
			maxRuns := len(leftPara.Runs)
			if len(rightPara.Runs) > maxRuns {
				maxRuns = len(rightPara.Runs)
			}

			for rIdx := 0; rIdx < maxRuns; rIdx++ {
				var leftRun, rightRun formatRun
				if rIdx < len(leftPara.Runs) {
					leftRun = leftPara.Runs[rIdx]
				}
				if rIdx < len(rightPara.Runs) {
					rightRun = rightPara.Runs[rIdx]
				}

				leftProps := leftRun.Properties
				rightProps := rightRun.Properties

				// Check bold
				if (leftProps == nil || !*leftProps.Bold) != (rightProps == nil || !*rightProps.Bold) {
					leftBold := leftProps != nil && leftProps.Bold != nil && *leftProps.Bold
					rightBold := rightProps != nil && rightProps.Bold != nil && *rightProps.Bold
					diffs = append(diffs, FormatDiff{
						Slide:      slide,
						ShapeKey:   key,
						ShapeName:  shapeName,
						ParagraphN: pIdx,
						RunN:       rIdx,
						Property:   "bold",
						Before:     fmt.Sprintf("%v", leftBold),
						After:      fmt.Sprintf("%v", rightBold),
					})
				}

				// Check italic
				if (leftProps == nil || !*leftProps.Italic) != (rightProps == nil || !*rightProps.Italic) {
					leftItalic := leftProps != nil && leftProps.Italic != nil && *leftProps.Italic
					rightItalic := rightProps != nil && rightProps.Italic != nil && *rightProps.Italic
					diffs = append(diffs, FormatDiff{
						Slide:      slide,
						ShapeKey:   key,
						ShapeName:  shapeName,
						ParagraphN: pIdx,
						RunN:       rIdx,
						Property:   "italic",
						Before:     fmt.Sprintf("%v", leftItalic),
						After:      fmt.Sprintf("%v", rightItalic),
					})
				}

				// Check color
				leftColor := ""
				if leftProps != nil {
					leftColor = leftProps.Color
				}
				rightColor := ""
				if rightProps != nil {
					rightColor = rightProps.Color
				}
				if leftColor != rightColor {
					diffs = append(diffs, FormatDiff{
						Slide:      slide,
						ShapeKey:   key,
						ShapeName:  shapeName,
						ParagraphN: pIdx,
						RunN:       rIdx,
						Property:   "color",
						Before:     leftColor,
						After:      rightColor,
					})
				}

				// Check font size
				leftSize := ""
				if leftProps != nil && leftProps.FontSize != nil {
					leftSize = fmt.Sprintf("%.1f", *leftProps.FontSize)
				}
				rightSize := ""
				if rightProps != nil && rightProps.FontSize != nil {
					rightSize = fmt.Sprintf("%.1f", *rightProps.FontSize)
				}
				if leftSize != rightSize {
					diffs = append(diffs, FormatDiff{
						Slide:      slide,
						ShapeKey:   key,
						ShapeName:  shapeName,
						ParagraphN: pIdx,
						RunN:       rIdx,
						Property:   "fontSize",
						Before:     leftSize,
						After:      rightSize,
					})
				}
			}
		}
	}
	return diffs
}

// M12-2: Geometry comparison helpers

type geometryShape struct {
	Key      string
	Name     string
	Geometry *model.Geometry
}

func indexGeometrySlides(session opc.PackageSession, graph *inspect.PresentationGraph) map[int]map[string]geometryShape {
	indexed := map[int]map[string]geometryShape{}
	if session == nil || graph == nil {
		return indexed
	}
	for _, slideRef := range graph.Slides {
		doc, err := session.ReadXMLPart(slideRef.PartURI)
		if err != nil || doc == nil || doc.Root() == nil {
			continue
		}
		spTree := doc.Root().FindElement("{" + namespaces.NsP + "}cSld/{" + namespaces.NsP + "}spTree")
		if spTree == nil {
			spTree = doc.Root().FindElement("cSld/spTree")
		}

		shapes := inspect.EnumerateShapes(spTree)
		shapeMap := map[string]geometryShape{}
		for _, shape := range shapes {
			key := shapeKey(shape)
			shapeMap[key] = geometryShape{Key: key, Name: shape.Name, Geometry: shape.Geometry}
		}
		indexed[slideRef.SlideNumber] = shapeMap
	}
	return indexed
}

func shapeKey(shape model.ShapeInfo) string {
	if shape.ID > 0 {
		return fmt.Sprintf("shape:%d", shape.ID)
	}
	if shape.Name != "" {
		return shape.Name
	}
	return string(shape.Type)
}

func compareGeometrySlide(slide int, before, after map[string]geometryShape) []GeometryDiff {
	keys := mapKeys(before, after)
	diffs := make([]GeometryDiff, 0)

	for _, key := range keys {
		leftShape := before[key]
		rightShape := after[key]

		shapeName := leftShape.Name
		if shapeName == "" {
			shapeName = rightShape.Name
		}

		leftGeom := leftShape.Geometry
		rightGeom := rightShape.Geometry

		// Handle nil geometries
		if leftGeom == nil && rightGeom == nil {
			continue
		}

		var leftX, leftY, leftCX, leftCY, leftRot int64
		var leftFlipH, leftFlipV bool
		if leftGeom != nil && leftGeom.Bounds != nil {
			leftX = leftGeom.Bounds.X
			leftY = leftGeom.Bounds.Y
			leftCX = leftGeom.Bounds.CX
			leftCY = leftGeom.Bounds.CY
			leftRot = int64(leftGeom.Rotation)
			leftFlipH = leftGeom.FlipH
			leftFlipV = leftGeom.FlipV
		}

		var rightX, rightY, rightCX, rightCY, rightRot int64
		var rightFlipH, rightFlipV bool
		if rightGeom != nil && rightGeom.Bounds != nil {
			rightX = rightGeom.Bounds.X
			rightY = rightGeom.Bounds.Y
			rightCX = rightGeom.Bounds.CX
			rightCY = rightGeom.Bounds.CY
			rightRot = int64(rightGeom.Rotation)
			rightFlipH = rightGeom.FlipH
			rightFlipV = rightGeom.FlipV
		}

		// Compare bounds
		if leftX != rightX {
			diffs = append(diffs, GeometryDiff{
				Slide:     slide,
				ShapeKey:  key,
				ShapeName: shapeName,
				Property:  "x",
				Before:    fmt.Sprintf("%d", leftX),
				After:     fmt.Sprintf("%d", rightX),
			})
		}

		if leftY != rightY {
			diffs = append(diffs, GeometryDiff{
				Slide:     slide,
				ShapeKey:  key,
				ShapeName: shapeName,
				Property:  "y",
				Before:    fmt.Sprintf("%d", leftY),
				After:     fmt.Sprintf("%d", rightY),
			})
		}

		if leftCX != rightCX {
			diffs = append(diffs, GeometryDiff{
				Slide:     slide,
				ShapeKey:  key,
				ShapeName: shapeName,
				Property:  "cx",
				Before:    fmt.Sprintf("%d", leftCX),
				After:     fmt.Sprintf("%d", rightCX),
			})
		}

		if leftCY != rightCY {
			diffs = append(diffs, GeometryDiff{
				Slide:     slide,
				ShapeKey:  key,
				ShapeName: shapeName,
				Property:  "cy",
				Before:    fmt.Sprintf("%d", leftCY),
				After:     fmt.Sprintf("%d", rightCY),
			})
		}

		// Compare rotation
		if leftRot != rightRot {
			diffs = append(diffs, GeometryDiff{
				Slide:     slide,
				ShapeKey:  key,
				ShapeName: shapeName,
				Property:  "rotation",
				Before:    fmt.Sprintf("%d", leftRot),
				After:     fmt.Sprintf("%d", rightRot),
			})
		}

		// Compare flip
		if leftFlipH != rightFlipH {
			diffs = append(diffs, GeometryDiff{
				Slide:     slide,
				ShapeKey:  key,
				ShapeName: shapeName,
				Property:  "flipH",
				Before:    fmt.Sprintf("%v", leftFlipH),
				After:     fmt.Sprintf("%v", rightFlipH),
			})
		}

		if leftFlipV != rightFlipV {
			diffs = append(diffs, GeometryDiff{
				Slide:     slide,
				ShapeKey:  key,
				ShapeName: shapeName,
				Property:  "flipV",
				Before:    fmt.Sprintf("%v", leftFlipV),
				After:     fmt.Sprintf("%v", rightFlipV),
			})
		}
	}
	return diffs
}
