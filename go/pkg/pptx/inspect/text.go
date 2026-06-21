package inspect

import (
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	ns "github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

// ExtractTextBody extracts text content and properties from a p:txBody element
func ExtractTextBody(txBody *etree.Element) *model.TextBlockInfo {
	if txBody == nil {
		return &model.TextBlockInfo{
			Paragraphs: []model.Paragraph{},
			PlainText:  "",
		}
	}

	info := &model.TextBlockInfo{
		Paragraphs: []model.Paragraph{},
	}

	// Extract body properties (a:bodyPr)
	bodyPr := xmlx.FindChild(txBody, ns.NsA, "bodyPr")
	if bodyPr != nil {
		info.BodyProperties = parseBodyProperties(bodyPr)
	}

	var plainTextParts []string

	// Iterate through paragraphs (a:p elements)
	for _, p := range xmlx.FindChildren(txBody, ns.NsA, "p") {
		paragraph := parseParagraph(p)
		info.Paragraphs = append(info.Paragraphs, paragraph)
		plainTextParts = append(plainTextParts, paragraph.Text)
	}

	// Join paragraphs with newlines
	info.PlainText = strings.Join(plainTextParts, "\n")

	return info
}

// parseBodyProperties extracts text body properties (a:bodyPr)
func parseBodyProperties(bodyPr *etree.Element) *model.TextBodyProperties {
	props := &model.TextBodyProperties{}

	// a:bodyPr attributes
	if anchor, ok := xmlx.GetAttr(bodyPr, "anchor"); ok {
		props.Anchor = anchor
	}

	if wrap, ok := xmlx.GetAttr(bodyPr, "wrap"); ok {
		props.Wrap = wrap
	}

	if vert, ok := xmlx.GetAttr(bodyPr, "vert"); ok {
		props.VerticalMode = vert
	}

	// Parse inset values (lIns, tIns, rIns, bIns)
	if lIns, ok := xmlx.GetAttr(bodyPr, "lIns"); ok {
		if val, err := strconv.ParseInt(lIns, 10, 64); err == nil {
			props.LeftInset = &val
		}
	}

	if tIns, ok := xmlx.GetAttr(bodyPr, "tIns"); ok {
		if val, err := strconv.ParseInt(tIns, 10, 64); err == nil {
			props.TopInset = &val
		}
	}

	if rIns, ok := xmlx.GetAttr(bodyPr, "rIns"); ok {
		if val, err := strconv.ParseInt(rIns, 10, 64); err == nil {
			props.RightInset = &val
		}
	}

	if bIns, ok := xmlx.GetAttr(bodyPr, "bIns"); ok {
		if val, err := strconv.ParseInt(bIns, 10, 64); err == nil {
			props.BottomInset = &val
		}
	}

	// Parse autofit settings
	if xmlx.FindChild(bodyPr, ns.NsA, "noAutofit") != nil {
		props.AutofitType = "noAutofit"
	} else if xmlx.FindChild(bodyPr, ns.NsA, "normAutofit") != nil {
		props.AutofitType = "normAutofit"
	} else if xmlx.FindChild(bodyPr, ns.NsA, "spAutoFit") != nil {
		props.AutofitType = "spAutoFit"
	}

	return props
}

// parseParagraph extracts text and properties from a single paragraph element
func parseParagraph(p *etree.Element) model.Paragraph {
	paragraph := model.Paragraph{
		Runs: []interface{}{},
	}

	// Extract paragraph properties (a:pPr)
	pPr := xmlx.FindChild(p, ns.NsA, "pPr")
	if pPr != nil {
		paragraph.Properties = parseParagraphProperties(pPr)
		// Also set Level on paragraph for backward compatibility
		if paragraph.Properties != nil && paragraph.Properties.Level != nil {
			paragraph.Level = paragraph.Properties.Level
		}
	}

	var textParts []string

	// Process all child elements in order
	for _, child := range p.ChildElements() {
		localName := getLocalName(child.Tag)

		switch localName {
		case "r": // Text run
			run, text := parseRun(child)
			paragraph.Runs = append(paragraph.Runs, run)
			if text != "" {
				textParts = append(textParts, text)
			}
		case "br": // Line break
			textParts = append(textParts, "\n")
			brk := &model.Break{Type: "break"}
			paragraph.Runs = append(paragraph.Runs, brk)
		case "tab": // Tab
			textParts = append(textParts, "\t")
			tab := &model.Tab{Type: "tab"}
			paragraph.Runs = append(paragraph.Runs, tab)
		case "fld": // Field
			fldText := extractFieldText(child)
			if fldText != "" {
				textParts = append(textParts, fldText)
				fld := &model.Field{
					Type: "field",
					Text: fldText,
				}
				paragraph.Runs = append(paragraph.Runs, fld)
			}
		case "pPr": // Already handled above
		case "endParaRPr": // End paragraph run properties - skip
		}
	}

	paragraph.Text = strings.Join(textParts, "")
	return paragraph
}

// parseParagraphProperties extracts properties from a:pPr element
func parseParagraphProperties(pPr *etree.Element) *model.ParagraphProperties {
	props := &model.ParagraphProperties{}

	// Level attribute
	if level, ok := xmlx.GetAttr(pPr, "lvl"); ok {
		if val, err := strconv.Atoi(level); err == nil {
			val32 := int32(val)
			props.Level = &val32
		}
	}

	// Alignment
	if algn, ok := xmlx.GetAttr(pPr, "algn"); ok {
		props.Alignment = algn
	}

	// Parse spacing (a:spcBef, a:spcAft, a:lnSpc)
	spcBef := xmlx.FindChild(pPr, ns.NsA, "spcBef")
	if spcBef != nil {
		if val := extractSpacing(spcBef); val != nil {
			props.SpaceBefore = val
		}
	}

	spcAft := xmlx.FindChild(pPr, ns.NsA, "spcAft")
	if spcAft != nil {
		if val := extractSpacing(spcAft); val != nil {
			props.SpaceAfter = val
		}
	}

	lnSpc := xmlx.FindChild(pPr, ns.NsA, "lnSpc")
	if lnSpc != nil {
		if val := extractSpacing(lnSpc); val != nil {
			props.LineSpacing = val
		}
	}

	// Parse bullet settings (a:buNone, a:buChar, a:buAutoNum, etc.)
	if xmlx.FindChild(pPr, ns.NsA, "buNone") != nil {
		props.BulletMode = "buNone"
	} else if buChar := xmlx.FindChild(pPr, ns.NsA, "buChar"); buChar != nil {
		props.BulletMode = "buChar"
		if char, ok := xmlx.GetAttr(buChar, "char"); ok {
			props.BulletCharacter = char
		}
	} else if buAutoNum := xmlx.FindChild(pPr, ns.NsA, "buAutoNum"); buAutoNum != nil {
		props.BulletMode = "buAutoNum"
		if scheme, ok := xmlx.GetAttr(buAutoNum, "type"); ok {
			props.AutoNumberingScheme = scheme
		}
	}

	// Parse default run properties (a:defRPr)
	defRPr := xmlx.FindChild(pPr, ns.NsA, "defRPr")
	if defRPr != nil {
		props.DefaultRunProps = parseRunProperties(defRPr)
	}

	return props
}

// parseRun extracts text and properties from a single text run element
func parseRun(r *etree.Element) (model.TextRun, string) {
	run := model.TextRun{}

	// Find the text element within the run (a:t)
	t := xmlx.FindChild(r, ns.NsA, "t")
	if t != nil {
		run.Text = t.Text()
	}

	// Parse run properties (a:rPr) if present
	rPr := xmlx.FindChild(r, ns.NsA, "rPr")
	if rPr != nil {
		run.Properties = parseRunProperties(rPr)
	}

	return run, run.Text
}

// parseRunProperties extracts properties from a:rPr element
func parseRunProperties(rPr *etree.Element) *model.RunProperties {
	props := &model.RunProperties{}

	// Boolean flags
	if _, ok := xmlx.GetAttr(rPr, "b"); ok {
		b := true
		props.Bold = &b
	}

	if _, ok := xmlx.GetAttr(rPr, "i"); ok {
		i := true
		props.Italic = &i
	}

	// Underline
	if u, ok := xmlx.GetAttr(rPr, "u"); ok {
		props.Underline = u
	}

	// Strike
	if strike, ok := xmlx.GetAttr(rPr, "strike"); ok {
		props.Strike = strike
	}

	// Font family (a:latin, a:ea, or a:cs)
	if latin := xmlx.FindChild(rPr, ns.NsA, "latin"); latin != nil {
		if typeface, ok := xmlx.GetAttr(latin, "typeface"); ok {
			props.FontFamily = typeface
		}
	} else if ea := xmlx.FindChild(rPr, ns.NsA, "ea"); ea != nil {
		if typeface, ok := xmlx.GetAttr(ea, "typeface"); ok {
			props.FontFamily = typeface
		}
	} else if cs := xmlx.FindChild(rPr, ns.NsA, "cs"); cs != nil {
		if typeface, ok := xmlx.GetAttr(cs, "typeface"); ok {
			props.FontFamily = typeface
		}
	}

	// Font size (in hundredths of a point, convert to points)
	if sz, ok := xmlx.GetAttr(rPr, "sz"); ok {
		if val, err := strconv.ParseFloat(sz, 64); err == nil {
			// Convert from hundredths of a point to points (e.g., 2400 = 24.0pt)
			fontSize := val / 100.0
			props.FontSize = &fontSize
		}
	}

	// Baseline (sup/sub)
	if baseline, ok := xmlx.GetAttr(rPr, "baseline"); ok {
		if baseline != "0" {
			props.Baseline = baseline
		}
	}

	// Language
	if lang, ok := xmlx.GetAttr(rPr, "lang"); ok {
		props.Language = lang
	}

	// Solid fill color
	if solidFill := xmlx.FindChild(rPr, ns.NsA, "solidFill"); solidFill != nil {
		props.SolidColor = extractSolidColor(solidFill)
	}

	return props
}

// extractSolidColor extracts color information from a:solidFill element
func extractSolidColor(solidFill *etree.Element) *model.SolidColor {
	color := &model.SolidColor{}

	// Check for srgbClr (standard RGB color)
	if srgbClr := xmlx.FindChild(solidFill, ns.NsA, "srgbClr"); srgbClr != nil {
		color.Type = "srgbClr"
		if val, ok := xmlx.GetAttr(srgbClr, "val"); ok {
			color.Value = val
		}
	} else if schemeClr := xmlx.FindChild(solidFill, ns.NsA, "schemeClr"); schemeClr != nil {
		// Theme color
		color.Type = "schemeClr"
		if val, ok := xmlx.GetAttr(schemeClr, "val"); ok {
			color.Value = val
		}
	}

	return color
}

// parseSolidColor extracts color information from a:solidFill
func parseSolidColor(solidFill *etree.Element) *model.SolidColor {
	color := &model.SolidColor{}

	// Check for srgbClr (standard RGB color)
	if srgbClr := xmlx.FindChild(solidFill, ns.NsA, "srgbClr"); srgbClr != nil {
		color.Type = "srgbClr"
		if val, ok := xmlx.GetAttr(srgbClr, "val"); ok {
			color.Value = val
		}
		// Parse alpha if present
		if alpha := xmlx.FindChild(srgbClr, ns.NsA, "alpha"); alpha != nil {
			if a, ok := xmlx.GetAttr(alpha, "val"); ok {
				if f, err := strconv.ParseFloat(a, 64); err == nil {
					alphaVal := f / 100000.0 // Convert from internal format to 0-1
					color.Alpha = &alphaVal
				}
			}
		}
	} else if schemeClr := xmlx.FindChild(solidFill, ns.NsA, "schemeClr"); schemeClr != nil {
		color.Type = "schemeClr"
		if val, ok := xmlx.GetAttr(schemeClr, "val"); ok {
			color.Value = val
		}
		// Parse tint/shade
		if tint := xmlx.FindChild(schemeClr, ns.NsA, "tint"); tint != nil {
			if t, ok := xmlx.GetAttr(tint, "val"); ok {
				if f, err := strconv.ParseFloat(t, 64); err == nil {
					tintVal := f / 100000.0
					color.Tint = &tintVal
				}
			}
		}
		if shade := xmlx.FindChild(schemeClr, ns.NsA, "shade"); shade != nil {
			if s, ok := xmlx.GetAttr(shade, "val"); ok {
				if f, err := strconv.ParseFloat(s, 64); err == nil {
					shadeVal := f / 100000.0
					color.Shade = &shadeVal
				}
			}
		}
	}

	return color
}

// extractSpacing extracts spacing value from a spacing element (a:spcBef, a:spcAft, a:lnSpc)
func extractSpacing(spc *etree.Element) *int64 {
	// Try spcPts (space in points)
	if spcPts := xmlx.FindChild(spc, ns.NsA, "spcPts"); spcPts != nil {
		if val, ok := xmlx.GetAttr(spcPts, "val"); ok {
			if i, err := strconv.ParseInt(val, 10, 64); err == nil {
				return &i
			}
		}
	}

	return nil
}

// extractFieldText extracts text from a field element (a:fld)
func extractFieldText(fld *etree.Element) string {
	// Find the run properties and text within the field
	for _, child := range fld.ChildElements() {
		localName := getLocalName(child.Tag)
		if localName == "t" {
			return child.Text()
		}
	}
	return ""
}

// getLocalName extracts the local name from a qualified tag
// Tags are in format "{namespace}localname" or just "localname"
func getLocalName(tag string) string {
	if len(tag) > 0 && tag[0] == '{' {
		for i := 1; i < len(tag); i++ {
			if tag[i] == '}' {
				return tag[i+1:]
			}
		}
	}
	return tag
}
