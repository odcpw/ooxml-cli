package cli

import (
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxchart "github.com/ooxml-cli/ooxml-cli/pkg/pptx/chart"
	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
)

// resolveChartFillColor validates a --fill-color value. "none" (case-insensitive)
// requests an explicit no-fill; any other value must be a hex RRGGBB color.
func resolveChartFillColor(value string) (color string, noFill bool, err error) {
	v := strings.TrimSpace(value)
	if strings.EqualFold(v, "none") {
		return "", true, nil
	}
	normalized, nerr := xlsxchart.NormalizeHexColor(v)
	if nerr != nil {
		return "", false, NewCLIErrorf(ExitInvalidArgs, "%v", nerr)
	}
	return normalized, false, nil
}

// resolveChartExpectFill resolves an --expect-fill guard value. It accepts
// "none", "scheme:<name>", or a hex RRGGBB color (normalized when hex).
func resolveChartExpectFill(value string) (string, error) {
	v := strings.TrimSpace(value)
	if v == "" || strings.EqualFold(v, "none") {
		return "none", nil
	}
	if strings.HasPrefix(v, "scheme:") {
		return v, nil
	}
	normalized, err := xlsxchart.NormalizeHexColor(v)
	if err != nil {
		return "", NewCLIErrorf(ExitInvalidArgs, "--expect-fill must be a #RRGGBB color, scheme:<name>, or none")
	}
	return normalized, nil
}

func inspectChartStylesByPart(filePath string, packageType opc.PackageType, partURIs []string) map[string]*xlsxchart.ChartStyle {
	result := map[string]*xlsxchart.ChartStyle{}
	pkg, err := openPackageExpectType(filePath, packageType)
	if err != nil {
		return result
	}
	defer pkg.Close()
	for _, partURI := range partURIs {
		if partURI == "" {
			continue
		}
		if _, ok := result[partURI]; ok {
			continue
		}
		if style, err := xlsxchart.InspectStyle(pkg, partURI); err == nil {
			result[partURI] = style
		}
	}
	return result
}

// readXLSXTemplateChartStyle opens an XLSX file read-only and returns the style
// of the chart selected by chartSel (using the standard chart selector grammar).
func readXLSXTemplateChartStyle(filePath, chartSel string) (*xlsxchart.ChartStyle, error) {
	pkg, err := opc.Open(filePath)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to open template %s: %v", filePath, err)
	}
	defer pkg.Close()
	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to parse template workbook: %v", err)
	}
	charts, err := xlsxchart.List(pkg, workbook, nil)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to list template charts: %v", err)
	}
	selected, err := selectXLSXChart(charts, chartSel)
	if err != nil {
		return nil, err
	}
	style, err := xlsxchart.InspectStyle(pkg, selected.PartURI)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to read template chart style: %v", err)
	}
	return style, nil
}

// readPPTXTemplateChartStyle opens a PPTX file read-only and returns the style of
// the chart selected by chartSel on the given slide (0 searches all slides).
func readPPTXTemplateChartStyle(filePath string, slide int, chartSel string) (*xlsxchart.ChartStyle, error) {
	pkg, err := opc.Open(filePath)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to open template %s: %v", filePath, err)
	}
	defer pkg.Close()
	charts, err := pptxchart.List(pkg, slide)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to list template charts: %v", err)
	}
	selected, err := selectPPTXChart(charts, chartSel)
	if err != nil {
		return nil, err
	}
	style, err := xlsxchart.InspectStyle(pkg, selected.PartURI)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to read template chart style: %v", err)
	}
	return style, nil
}
