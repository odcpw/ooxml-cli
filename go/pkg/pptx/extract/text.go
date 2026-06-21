package extract

import (
	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/normalize"
)

// ExtractedShape represents a shape with extracted text
type ExtractedShape struct {
	ID   int                  `json:"id"`
	Name string               `json:"name"`
	Type model.ShapeType      `json:"type"`
	Key  string               `json:"key"`
	Text *model.TextBlockInfo `json:"text"`
}

// ExtractedSlide represents a slide with extracted text from shapes
type ExtractedSlide struct {
	Slide  int              `json:"slide"`
	Shapes []ExtractedShape `json:"shapes"`
}

// TextExtractionResult represents the overall result of text extraction
type TextExtractionResult struct {
	File   string           `json:"file"`
	Slides []ExtractedSlide `json:"slides"`
}

// ExtractTextRequest holds parameters for text extraction
type ExtractTextRequest struct {
	Session      opc.PackageSession
	Graph        *inspect.PresentationGraph
	SlideNumbers []int
}

// ExtractText extracts text from specified slides in a presentation
// Returns a TextExtractionResult with text organized by shape with normalized placeholder keys
func ExtractText(req *ExtractTextRequest) (*TextExtractionResult, error) {
	if req == nil || req.Session == nil || req.Graph == nil {
		return &TextExtractionResult{
			Slides: []ExtractedSlide{},
		}, nil
	}

	result := &TextExtractionResult{
		Slides: []ExtractedSlide{},
	}

	// If no slide numbers specified, extract from all slides
	if len(req.SlideNumbers) == 0 {
		for _, slideRef := range req.Graph.Slides {
			req.SlideNumbers = append(req.SlideNumbers, slideRef.SlideNumber)
		}
	}

	// Extract text from each requested slide
	for _, slideNum := range req.SlideNumbers {
		if slideNum < 1 || slideNum > len(req.Graph.Slides) {
			// Skip invalid slide numbers
			continue
		}

		slideRef := req.Graph.Slides[slideNum-1]

		// Read slide XML
		slideDoc, err := req.Session.ReadXMLPart(slideRef.PartURI)
		if err != nil {
			continue
		}

		// Extract shapes from slide
		spTree := slideDoc.Root().FindElement("{" + namespaces.NsP + "}cSld/{" + namespaces.NsP + "}spTree")
		if spTree == nil {
			spTree = slideDoc.Root().FindElement("cSld/spTree")
		}

		extracted := ExtractedSlide{
			Slide:  slideNum,
			Shapes: []ExtractedShape{},
		}

		// Get layout for placeholder normalization
		layoutRef := slideRef.LayoutPartURI
		var layoutDoc *etree.Document
		var layoutShapes []*etree.Element
		var masterShapes []*etree.Element

		if layoutRef != "" {
			layoutDoc, _ = req.Session.ReadXMLPart(layoutRef)
			if layoutDoc != nil {
				layoutSpTree := layoutDoc.Root().FindElement("{" + namespaces.NsP + "}cSld/{" + namespaces.NsP + "}spTree")
				if layoutSpTree == nil {
					layoutSpTree = layoutDoc.Root().FindElement("cSld/spTree")
				}
				if layoutSpTree != nil {
					layoutShapes = layoutSpTree.FindElements("{" + namespaces.NsP + "}sp")
					if len(layoutShapes) == 0 {
						layoutShapes = layoutSpTree.FindElements("sp")
					}
				}

				// Get master from layout
				layout := findLayoutByPartURI(req.Graph, layoutRef)
				if layout != nil && layout.MasterPartURI != "" {
					masterDoc, _ := req.Session.ReadXMLPart(layout.MasterPartURI)
					if masterDoc != nil {
						masterSpTree := masterDoc.Root().FindElement("{" + namespaces.NsP + "}cSld/{" + namespaces.NsP + "}spTree")
						if masterSpTree == nil {
							masterSpTree = masterDoc.Root().FindElement("cSld/spTree")
						}
						if masterSpTree != nil {
							masterShapes = masterSpTree.FindElements("{" + namespaces.NsP + "}sp")
							if len(masterShapes) == 0 {
								masterShapes = masterSpTree.FindElements("sp")
							}
						}
					}
				}
			}
		}

		// Extract shapes with text
		if spTree != nil {
			slideShapes := spTree.FindElements("{" + namespaces.NsP + "}sp")
			if len(slideShapes) == 0 {
				slideShapes = spTree.FindElements("sp")
			}

			// Build placeholder info for key generation
			placeholderInfo := make(map[int]model.PlaceholderInfo)

			// Normalize placeholders if we have layout/master info
			if len(layoutShapes) > 0 || len(masterShapes) > 0 {
				layoutCtx := normalize.NewSimpleLayoutContext(make(map[string]int))
				if layoutDoc != nil {
					// Build layout context by counting roles
					layoutPlaceholders := []model.ResolvedPlaceholder{}
					for _, lsp := range layoutShapes {
						ph := normalize.ParsePlaceholder(lsp)
						if ph != nil {
							resolved := &model.ResolvedPlaceholder{Raw: *ph}
							resolved.Role = normalize.CanonicalRole(resolved.Raw.Type)
							layoutPlaceholders = append(layoutPlaceholders, *resolved)
						}
					}
					roleCounts := make(map[string]int)
					for _, ph := range layoutPlaceholders {
						if ph.Role != "" {
							roleCounts[ph.Role]++
						}
					}
					layoutCtx = normalize.NewSimpleLayoutContext(roleCounts)
				}

				// Normalize each slide placeholder
				normReq := &normalize.NormalizePlaceholdersRequest{
					SlideShapes:   []*etree.Element{}, // Will be filled per shape
					LayoutShapes:  layoutShapes,
					MasterShapes:  masterShapes,
					LayoutContext: layoutCtx,
				}

				// Get placeholder info for shapes
				for i, sp := range slideShapes {
					// Normalize this single shape
					normReq.SlideShapes = []*etree.Element{sp}
					normalized := normalize.NormalizePlaceholders(normReq)
					if len(normalized) > 0 {
						placeholderInfo[i] = normalized[0]
					}
				}
			}

			// Extract text from each shape
			for shapeIdx, sp := range slideShapes {
				shapeType := model.ShapeTypeSP
				shapeID := 0
				shapeName := ""

				// Get shape metadata
				nvSpPr := sp.FindElement("{" + namespaces.NsP + "}nvSpPr")
				if nvSpPr == nil {
					nvSpPr = sp.FindElement("nvSpPr")
				}
				if nvSpPr != nil {
					cNvPr := nvSpPr.FindElement("{" + namespaces.NsP + "}cNvPr")
					if cNvPr == nil {
						cNvPr = nvSpPr.FindElement("cNvPr")
					}
					if cNvPr != nil {
						if idStr := cNvPr.SelectAttrValue("id", ""); idStr != "" {
							// Parse shape ID
							for _, c := range idStr {
								if c >= '0' && c <= '9' {
									shapeID = shapeID*10 + int(c-'0')
								}
							}
						}
						shapeName = cNvPr.SelectAttrValue("name", "")
					}
				}

				// Get text body
				txBody := sp.FindElement("{" + namespaces.NsP + "}txBody")
				if txBody == nil {
					txBody = sp.FindElement("txBody")
				}

				var textInfo *model.TextBlockInfo
				if txBody != nil {
					textInfo = inspect.ExtractTextBody(txBody)
				} else {
					// Empty text for shapes without text
					textInfo = &model.TextBlockInfo{
						Paragraphs: []model.Paragraph{},
						PlainText:  "",
					}
				}

				// Get placeholder key if available
				key := ""
				if phInfo, ok := placeholderInfo[shapeIdx]; ok {
					key = phInfo.Key
				}
				if key == "" {
					// Fallback to shape name or ID
					if shapeName != "" {
						key = shapeName
					} else {
						// Use a generic key format
						key = "shape:" + string(rune(shapeID+48))
						for shapeID >= 10 {
							shapeID /= 10
						}
					}
				}

				extractedShape := ExtractedShape{
					ID:   shapeID,
					Name: shapeName,
					Type: shapeType,
					Key:  key,
					Text: textInfo,
				}

				extracted.Shapes = append(extracted.Shapes, extractedShape)
			}
		}

		result.Slides = append(result.Slides, extracted)
	}

	return result, nil
}

// findLayoutByPartURI finds a layout in the graph by its part URI
func findLayoutByPartURI(graph *inspect.PresentationGraph, uri string) *inspect.LayoutRef {
	for i, layout := range graph.Layouts {
		if layout.PartURI == uri {
			return &graph.Layouts[i]
		}
	}
	return nil
}
