package inspect

import (
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxinspect "github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	tmpl "github.com/ooxml-cli/ooxml-cli/pkg/template"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/styles"
)

// ExtractXLSXTemplateTokens reads design tokens (theme, named cell styles, chart
// style summaries) from an XLSX/XLTX package.
//
// The theme part is DrawingML and shares the PPTX theme schema, so it reuses
// pptx/inspect.ParseTheme. Named cell styles are resolved from xl/styles.xml.
// Chart summaries reuse the shared chart.InspectStyle reader. Missing data
// yields empty lists or an omitted theme, never an error.
func ExtractXLSXTemplateTokens(session opc.PackageSession, source string) (*tmpl.TemplateTokens, error) {
	tokens := tmpl.NewTokens(tmpl.KindXLSX, source)
	xlsx := &tmpl.XLSXTokens{
		NamedCellStyles: []tmpl.NamedCellStyle{},
		ChartStyles:     []tmpl.ChartStyleSummary{},
	}

	workbook, err := ParseWorkbook(session)
	if err != nil {
		return nil, err
	}

	if workbook != nil {
		if workbook.ThemeURI != "" {
			if theme, terr := pptxinspect.ParseTheme(session, workbook.ThemeURI); terr == nil && theme != nil {
				xlsx.Theme = theme
			}
		}
		if workbook.StylesURI != "" {
			xlsx.NamedCellStyles = extractNamedCellStyles(session, workbook.StylesURI)
		}
	}

	// ChartStyles are populated by the caller (CLI) via chart.SummarizeChartStyles
	// to avoid an xlsx/inspect -> xlsx/chart import cycle.

	tokens.XLSX = xlsx
	return tokens, nil
}

// extractNamedCellStyles resolves <cellStyle> entries to their practical font,
// fill, and number-format properties via the cellStyleXfs/fonts/fills tables.
func extractNamedCellStyles(session opc.PackageSession, stylesURI string) []tmpl.NamedCellStyle {
	doc, err := session.ReadXMLPart(stylesURI)
	if err != nil || doc == nil {
		return []tmpl.NamedCellStyle{}
	}
	return namedCellStylesFromDoc(doc)
}

// namedCellStylesFromDoc resolves named cell styles from a parsed styles.xml
// document. Split out so it can be unit-tested without a package session.
func namedCellStylesFromDoc(doc *etree.Document) []tmpl.NamedCellStyle {
	out := []tmpl.NamedCellStyle{}
	if doc == nil || doc.Root() == nil {
		return out
	}
	root := doc.Root()

	fonts := childElems(findChild(root, "fonts"))
	fills := childElems(findChild(root, "fills"))
	cellStyleXfs := childElems(findChild(root, "cellStyleXfs"))
	numFmts := numFmtMap(findChild(root, "numFmts"))

	cellStyles := findChild(root, "cellStyles")
	if cellStyles == nil {
		return out
	}
	for _, cs := range childElems(cellStyles) {
		if localName(cs) != "cellStyle" {
			continue
		}
		name := cs.SelectAttrValue("name", "")
		if name == "" {
			continue
		}
		ncs := tmpl.NamedCellStyle{
			Name:    name,
			Builtin: cs.SelectAttrValue("builtinId", "") != "",
		}
		xfID, _ := strconv.Atoi(cs.SelectAttrValue("xfId", ""))
		if xfID >= 0 && xfID < len(cellStyleXfs) {
			xf := cellStyleXfs[xfID]
			applyXfToNamedStyle(&ncs, xf, fonts, fills, numFmts)
		}
		out = append(out, ncs)
	}
	return out
}

func applyXfToNamedStyle(ncs *tmpl.NamedCellStyle, xf *etree.Element, fonts, fills []*etree.Element, numFmts map[int]string) {
	if fid, ok := attrInt(xf, "fontId"); ok && fid >= 0 && fid < len(fonts) {
		applyFont(ncs, fonts[fid])
	}
	if fid, ok := attrInt(xf, "fillId"); ok && fid >= 0 && fid < len(fills) {
		ncs.FillColor = fillColor(fills[fid])
	}
	if nf, ok := attrInt(xf, "numFmtId"); ok {
		if code, found := numFmts[nf]; found {
			ncs.NumberFormatCode = code
		} else if code, found := styles.BuiltinNumberFormatCode(nf); found {
			ncs.NumberFormatCode = code
		}
	}
}

func applyFont(ncs *tmpl.NamedCellStyle, font *etree.Element) {
	if name := findChild(font, "name"); name != nil {
		ncs.FontName = name.SelectAttrValue("val", "")
	}
	if sz := findChild(font, "sz"); sz != nil {
		if v, err := strconv.ParseFloat(sz.SelectAttrValue("val", ""), 64); err == nil {
			ncs.SizePt = v
		}
	}
	if findChild(font, "b") != nil {
		ncs.Bold = true
	}
	if findChild(font, "i") != nil {
		ncs.Italic = true
	}
	if color := findChild(font, "color"); color != nil {
		ncs.Color = rgbFromColor(color)
	}
}

func fillColor(fill *etree.Element) string {
	pf := findChild(fill, "patternFill")
	if pf == nil {
		return ""
	}
	if fg := findChild(pf, "fgColor"); fg != nil {
		return rgbFromColor(fg)
	}
	return ""
}

// rgbFromColor returns a 6-hex-digit RRGGBB string from an rgb attribute,
// stripping a leading 2-digit alpha channel when present.
func rgbFromColor(color *etree.Element) string {
	rgb := color.SelectAttrValue("rgb", "")
	if rgb == "" {
		return ""
	}
	rgb = strings.ToUpper(rgb)
	if len(rgb) == 8 {
		rgb = rgb[2:]
	}
	return rgb
}

// --- small etree helpers (namespace-tolerant by local name) ---

func localName(e *etree.Element) string {
	if e == nil {
		return ""
	}
	return e.Tag
}

func findChild(parent *etree.Element, local string) *etree.Element {
	if parent == nil {
		return nil
	}
	for _, child := range parent.ChildElements() {
		if child.Tag == local {
			return child
		}
	}
	return nil
}

func childElems(parent *etree.Element) []*etree.Element {
	if parent == nil {
		return nil
	}
	return parent.ChildElements()
}

func attrInt(e *etree.Element, key string) (int, bool) {
	if e == nil {
		return 0, false
	}
	v := e.SelectAttrValue(key, "")
	if v == "" {
		return 0, false
	}
	n, err := strconv.Atoi(v)
	if err != nil {
		return 0, false
	}
	return n, true
}

func numFmtMap(numFmts *etree.Element) map[int]string {
	m := map[int]string{}
	for _, nf := range childElems(numFmts) {
		if nf.Tag != "numFmt" {
			continue
		}
		id, err := strconv.Atoi(nf.SelectAttrValue("numFmtId", ""))
		if err != nil {
			continue
		}
		m[id] = nf.SelectAttrValue("formatCode", "")
	}
	return m
}
