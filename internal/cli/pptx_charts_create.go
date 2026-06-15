package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxchart "github.com/ooxml-cli/ooxml-cli/pkg/pptx/chart"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	pptxmutate "github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/rangeio"
	"github.com/spf13/cobra"
)

// emuPerInch is the EMU count in one inch (English Metric Units).
const emuPerInch = int64(914400)

type PPTXChartsCreateResult struct {
	File                    string               `json:"file"`
	Output                  string               `json:"output,omitempty"`
	DryRun                  bool                 `json:"dryRun"`
	Action                  string               `json:"action"`
	Slide                   int                  `json:"slide"`
	ChartType               string               `json:"chartType"`
	Title                   string               `json:"title,omitempty"`
	ChartPartURI            string               `json:"chartPartUri"`
	ChartRelationshipID     string               `json:"chartRelationshipId"`
	ShapeID                 int                  `json:"shapeId"`
	ShapeName               string               `json:"shapeName"`
	SeriesCount             int                  `json:"seriesCount"`
	Categories              int                  `json:"categories"`
	X                       int64                `json:"x"`
	Y                       int64                `json:"y"`
	CX                      int64                `json:"cx"`
	CY                      int64                `json:"cy"`
	SourceMode              string               `json:"sourceMode"`
	SourceFile              string               `json:"sourceFile,omitempty"`
	SourceSheet             string               `json:"sourceSheet,omitempty"`
	SourceRange             string               `json:"sourceRange,omitempty"`
	EmbeddedWorkbookPartURI string               `json:"embeddedWorkbookPartUri,omitempty"`
	Chart                   *PPTXChartResultItem `json:"chart,omitempty"`
	Warnings                []string             `json:"warnings,omitempty"`
	ChartShowCommand        string               `json:"chartShowCommand,omitempty"`
	ChartsListCommand       string               `json:"chartsListCommand,omitempty"`
	ValidateCommand         string               `json:"validateCommand,omitempty"`
	RenderCommand           string               `json:"renderCommand,omitempty"`
	ChartShowCommandTpl     string               `json:"chartShowCommandTemplate,omitempty"`
	ChartsListCommandTpl    string               `json:"chartsListCommandTemplate,omitempty"`
	ValidateCommandTpl      string               `json:"validateCommandTemplate,omitempty"`
	RenderCommandTpl        string               `json:"renderCommandTemplate,omitempty"`
}

var (
	pptxChartsCreateSlide         int
	pptxChartsCreateType          string
	pptxChartsCreateTitle         string
	pptxChartsCreateValuesJSON    string
	pptxChartsCreateValuesFile    string
	pptxChartsCreateSourceFile    string
	pptxChartsCreateSourceSheet   string
	pptxChartsCreateSourceRange   string
	pptxChartsCreateExpectRange   string
	pptxChartsCreateMaxCells      int
	pptxChartsCreateX             int64
	pptxChartsCreateY             int64
	pptxChartsCreateCX            int64
	pptxChartsCreateCY            int64
	pptxChartsCreateEmbedWorkbook bool
)

var pptxChartsCreateCmd = &cobra.Command{
	Use:   "create <file>",
	Short: "Author a new slide chart from inline data or an xlsx range",
	Long: `Create a bar, line, area, pie, or scatter chart on a slide.

Data source (exactly one):
  --values-json '[["","S1"],["A",10],["B",20]]'   inline row-major matrix
  --values-file values.json                         inline matrix from a JSON file
  --source-file data.xlsx --source-sheet Sheet1 --source-range A1:B3

The first column holds categories and the first row holds series names. With an
xlsx source, pass --embed-workbook to embed the source workbook so the chart
data stays editable.

Geometry uses EMUs (1 inch = 914400). When omitted, the chart is centred on the
slide using the presentation slide size.`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if pptxChartsCreateSlide < 1 {
			return InvalidArgsError("--slide must be >= 1")
		}
		chartType := strings.ToLower(strings.TrimSpace(pptxChartsCreateType))
		if chartType == "" {
			return InvalidArgsError("--type is required (bar, line, area, pie, scatter)")
		}

		sourceMode, sourceSheet, sourceRange, matrix, embedded, err := resolvePPTXChartCreateSource(filePath)
		if err != nil {
			return err
		}
		parsedRange, err := address.ParseRange(sourceRange)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "invalid source range %q: %v", sourceRange, err)
		}

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}
		wantReadback := GetGlobalConfig(cmd).Format == "json"

		writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
		if err != nil {
			return err
		}

		var result *PPTXChartsCreateResult
		if err := writer.Write(func(session opc.PackageSession) error {
			graph, err := inspect.ParsePresentation(session)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", err)
			}
			if pptxChartsCreateSlide > len(graph.Slides) {
				return NewCLIErrorf(ExitTargetNotFound, "slide %d not found (presentation has %d slides)", pptxChartsCreateSlide, len(graph.Slides))
			}
			slideRef := graph.Slides[pptxChartsCreateSlide-1]

			x, y, cx, cy := resolvePPTXChartGeometry(cmd, graph.SlideSize)

			createResult, err := pptxmutate.CreateSlideChart(&pptxmutate.CreateSlideChartRequest{
				Package:          session,
				SlideRef:         &slideRef,
				ChartType:        chartType,
				Title:            pptxChartsCreateTitle,
				SourceSheet:      sourceSheet,
				SourceRange:      parsedRange,
				SourceCells:      matrix,
				X:                x,
				Y:                y,
				CX:               cx,
				CY:               cy,
				EmbeddedWorkbook: embedded,
			})
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "failed to create chart: %v", err)
			}

			destinationFile := mutationOutputPathForResult(filePath, mutOpts)
			result = &PPTXChartsCreateResult{
				File:                    filePath,
				Output:                  destinationFile,
				DryRun:                  mutOpts != nil && mutOpts.DryRun,
				Action:                  "pptx.chart.create",
				Slide:                   pptxChartsCreateSlide,
				ChartType:               createResult.ChartType,
				Title:                   createResult.Title,
				ChartPartURI:            createResult.ChartURI,
				ChartRelationshipID:     createResult.ChartRelationshipID,
				ShapeID:                 createResult.ShapeID,
				ShapeName:               createResult.ShapeName,
				SeriesCount:             createResult.SeriesCount,
				Categories:              createResult.Categories,
				X:                       x,
				Y:                       y,
				CX:                      cx,
				CY:                      cy,
				SourceMode:              sourceMode,
				SourceSheet:             sourceSheet,
				SourceRange:             sourceRange,
				EmbeddedWorkbookPartURI: createResult.EmbeddedWorkbookPartURI,
				Warnings:                uniqueSortedWarnings(createResult.Warnings),
			}
			if sourceMode == "external" {
				result.SourceFile = pptxChartsCreateSourceFile
			}

			if wantReadback {
				charts, err := pptxchart.List(session, pptxChartsCreateSlide)
				if err != nil {
					return NewCLIErrorf(ExitUnexpected, "failed to read back PPTX charts: %v", err)
				}
				selected, err := selectPPTXChart(charts, "part:"+createResult.ChartURI)
				if err != nil {
					return err
				}
				item := pptxChartItemForUpdate(destinationFile, selected)
				result.Chart = &item
			}
			return nil
		}); err != nil {
			return err
		}

		applyPPTXChartCreateCommands(result)

		if GetGlobalConfig(cmd).Format == "json" {
			return outputPPTXChartCreateJSON(cmd, result)
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("created %s chart on slide %d (%d series) shape %d", result.ChartType, result.Slide, result.SeriesCount, result.ShapeID)))
	},
}

// resolvePPTXChartCreateSource resolves the chart data source from inline values
// or an xlsx range. It returns the source mode, the formula sheet name, the
// formula range, the cell matrix, and any embedded workbook bytes.
func resolvePPTXChartCreateSource(filePath string) (mode, sheet, rangeStr string, matrix [][]rangeio.Cell, embedded []byte, err error) {
	inlineJSON := strings.TrimSpace(pptxChartsCreateValuesJSON)
	inlineFile := strings.TrimSpace(pptxChartsCreateValuesFile)
	sourceFile := strings.TrimSpace(pptxChartsCreateSourceFile)

	inlineCount := 0
	if inlineJSON != "" {
		inlineCount++
	}
	if inlineFile != "" {
		inlineCount++
	}
	if inlineCount > 1 {
		return "", "", "", nil, nil, InvalidArgsError("specify only one of --values-json or --values-file")
	}
	if inlineCount == 1 && sourceFile != "" {
		return "", "", "", nil, nil, InvalidArgsError("specify either inline values or --source-file, not both")
	}

	if sourceFile != "" {
		src, cells, rerr := loadXLSXRangeOrTableSourceForCLI(sourceFile, pptxChartsCreateSourceSheet, pptxChartsCreateSourceRange, "", pptxChartsCreateMaxCells)
		if rerr != nil {
			return "", "", "", nil, nil, rerr
		}
		if pptxChartsCreateExpectRange != "" && !strings.EqualFold(src.Range, pptxChartsCreateExpectRange) {
			return "", "", "", nil, nil, NewCLIErrorf(ExitInvalidArgs, "source range mismatch: expected %s but found %s", pptxChartsCreateExpectRange, src.Range)
		}
		var wb []byte
		if pptxChartsCreateEmbedWorkbook {
			wb, rerr = os.ReadFile(sourceFile)
			if rerr != nil {
				return "", "", "", nil, nil, NewCLIErrorf(ExitUnexpected, "failed to read source workbook for embedding: %v", rerr)
			}
		}
		return "external", src.Sheet, src.Range, cells, wb, nil
	}

	// Inline source.
	var raw string
	if inlineFile != "" {
		data, rerr := os.ReadFile(inlineFile)
		if rerr != nil {
			return "", "", "", nil, nil, NewCLIErrorf(ExitInvalidArgs, "failed to read --values-file: %v", rerr)
		}
		raw = string(data)
	} else if inlineJSON != "" {
		raw = inlineJSON
	} else {
		return "", "", "", nil, nil, InvalidArgsError("must specify --values-json, --values-file, or --source-file")
	}

	cells, rng, perr := parsePPTXChartInlineMatrix(raw, pptxChartsCreateMaxCells)
	if perr != nil {
		return "", "", "", nil, nil, perr
	}
	return "inline", "Sheet1", rng, cells, nil, nil
}

// parsePPTXChartInlineMatrix parses a row-major JSON matrix into a cell matrix
// anchored at A1, returning the matrix and its A1 range. Numeric cells are typed
// as numbers; everything else is a string.
func parsePPTXChartInlineMatrix(raw string, maxCells int) ([][]rangeio.Cell, string, error) {
	var rows [][]interface{}
	if err := json.Unmarshal([]byte(raw), &rows); err != nil {
		return nil, "", NewCLIErrorf(ExitInvalidArgs, "invalid --values JSON matrix: %v", err)
	}
	if len(rows) == 0 {
		return nil, "", InvalidArgsError("inline values matrix is empty")
	}
	cols := 0
	for _, r := range rows {
		if len(r) > cols {
			cols = len(r)
		}
	}
	if cols == 0 {
		return nil, "", InvalidArgsError("inline values matrix has no columns")
	}
	if maxCells > 0 && len(rows)*cols > maxCells {
		return nil, "", NewCLIErrorf(ExitInvalidArgs, "inline matrix has %d cells, exceeding --max-cells %d", len(rows)*cols, maxCells)
	}

	matrix := make([][]rangeio.Cell, len(rows))
	for ri, r := range rows {
		matrix[ri] = make([]rangeio.Cell, cols)
		for ci := 0; ci < cols; ci++ {
			if ci >= len(r) || r[ci] == nil {
				matrix[ri][ci] = rangeio.Cell{Null: true}
				continue
			}
			matrix[ri][ci] = inlineCellFromValue(r[ci])
		}
	}

	endCol, err := address.ColumnIndexToLetters(cols)
	if err != nil {
		return nil, "", NewCLIErrorf(ExitInvalidArgs, "matrix too wide: %v", err)
	}
	rng := fmt.Sprintf("A1:%s%d", endCol, len(rows))
	return matrix, rng, nil
}

func inlineCellFromValue(v interface{}) rangeio.Cell {
	switch t := v.(type) {
	case float64:
		return rangeio.Cell{Type: "number", Value: trimFloat(t)}
	case json.Number:
		return rangeio.Cell{Type: "number", Value: t.String()}
	case bool:
		if t {
			return rangeio.Cell{Type: "boolean", Value: "1"}
		}
		return rangeio.Cell{Type: "boolean", Value: "0"}
	case string:
		return rangeio.Cell{Type: "string", Value: t}
	default:
		return rangeio.Cell{Type: "string", Value: fmt.Sprintf("%v", t)}
	}
}

func trimFloat(f float64) string {
	return strconv.FormatFloat(f, 'f', -1, 64)
}

// resolvePPTXChartGeometry returns the chart EMU geometry, defaulting to a
// centred chart sized at half the slide when flags are not provided. The --x and
// --y flags are recentred only when they were not supplied on the command line,
// so an explicit --x 0 / --y 0 anchors the chart at the slide's top/left edge.
func resolvePPTXChartGeometry(cmd *cobra.Command, size inspect.SlideSizeInfo) (x, y, cx, cy int64) {
	slideCX := size.CX
	slideCY := size.CY
	if slideCX <= 0 {
		slideCX = 10 * emuPerInch
	}
	if slideCY <= 0 {
		slideCY = int64(7.5 * float64(emuPerInch))
	}

	cx = pptxChartsCreateCX
	cy = pptxChartsCreateCY
	if cx <= 0 {
		cx = slideCX / 2
	}
	if cy <= 0 {
		cy = slideCY / 2
	}

	xChanged := cmd != nil && cmd.Flags().Changed("x")
	yChanged := cmd != nil && cmd.Flags().Changed("y")

	x = pptxChartsCreateX
	y = pptxChartsCreateY
	if !xChanged {
		x = (slideCX - cx) / 2
		if x < 0 {
			x = 0
		}
	}
	if !yChanged {
		y = (slideCY - cy) / 2
		if y < 0 {
			y = 0
		}
	}
	return x, y, cx, cy
}

func applyPPTXChartCreateCommands(result *PPTXChartsCreateResult) {
	if result == nil {
		return
	}
	selector := "part:" + result.ChartPartURI
	if result.Output == "" {
		placeholder := outputPlaceholder()
		result.ChartShowCommandTpl = pptxChartShowCommand(placeholder, result.Slide, selector)
		result.ChartsListCommandTpl = pptxChartsListCommand(placeholder, result.Slide)
		result.ValidateCommandTpl = pptxValidateCommand(placeholder)
		result.RenderCommandTpl = pptxRenderCommand(placeholder)
		return
	}
	result.ChartShowCommand = pptxChartShowCommand(result.Output, result.Slide, selector)
	result.ChartsListCommand = pptxChartsListCommand(result.Output, result.Slide)
	result.ValidateCommand = pptxValidateCommand(result.Output)
	result.RenderCommand = pptxRenderCommand(result.Output)
}

func pptxChartsListCommand(filePath string, slide int) string {
	command := fmt.Sprintf("ooxml --json pptx charts list %s", pptxXLSXCommandArg(filePath))
	if slide > 0 {
		command += fmt.Sprintf(" --slide %d", slide)
	}
	return command
}

func outputPPTXChartCreateJSON(cmd *cobra.Command, result *PPTXChartsCreateResult) error {
	config := GetGlobalConfig(cmd)
	var (
		data []byte
		err  error
	)
	if config.Pretty {
		data, err = json.MarshalIndent(result, "", "  ")
	} else {
		data, err = json.Marshal(result)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal PPTX chart create JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func init() {
	f := pptxChartsCreateCmd.Flags()
	f.IntVar(&pptxChartsCreateSlide, "slide", 0, "1-based slide number to add the chart to (required)")
	f.StringVar(&pptxChartsCreateType, "type", "", "chart type: bar, line, area, pie, or scatter")
	f.StringVar(&pptxChartsCreateTitle, "title", "", "chart title")
	f.StringVar(&pptxChartsCreateValuesJSON, "values-json", "", "inline row-major JSON matrix (first column categories, first row series names)")
	f.StringVar(&pptxChartsCreateValuesFile, "values-file", "", "path to a JSON file containing the inline matrix")
	f.StringVar(&pptxChartsCreateSourceFile, "source-file", "", "xlsx file to source chart data from")
	f.StringVar(&pptxChartsCreateSourceSheet, "source-sheet", "", "source sheet number (1-based) or exact name")
	f.StringVar(&pptxChartsCreateSourceRange, "source-range", "", "source A1 range such as A1:C5")
	f.StringVar(&pptxChartsCreateExpectRange, "expect-source-range", "", "guard: require the resolved source range to match")
	f.IntVar(&pptxChartsCreateMaxCells, "max-cells", 100000, "maximum source cells to read (0 for unlimited)")
	f.Int64Var(&pptxChartsCreateX, "x", 0, "left position in EMUs (default: centred)")
	f.Int64Var(&pptxChartsCreateY, "y", 0, "top position in EMUs (default: centred)")
	f.Int64Var(&pptxChartsCreateCX, "cx", 0, "width in EMUs (default: half the slide width)")
	f.Int64Var(&pptxChartsCreateCY, "cy", 0, "height in EMUs (default: half the slide height)")
	f.BoolVar(&pptxChartsCreateEmbedWorkbook, "embed-workbook", false, "embed the source xlsx workbook so chart data stays editable (requires --source-file)")
	AddMutationFlags(pptxChartsCreateCmd)
	chartsCmd.AddCommand(pptxChartsCreateCmd)
}
