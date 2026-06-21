package inspect

import (
	"sort"
	"strconv"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	tmpl "github.com/ooxml-cli/ooxml-cli/pkg/template"
)

const drawingmlNS = "http://schemas.openxmlformats.org/drawingml/2006/main"
const pmlNS = "http://schemas.openxmlformats.org/presentationml/2006/main"

// ExtractPPTXTemplateTokens reads design tokens (theme, per-master default text
// styles, table style ids) from a PPTX/POTX package.
//
// It reuses ParsePresentation for the master/layout/theme graph and ParseTheme
// for the color/font scheme. All token lists are emitted in deterministic order;
// missing data yields empty lists, not errors. Chart style summaries are left
// empty here and populated by the caller via chart.SummarizeChartStyles, to keep
// this package free of an xlsx/chart import cycle.
func ExtractPPTXTemplateTokens(session opc.PackageSession, source string) (*tmpl.TemplateTokens, error) {
	tokens := tmpl.NewTokens(tmpl.KindPPTX, source)
	pptx := &tmpl.PPTXTokens{
		DefaultTextStyles: []tmpl.DefaultTextStyle{},
		TableStyles:       []tmpl.TableStyle{},
		ChartStyles:       []tmpl.ChartStyleSummary{},
	}

	graph, err := ParsePresentation(session)
	if err != nil {
		return nil, err
	}

	if graph != nil && len(graph.Masters) > 0 {
		// Theme from the first master's theme reference.
		if theme, terr := ParseTheme(session, graph.Masters[0].ThemeURI); terr == nil && theme != nil {
			pptx.Theme = theme
		}
		// Per-master default text styles.
		for _, master := range graph.Masters {
			pptx.DefaultTextStyles = append(pptx.DefaultTextStyles, extractMasterTextStyles(session, master.PartURI)...)
		}
	}

	pptx.TableStyles = extractTableStyles(session)
	// ChartStyles are populated by the caller (CLI) via chart.SummarizeChartStyles
	// to keep this package free of an xlsx/chart import (which would form an
	// import cycle through the chart package's own test dependencies).

	tokens.PPTX = pptx
	return tokens, nil
}

// extractMasterTextStyles reads <p:txStyles>/{titleStyle,bodyStyle,otherStyle}
// level-1 defaults from a slide master.
func extractMasterTextStyles(session opc.PackageSession, masterURI string) []tmpl.DefaultTextStyle {
	out := []tmpl.DefaultTextStyle{}
	if masterURI == "" {
		return out
	}
	doc, err := session.ReadXMLPart(masterURI)
	if err != nil || doc == nil || doc.Root() == nil {
		return out
	}
	txStyles := xmlx.FindChild(doc.Root(), pmlNS, "txStyles")
	if txStyles == nil {
		return out
	}

	roles := []struct {
		elem string
		role string
	}{
		{"titleStyle", "title"},
		{"bodyStyle", "body"},
		{"otherStyle", "other"},
	}
	for _, r := range roles {
		styleElem := xmlx.FindChild(txStyles, pmlNS, r.elem)
		if styleElem == nil {
			continue
		}
		lvl1 := xmlx.FindChild(styleElem, drawingmlNS, "lvl1pPr")
		if lvl1 == nil {
			continue
		}
		defRPr := xmlx.FindChild(lvl1, drawingmlNS, "defRPr")
		if defRPr == nil {
			continue
		}
		ds := tmpl.DefaultTextStyle{MasterRef: masterURI, Role: r.role}
		if sz := defRPr.SelectAttrValue("sz", ""); sz != "" {
			if v, e := strconv.Atoi(sz); e == nil {
				ds.SizePt = float64(v) / 100.0
			}
		}
		// Font: <a:latin typeface="..."> where typeface may be a theme alias
		// (+mj-lt / +mn-lt) or a literal family name.
		if latin := xmlx.FindChild(defRPr, drawingmlNS, "latin"); latin != nil {
			tf := latin.SelectAttrValue("typeface", "")
			switch {
			case strings.HasPrefix(tf, "+mj"):
				ds.FontRef = "major"
			case strings.HasPrefix(tf, "+mn"):
				ds.FontRef = "minor"
			case tf != "":
				ds.FontName = tf
			}
		}
		// Color: <a:solidFill> with srgbClr (literal) or schemeClr (theme ref).
		if fill := xmlx.FindChild(defRPr, drawingmlNS, "solidFill"); fill != nil {
			if srgb := xmlx.FindChild(fill, drawingmlNS, "srgbClr"); srgb != nil {
				ds.Color = strings.ToUpper(srgb.SelectAttrValue("val", ""))
			} else if scheme := xmlx.FindChild(fill, drawingmlNS, "schemeClr"); scheme != nil {
				ds.ColorRef = scheme.SelectAttrValue("val", "")
			}
		}
		out = append(out, ds)
	}
	return out
}

// extractTableStyles scans slides, layouts, and masters for referenced table
// style ids and resolves names from the table-style part when present.
func extractTableStyles(session opc.PackageSession) []tmpl.TableStyle {
	out := []tmpl.TableStyle{}
	names := tableStyleNames(session)

	seen := map[string]bool{}
	ordered := []string{}
	parts := session.ListParts()
	uris := make([]string, 0, len(parts))
	for _, p := range parts {
		u := p.URI
		if strings.HasPrefix(u, "/ppt/slides/slide") ||
			strings.HasPrefix(u, "/ppt/slideLayouts/slideLayout") ||
			strings.HasPrefix(u, "/ppt/slideMasters/slideMaster") {
			if strings.HasSuffix(u, ".xml") && !strings.Contains(u, "/_rels/") {
				uris = append(uris, u)
			}
		}
	}
	sort.Strings(uris)
	for _, u := range uris {
		doc, err := session.ReadXMLPart(u)
		if err != nil || doc == nil || doc.Root() == nil {
			continue
		}
		for _, idElem := range doc.Root().FindElements("//tableStyleId") {
			id := strings.TrimSpace(idElem.Text())
			if id == "" || seen[id] {
				continue
			}
			seen[id] = true
			ordered = append(ordered, id)
		}
	}
	sort.Strings(ordered)
	for _, id := range ordered {
		out = append(out, tmpl.TableStyle{StyleID: id, Name: names[id]})
	}
	return out
}

// tableStyleNames builds an id->name map from /ppt/tableStyles.xml when present.
func tableStyleNames(session opc.PackageSession) map[string]string {
	names := map[string]string{}
	for _, p := range session.ListParts() {
		if !strings.HasPrefix(p.URI, "/ppt/tableStyles") || !strings.HasSuffix(p.URI, ".xml") {
			continue
		}
		doc, err := session.ReadXMLPart(p.URI)
		if err != nil || doc == nil || doc.Root() == nil {
			continue
		}
		for _, ts := range doc.Root().FindElements("//tblStyle") {
			id := ts.SelectAttrValue("styleId", "")
			name := ts.SelectAttrValue("styleName", "")
			if id != "" && name != "" {
				names[id] = name
			}
		}
	}
	return names
}
