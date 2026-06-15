package chart

import (
	"sort"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	tmpl "github.com/ooxml-cli/ooxml-cli/pkg/template"
)

// SummarizeChartStyles finds every chart part under uriPrefix (e.g.
// "/ppt/charts/chart" or "/xl/charts/chart") and returns a practical,
// design-transfer style summary for each, in sorted part-URI order.
//
// It is the single chart-summary entry point shared by the PPTX and XLSX
// template-token extractors. Charts that fail to parse are skipped rather than
// failing the whole extraction. The result is always non-nil.
func SummarizeChartStyles(session opc.PackageSession, uriPrefix string) []tmpl.ChartStyleSummary {
	out := []tmpl.ChartStyleSummary{}
	if session == nil {
		return out
	}
	uris := []string{}
	for _, p := range session.ListParts() {
		if strings.HasPrefix(p.URI, uriPrefix) &&
			strings.HasSuffix(p.URI, ".xml") && !strings.Contains(p.URI, "/_rels/") {
			uris = append(uris, p.URI)
		}
	}
	sort.Strings(uris)
	for _, u := range uris {
		style, err := InspectStyle(session, u)
		if err != nil || style == nil {
			continue
		}
		out = append(out, summarizeOne(u, style))
	}
	return out
}

func summarizeOne(uri string, style *ChartStyle) tmpl.ChartStyleSummary {
	s := tmpl.ChartStyleSummary{PartURI: uri}
	if len(style.Types) > 0 {
		s.ChartType = style.Types[0]
	}
	if len(style.Series) > 0 {
		s.SeriesFillColor = style.Series[0].FillColor
		s.SeriesLineColor = style.Series[0].LineColor
	}
	if style.Title.Font != nil {
		s.TitleFontFamily = style.Title.Font.Family
	}
	return s
}
