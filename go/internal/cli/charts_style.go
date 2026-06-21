package cli

import (
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
	"github.com/spf13/cobra"
)

// chartLegendPositions maps friendly --position values (and raw ST_LegendPos
// codes) to the ST_LegendPos code stored in c:legendPos.
var chartLegendPositions = map[string]string{
	"right": "r", "left": "l", "top": "t", "bottom": "b",
	"r": "r", "l": "l", "t": "t", "b": "b", "tr": "tr",
}

// chartMarkerSymbols are the marker symbols this slice exposes (a practical
// subset of ST_MarkerStyle).
var chartMarkerSymbols = map[string]bool{
	"circle": true, "square": true, "diamond": true, "triangle": true, "none": true,
}

// parseChartLegendPosition resolves a --position value into an ST_LegendPos code.
// "none" signals that the legend should be removed entirely.
func parseChartLegendPosition(value string) (code string, remove bool, err error) {
	v := strings.ToLower(strings.TrimSpace(value))
	if v == "none" {
		return "", true, nil
	}
	code, ok := chartLegendPositions[v]
	if !ok {
		return "", false, InvalidArgsError("--position must be right, left, top, bottom, or none")
	}
	return code, false, nil
}

// parseChartExpectLegendPosition resolves an --expect-position guard value into
// the ST_LegendPos code expected on the current chart ("" means no legend).
func parseChartExpectLegendPosition(value string) (string, error) {
	code, remove, err := parseChartLegendPosition(value)
	if err != nil {
		return "", InvalidArgsError("--expect-position must be right, left, top, bottom, or none")
	}
	if remove {
		return "", nil
	}
	return code, nil
}

func parseChartMarkerSymbol(value string) (string, error) {
	v := strings.ToLower(strings.TrimSpace(value))
	if !chartMarkerSymbols[v] {
		return "", InvalidArgsError("--marker-symbol must be circle, square, diamond, triangle, or none")
	}
	return v, nil
}

// chartFontFlags carries optional title font overrides resolved from a command.
type chartFontFlags struct {
	family string
	sizePt *float64
	color  string
	bold   *bool
	italic *bool
}

// resolveChartFontFlags reads the optional font flags shared by the set-title
// commands. Only flags the user actually set are returned, so unspecified font
// fields are left untouched.
func resolveChartFontFlags(cmd *cobra.Command, family string, sizePt float64, color string, bold, italic bool) (chartFontFlags, error) {
	f := chartFontFlags{}
	if cmd.Flags().Changed("font-family") {
		if strings.TrimSpace(family) == "" {
			return f, InvalidArgsError("--font-family must not be empty")
		}
		f.family = strings.TrimSpace(family)
	}
	if cmd.Flags().Changed("font-size") {
		if sizePt <= 0 {
			return f, InvalidArgsError("--font-size must be greater than 0")
		}
		v := sizePt
		f.sizePt = &v
	}
	if cmd.Flags().Changed("font-color") {
		normalized, err := xlsxchart.NormalizeHexColor(color)
		if err != nil {
			return f, NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		f.color = normalized
	}
	if cmd.Flags().Changed("font-bold") {
		v := bold
		f.bold = &v
	}
	if cmd.Flags().Changed("font-italic") {
		v := italic
		f.italic = &v
	}
	return f, nil
}

// chartSeriesStyleFlags carries optional series style overrides from a command.
type chartSeriesStyleFlags struct {
	fillColor   string
	lineColor   string
	lineWidthPt *float64
	markerSym   string
	markerSize  *int
}

// resolveChartSeriesStyleFlags reads and validates the series style flags shared
// by the set-series-style commands, requiring at least one to be present.
func resolveChartSeriesStyleFlags(cmd *cobra.Command, fillColor, lineColor string, lineWidthPt float64, markerSymbol string, markerSize int) (chartSeriesStyleFlags, error) {
	f := chartSeriesStyleFlags{}
	any := false
	if cmd.Flags().Changed("fill-color") {
		normalized, err := xlsxchart.NormalizeHexColor(fillColor)
		if err != nil {
			return f, NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		f.fillColor = normalized
		any = true
	}
	if cmd.Flags().Changed("line-color") {
		normalized, err := xlsxchart.NormalizeHexColor(lineColor)
		if err != nil {
			return f, NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		f.lineColor = normalized
		any = true
	}
	if cmd.Flags().Changed("line-width-pt") {
		if lineWidthPt <= 0 {
			return f, InvalidArgsError("--line-width-pt must be greater than 0")
		}
		v := lineWidthPt
		f.lineWidthPt = &v
		any = true
	}
	if cmd.Flags().Changed("marker-symbol") {
		symbol, err := parseChartMarkerSymbol(markerSymbol)
		if err != nil {
			return f, err
		}
		f.markerSym = symbol
		any = true
	}
	if cmd.Flags().Changed("marker-size") {
		if markerSize < 2 || markerSize > 72 {
			return f, InvalidArgsError("--marker-size must be between 2 and 72")
		}
		v := markerSize
		f.markerSize = &v
		any = true
	}
	if !any {
		return f, InvalidArgsError("set-series-style requires at least one of --fill-color, --line-color, --line-width-pt, --marker-symbol, or --marker-size")
	}
	return f, nil
}

// parseChartAxisKind validates an --axis value into the canonical axis kind.
func parseChartAxisKind(value string) (string, error) {
	v := strings.ToLower(strings.TrimSpace(value))
	switch v {
	case "category", "value":
		return v, nil
	default:
		return "", InvalidArgsError("--axis is required; use category or value")
	}
}

// chartAxisFlags carries the resolved axis mutation overrides shared by the
// pptx/xlsx set-axis commands. Only flags the user actually set are populated.
type chartAxisFlags struct {
	setTitle bool
	title    string

	setHidden bool
	hidden    bool

	min          *float64
	max          *float64
	majorUnit    *float64
	numberFormat string

	setMajorGridlines bool
	majorGridlines    bool
	setMinorGridlines bool
	minorGridlines    bool

	tickFamily string
	tickSizePt *float64
	tickColor  string
	tickBold   *bool
	tickItalic *bool

	titleFamily string
	titleSizePt *float64
	titleColor  string
	titleBold   *bool
	titleItalic *bool
}

// axisFlagInputs holds the raw flag-backed values for resolveChartAxisFlags.
type axisFlagInputs struct {
	title          string
	hidden         bool
	min            float64
	max            float64
	majorUnit      float64
	numberFormat   string
	majorGridlines bool
	minorGridlines bool
	tickFamily     string
	tickSize       float64
	tickColor      string
	tickBold       bool
	tickItalic     bool
	titleFamily    string
	titleSize      float64
	titleColor     string
	titleBold      bool
	titleItalic    bool
}

// resolveChartAxisFlags reads and validates the axis mutation flags, requiring
// at least one property to change.
func resolveChartAxisFlags(cmd *cobra.Command, in axisFlagInputs) (chartAxisFlags, error) {
	f := chartAxisFlags{}
	any := false
	if cmd.Flags().Changed("title") {
		f.setTitle = true
		f.title = in.title
		any = true
	}
	if cmd.Flags().Changed("hidden") {
		f.setHidden = true
		f.hidden = in.hidden
		any = true
	}
	if cmd.Flags().Changed("min") {
		v := in.min
		f.min = &v
		any = true
	}
	if cmd.Flags().Changed("max") {
		v := in.max
		f.max = &v
		any = true
	}
	if f.min != nil && f.max != nil && *f.min >= *f.max {
		return f, InvalidArgsError("--min must be less than --max")
	}
	if cmd.Flags().Changed("major-unit") {
		if in.majorUnit <= 0 {
			return f, InvalidArgsError("--major-unit must be greater than 0")
		}
		v := in.majorUnit
		f.majorUnit = &v
		any = true
	}
	if cmd.Flags().Changed("number-format") {
		if strings.TrimSpace(in.numberFormat) == "" {
			return f, InvalidArgsError("--number-format must not be empty")
		}
		f.numberFormat = in.numberFormat
		any = true
	}
	if cmd.Flags().Changed("major-gridlines") {
		f.setMajorGridlines = true
		f.majorGridlines = in.majorGridlines
		any = true
	}
	if cmd.Flags().Changed("minor-gridlines") {
		f.setMinorGridlines = true
		f.minorGridlines = in.minorGridlines
		any = true
	}
	if cmd.Flags().Changed("tick-label-font-size") {
		if in.tickSize <= 0 {
			return f, InvalidArgsError("--tick-label-font-size must be greater than 0")
		}
		v := in.tickSize
		f.tickSizePt = &v
		any = true
	}
	if cmd.Flags().Changed("tick-label-font-color") {
		normalized, err := xlsxchart.NormalizeHexColor(in.tickColor)
		if err != nil {
			return f, NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		f.tickColor = normalized
		any = true
	}
	if cmd.Flags().Changed("tick-label-font-bold") {
		v := in.tickBold
		f.tickBold = &v
		any = true
	}
	if cmd.Flags().Changed("tick-label-font-italic") {
		v := in.tickItalic
		f.tickItalic = &v
		any = true
	}
	if cmd.Flags().Changed("tick-label-font-family") {
		if strings.TrimSpace(in.tickFamily) == "" {
			return f, InvalidArgsError("--tick-label-font-family must not be empty")
		}
		f.tickFamily = strings.TrimSpace(in.tickFamily)
		any = true
	}
	if cmd.Flags().Changed("title-font-family") {
		if strings.TrimSpace(in.titleFamily) == "" {
			return f, InvalidArgsError("--title-font-family must not be empty")
		}
		f.titleFamily = strings.TrimSpace(in.titleFamily)
		any = true
	}
	if cmd.Flags().Changed("title-font-size") {
		if in.titleSize <= 0 {
			return f, InvalidArgsError("--title-font-size must be greater than 0")
		}
		v := in.titleSize
		f.titleSizePt = &v
		any = true
	}
	if cmd.Flags().Changed("title-font-color") {
		normalized, err := xlsxchart.NormalizeHexColor(in.titleColor)
		if err != nil {
			return f, NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		f.titleColor = normalized
		any = true
	}
	if cmd.Flags().Changed("title-font-bold") {
		v := in.titleBold
		f.titleBold = &v
		any = true
	}
	if cmd.Flags().Changed("title-font-italic") {
		v := in.titleItalic
		f.titleItalic = &v
		any = true
	}
	if !any {
		return f, InvalidArgsError("set-axis requires at least one of --title, --hidden, --min, --max, --major-unit, --number-format, --major-gridlines, --minor-gridlines, or a --title-font-*/--tick-label-font-* flag")
	}
	return f, nil
}

// buildSetAxisRequest assembles a SetAxisRequest from resolved CLI flags. It is
// shared by the pptx and xlsx set-axis commands so both families stay identical.
func buildSetAxisRequest(pkg opc.PackageSession, chartURI, kind string, f chartAxisFlags, expectTitle *string, expectCount *int) *xlsxchart.SetAxisRequest {
	return &xlsxchart.SetAxisRequest{
		Package:             pkg,
		ChartURI:            chartURI,
		AxisKind:            kind,
		SetTitle:            f.setTitle,
		Title:               f.title,
		SetHidden:           f.setHidden,
		Hidden:              f.hidden,
		Min:                 f.min,
		Max:                 f.max,
		MajorUnit:           f.majorUnit,
		NumberFormat:        f.numberFormat,
		SetMajorGridlines:   f.setMajorGridlines,
		MajorGridlines:      f.majorGridlines,
		SetMinorGridlines:   f.setMinorGridlines,
		MinorGridlines:      f.minorGridlines,
		TickLabelFontFamily: f.tickFamily,
		TickLabelFontSizePt: f.tickSizePt,
		TickLabelFontColor:  f.tickColor,
		TickLabelFontBold:   f.tickBold,
		TickLabelFontItalic: f.tickItalic,
		TitleFontFamily:     f.titleFamily,
		TitleFontSizePt:     f.titleSizePt,
		TitleFontColor:      f.titleColor,
		TitleFontBold:       f.titleBold,
		TitleFontItalic:     f.titleItalic,
		ExpectAxisTitle:     expectTitle,
		ExpectAxisCount:     expectCount,
	}
}

// resolveChartConvertType validates the required --to value and the optional
// --expect-type guard, shared by the pptx/xlsx convert-type commands.
func resolveChartConvertType(cmd *cobra.Command, to, expect string) (xlsxchart.ChartType, *xlsxchart.ChartType, error) {
	if !cmd.Flags().Changed("to") {
		return "", nil, InvalidArgsError("--to is required (bar, column, line, area, pie, or scatter)")
	}
	target, err := xlsxchart.ParseChartType(to)
	if err != nil {
		return "", nil, NewCLIErrorf(ExitInvalidArgs, "%v", err)
	}
	var expectType *xlsxchart.ChartType
	if cmd.Flags().Changed("expect-type") {
		parsed, perr := xlsxchart.ParseChartType(expect)
		if perr != nil {
			return "", nil, NewCLIErrorf(ExitInvalidArgs, "invalid --expect-type: %v", perr)
		}
		expectType = &parsed
	}
	return target, expectType, nil
}

func chartStyleSelectorOrDefault(selector string) string {
	if strings.TrimSpace(selector) == "" {
		return "chart:1"
	}
	return selector
}
