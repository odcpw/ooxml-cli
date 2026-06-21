package cli

import (
	"os"
	"path/filepath"
	"testing"
)

type readSurfaceGoldens struct {
	XLSX xlsxReadSurfaceGolden `json:"xlsx"`
	VBA  vbaReadSurfaceGolden  `json:"vba"`
	DOCX docxReadSurfaceGolden `json:"docx"`
}

type xlsxReadSurfaceGolden struct {
	Sheets []xlsxReadSheetGolden `json:"sheets"`
	Range  xlsxReadRangeGolden   `json:"range"`
	Chart  xlsxReadChartGolden   `json:"chart"`
}

type xlsxReadSheetGolden struct {
	Name            string   `json:"name"`
	SheetID         string   `json:"sheetId"`
	PrimarySelector string   `json:"primarySelector"`
	Handle          string   `json:"handle"`
	Selectors       []string `json:"selectors"`
}

type xlsxReadRangeGolden struct {
	Sheet             string     `json:"sheet"`
	Range             string     `json:"range"`
	Rows              int        `json:"rows"`
	Cols              int        `json:"cols"`
	FormulaCount      int        `json:"formulaCount"`
	Values            [][]any    `json:"values"`
	Types             [][]string `json:"types"`
	NumberFormatCodes [][]any    `json:"numberFormatCodes"`
}

type xlsxReadChartGolden struct {
	Count         int      `json:"count"`
	Sheet         string   `json:"sheet"`
	Title         string   `json:"title"`
	Types         []string `json:"types"`
	SeriesCount   int      `json:"seriesCount"`
	CategoryRange string   `json:"categoryRange"`
	ValueRange    string   `json:"valueRange"`
	CategoryCount int      `json:"categoryCount"`
	ValueCount    int      `json:"valueCount"`
}

type vbaReadSurfaceGolden struct {
	InspectBinXLSX vbaInspectBinGolden `json:"inspectBinXlsx"`
	InspectBinPPTX vbaInspectBinGolden `json:"inspectBinPptx"`
}

type vbaInspectBinGolden struct {
	Family             string            `json:"family"`
	ModuleCount        int               `json:"moduleCount"`
	Modules            []vbaModuleGolden `json:"modules"`
	Compatibility      string            `json:"compatibility"`
	HostWarningCodes   []string          `json:"hostWarningCodes"`
	SignatureArtifacts int               `json:"signatureArtifacts"`
}

type vbaModuleGolden struct {
	Name            string `json:"name"`
	Kind            string `json:"kind"`
	Extension       string `json:"extension"`
	PrimarySelector string `json:"primarySelector"`
	LineCount       int    `json:"lineCount"`
}

type docxReadSurfaceGolden struct {
	Styles       docxStylesGolden       `json:"styles"`
	Block        docxBlockGolden        `json:"block"`
	Table        docxTableGolden        `json:"table"`
	HeaderFooter docxHeaderFooterGolden `json:"headerFooter"`
}

type docxStylesGolden struct {
	Count      int             `json:"count"`
	StylesPart string          `json:"stylesPart"`
	StyleIDs   []string        `json:"styleIds"`
	Heading1   docxStyleGolden `json:"heading1"`
}

type docxStyleGolden struct {
	StyleID string `json:"styleId"`
	Name    string `json:"name"`
	Type    string `json:"type"`
	BasedOn string `json:"basedOn"`
	Next    string `json:"next"`
	Handle  string `json:"handle"`
}

type docxBlockGolden struct {
	ID    string          `json:"id"`
	Index int             `json:"index"`
	Kind  string          `json:"kind"`
	Text  string          `json:"text"`
	Style string          `json:"style"`
	Runs  []docxRunGolden `json:"runs"`
}

type docxRunGolden struct {
	Text string `json:"text"`
	Bold bool   `json:"bold,omitempty"`
}

type docxTableGolden struct {
	Count  int        `json:"count"`
	Table  int        `json:"table"`
	Block  int        `json:"block"`
	Rows   int        `json:"rows"`
	Cols   int        `json:"cols"`
	Merged bool       `json:"merged"`
	Cells  [][]string `json:"cells"`
}

type docxHeaderFooterGolden struct {
	HeaderSelector          string   `json:"headerSelector"`
	HeaderSelectors         []string `json:"headerSelectors"`
	HeaderParagraphSelector string   `json:"headerParagraphSelector"`
	FooterSelector          string   `json:"footerSelector"`
	FooterSelectors         []string `json:"footerSelectors"`
	FooterParagraphSelector string   `json:"footerParagraphSelector"`
}

func TestPracticalReadSurfaceGoldens(t *testing.T) {
	actual := readSurfaceGoldens{
		XLSX: collectXLSXReadSurfaceGolden(t),
		VBA:  collectVBAReadSurfaceGolden(t),
		DOCX: collectDOCXReadSurfaceGolden(t),
	}
	assertGoldenJSONValue(t, "read_surface_summary.json", actual)
}

func collectXLSXReadSurfaceGolden(t *testing.T) xlsxReadSurfaceGolden {
	t.Helper()
	typesWorkbook := getXLSXTestFilePath("types-and-formulas")
	chartWorkbook := getXLSXTestFilePath("chart-workbook")

	var sheets XLSXSheetsListResult
	mustUnmarshalXLSXGolden(t, executeReadSurfaceCommand(t, "--format", "json", "xlsx", "sheets", "list", typesWorkbook), &sheets)
	sheetGoldens := make([]xlsxReadSheetGolden, 0, len(sheets.Sheets))
	for _, sheet := range sheets.Sheets {
		sheetGoldens = append(sheetGoldens, xlsxReadSheetGolden{
			Name:            sheet.Name,
			SheetID:         sheet.SheetID,
			PrimarySelector: sheet.PrimarySelector,
			Handle:          sheet.Handle,
			Selectors:       sheet.Selectors,
		})
	}

	var exported XLSXRangesExportResult
	mustUnmarshalXLSXGolden(t, executeReadSurfaceCommand(t,
		"--format", "json", "xlsx", "ranges", "export", typesWorkbook,
		"--sheet", "1",
		"--range", "A1:D4",
		"--include-types",
		"--include-formulas",
		"--include-formats",
	), &exported)

	var charts XLSXChartsResult
	mustUnmarshalXLSXGolden(t, executeReadSurfaceCommand(t, "--format", "json", "xlsx", "charts", "list", chartWorkbook), &charts)
	chartGolden := xlsxReadChartGolden{Count: len(charts.Charts)}
	if len(charts.Charts) > 0 {
		chart := charts.Charts[0]
		chartGolden.Sheet = chart.Sheet
		chartGolden.Title = chart.Title
		chartGolden.Types = append([]string(nil), chart.Types...)
		chartGolden.SeriesCount = len(chart.Series)
		if len(chart.Series) > 0 {
			series := chart.Series[0]
			if series.Categories != nil {
				chartGolden.CategoryRange = series.Categories.Range
				chartGolden.CategoryCount = series.Categories.PointCount
			}
			if series.Values != nil {
				chartGolden.ValueRange = series.Values.Range
				chartGolden.ValueCount = series.Values.PointCount
			}
		}
	}

	return xlsxReadSurfaceGolden{
		Sheets: sheetGoldens,
		Range: xlsxReadRangeGolden{
			Sheet:             exported.Sheet,
			Range:             exported.Range,
			Rows:              exported.Rows,
			Cols:              exported.Cols,
			FormulaCount:      exported.FormulaCount,
			Values:            exported.Values,
			Types:             exported.Types,
			NumberFormatCodes: exported.NumberFormatCodes,
		},
		Chart: chartGolden,
	}
}

func collectVBAReadSurfaceGolden(t *testing.T) vbaReadSurfaceGolden {
	t.Helper()
	projectData := syntheticCLIVBAProjectBinForModulesTest(t, []syntheticCLIVBAModule{
		{
			Name:       "Module1",
			StreamName: "Module1",
			Kind:       "standard",
			Source:     "Attribute VB_Name = \"Module1\"\r\nPublic Sub HelloWorld()\r\nEnd Sub\r\n",
		},
		{
			Name:       "Class1",
			StreamName: "Class1",
			Kind:       "class",
			Source:     "Attribute VB_Name = \"Class1\"\r\nPublic Function Answer()\r\nAnswer = 42\r\nEnd Function\r\n",
		},
		{
			Name:       "ThisWorkbook",
			StreamName: "ThisWorkbook",
			Kind:       "class",
			Source:     "Attribute VB_Name = \"ThisWorkbook\"\r\nPrivate Sub Workbook_Open()\r\nEnd Sub\r\n",
		},
	})
	binPath := filepath.Join(t.TempDir(), "vbaProject.bin")
	if err := os.WriteFile(binPath, projectData, 0o644); err != nil {
		t.Fatalf("write VBA project: %v", err)
	}

	var xlsx VBAInspectBinResult
	mustUnmarshalXLSXGolden(t, executeReadSurfaceCommand(t, "--format", "json", "vba", "inspect-bin", binPath, "--family", "xlsx"), &xlsx)
	var pptx VBAInspectBinResult
	mustUnmarshalXLSXGolden(t, executeReadSurfaceCommand(t, "--format", "json", "vba", "inspect-bin", binPath, "--family", "pptx"), &pptx)

	return vbaReadSurfaceGolden{
		InspectBinXLSX: summarizeVBAInspectBin(xlsx),
		InspectBinPPTX: summarizeVBAInspectBin(pptx),
	}
}

func summarizeVBAInspectBin(result VBAInspectBinResult) vbaInspectBinGolden {
	out := vbaInspectBinGolden{Family: result.Family}
	if result.Project == nil {
		return out
	}
	out.ModuleCount = result.Project.ModuleCount
	for _, module := range result.Project.Modules {
		out.Modules = append(out.Modules, vbaModuleGolden{
			Name:            module.Name,
			Kind:            module.Kind,
			Extension:       module.Extension,
			PrimarySelector: module.PrimarySelector,
			LineCount:       module.LineCount,
		})
	}
	if result.Project.OfficeCompatibility != nil {
		out.Compatibility = result.Project.OfficeCompatibility.Status
	}
	for _, warning := range result.Project.HostCompatibilityWarnings {
		out.HostWarningCodes = append(out.HostWarningCodes, warning.Code)
	}
	out.SignatureArtifacts = len(result.Project.SignatureArtifacts)
	return out
}

func collectDOCXReadSurfaceGolden(t *testing.T) docxReadSurfaceGolden {
	t.Helper()
	stylesPath := getDOCXTestFilePath("styles-catalog")
	blocksPath := getDOCXTestFilePath("mixed-blocks")
	tablePath := getDOCXTestFilePath("table")
	headersPath := getDOCXTestFilePath("headers")

	var styles DOCXStylesListResult
	mustUnmarshalXLSXGolden(t, executeReadSurfaceCommand(t, "--format", "json", "docx", "styles", "list", stylesPath), &styles)
	styleIDs := make([]string, 0, len(styles.Styles))
	for _, style := range styles.Styles {
		styleIDs = append(styleIDs, style.StyleID)
	}
	stylesPart := ""
	if styles.StylesPartURI != nil {
		stylesPart = *styles.StylesPartURI
	}

	var heading DOCXStylesShowResult
	mustUnmarshalXLSXGolden(t, executeReadSurfaceCommand(t, "--format", "json", "docx", "styles", "show", stylesPath, "--style", "Heading1"), &heading)
	headingGolden := docxStyleGolden{StyleID: heading.StyleID}
	if heading.Style != nil {
		headingGolden = docxStyleGolden{
			StyleID: heading.Style.StyleID,
			Name:    heading.Style.Name,
			Type:    heading.Style.Type,
			BasedOn: heading.Style.BasedOn,
			Next:    heading.Style.Next,
			Handle:  heading.Style.Handle,
		}
	}

	var blocks DOCXBlocksResult
	mustUnmarshalXLSXGolden(t, executeReadSurfaceCommand(t,
		"--format", "json", "docx", "blocks", blocksPath,
		"--block", "2",
		"--include-runs",
	), &blocks)
	blockGolden := docxBlockGolden{}
	if len(blocks.Blocks) > 0 {
		block := blocks.Blocks[0]
		blockGolden.ID = block.ID
		blockGolden.Index = block.Index
		blockGolden.Kind = string(block.Kind)
		blockGolden.Text = block.Text
		if block.Paragraph != nil {
			blockGolden.Style = block.Paragraph.Style
			for _, run := range block.Paragraph.Runs {
				blockGolden.Runs = append(blockGolden.Runs, docxRunGolden{Text: run.Text, Bold: run.Bold})
			}
		}
	}

	var tables DOCXTablesShowResult
	mustUnmarshalXLSXGolden(t, executeReadSurfaceCommand(t, "--format", "json", "docx", "tables", "show", tablePath), &tables)
	tableGolden := docxTableGolden{Count: len(tables.Tables)}
	if len(tables.Tables) > 0 {
		table := tables.Tables[0]
		tableGolden.Table = table.Table
		tableGolden.Block = table.Block
		tableGolden.Rows = table.Rows
		tableGolden.Cols = table.Cols
		tableGolden.Merged = table.Merged
		tableGolden.Cells = table.Cells
	}

	var headerFooterList DOCXHeadersListResult
	mustUnmarshalXLSXGolden(t, executeReadSurfaceCommand(t, "--format", "json", "docx", "headers", "list", headersPath), &headerFooterList)
	headerFooterGolden := docxHeaderFooterGolden{}
	if len(headerFooterList.Sections) > 0 {
		section := headerFooterList.Sections[0]
		if section.Headers != nil && section.Headers.Default != nil {
			headerFooterGolden.HeaderSelector = section.Headers.Default.PrimarySelector
			headerFooterGolden.HeaderSelectors = section.Headers.Default.Selectors
		}
		if section.Footers != nil && section.Footers.Default != nil {
			headerFooterGolden.FooterSelector = section.Footers.Default.PrimarySelector
			headerFooterGolden.FooterSelectors = section.Footers.Default.Selectors
		}
	}
	var headerShow DOCXHeadersShowResult
	mustUnmarshalXLSXGolden(t, executeReadSurfaceCommand(t, "--format", "json", "docx", "headers", "show", headersPath, "--selector", "header:1:default"), &headerShow)
	if len(headerShow.Paragraphs) > 0 {
		headerFooterGolden.HeaderParagraphSelector = headerShow.Paragraphs[0].PrimarySelector
	}
	var footerShow DOCXHeadersShowResult
	mustUnmarshalXLSXGolden(t, executeReadSurfaceCommand(t, "--format", "json", "docx", "footers", "show", headersPath, "--selector", "footer:1:default"), &footerShow)
	if len(footerShow.Paragraphs) > 0 {
		headerFooterGolden.FooterParagraphSelector = footerShow.Paragraphs[0].PrimarySelector
	}

	return docxReadSurfaceGolden{
		Styles: docxStylesGolden{
			Count:      styles.Count,
			StylesPart: stylesPart,
			StyleIDs:   styleIDs,
			Heading1:   headingGolden,
		},
		Block:        blockGolden,
		Table:        tableGolden,
		HeaderFooter: headerFooterGolden,
	}
}

func executeReadSurfaceCommand(t *testing.T, args ...string) string {
	t.Helper()
	out, err := executeRootForXLSXTest(t, args...)
	if err != nil {
		t.Fatalf("command failed: %v\nargs=%v\n%s", err, args, out)
	}
	return out
}
