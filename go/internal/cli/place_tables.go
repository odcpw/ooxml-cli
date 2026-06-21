package cli

import (
	"encoding/csv"
	"encoding/json"
	"fmt"
	"io"
	"os"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
)

var (
	placeTableSlide         int
	placeTableFile          string
	placeTableFormat        string // "csv" or "json"
	placeTableX             int64
	placeTableY             int64
	placeTableWidth         int64
	placeTableHeight        int64
	placeTableHasHeader     bool
	placeTableHasBandedRows bool
	placeTableHeaderColor   string
	placeTableBand1Color    string
	placeTableBand2Color    string
	placeTableFontSize      int
	placeTableBorderColor   string
	placeTableBorderWidth   int64
	placeTableName          string
)

var placeTableCmd = &cobra.Command{
	Use:   "table <file>",
	Short: "Place a table on a slide from CSV or JSON data",
	Long: `Place a new table on a slide from CSV or JSON data at specific EMU coordinates.

Usage:
  ooxml pptx place table <file> --slide <n> --data <path> --format <csv|json> --x <emus> --y <emus> --cx <emus> [--cy <emus>] [--header] [--banded-rows] [--out <output>] [--in-place] [--backup <backup>]

Coordinates:
  All coordinates (x, y, cx, cy) are specified in EMUs (English Metric Units).
  - 1 inch = 914400 EMUs
  - 1 cm = 360000 EMUs
  - x, y: position from top-left
  - cx: table width (required)
  - cy: table height (optional, auto-calculated if not provided)

Data Format:
  --format csv              - CSV file (comma-separated values)
  --format json             - JSON array of arrays: [["col1", "col2"], ["val1", "val2"]]

Table Options:
  --header                  - First row is header (applies bold formatting)
  --banded-rows             - Alternate rows have different background colors
  --header-color <hex>      - Header background color (e.g., "4472C4", default: "4472C4")
  --band1-color <hex>       - Band 1 background color (e.g., "D9E1F2", default: "D9E1F2")
  --band2-color <hex>       - Band 2 background color (optional)
  --font-size <pts>         - Default font size in points (default: 18)
  --border-color <hex>      - Border color (default: "000000")
  --border-width <emus>     - Border width in EMUs (default: 19050 = 0.5pt)

Output Options:
  --out <path>              - Write to output file (mutually exclusive with --in-place)
  --in-place                - Modify the input file directly (mutually exclusive with --out)
  --backup <path>           - Create backup when using --in-place (optional)

Examples:
  # Place a table from CSV at (1", 1") with width 5"
  ooxml pptx place table deck.pptx --slide 1 --data data.csv --format csv --x 914400 --y 914400 --cx 4572000 --header --banded-rows --out out.pptx

  # Place a table from JSON
  ooxml pptx place table deck.pptx --slide 2 --data data.json --format json --x 0 --y 0 --cx 9144000 --in-place`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]

		// Check if file exists
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		// Validate mutation flags
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		// Validate slide number
		if placeTableSlide < 1 {
			return InvalidArgsError("--slide must be >= 1")
		}

		// Validate data file
		if placeTableFile == "" {
			return InvalidArgsError("--data must be specified")
		}

		if _, err := os.Stat(placeTableFile); err != nil {
			return FileNotFoundError(placeTableFile)
		}

		// Validate format
		if placeTableFormat != "csv" && placeTableFormat != "json" {
			return InvalidArgsError("--format must be 'csv' or 'json'")
		}

		// Validate dimensions
		if placeTableWidth <= 0 {
			return InvalidArgsError(fmt.Sprintf("table width must be positive: cx=%d", placeTableWidth))
		}

		// Get global config
		config := GetGlobalConfig(cmd)

		// Load table data
		tableData, err := loadTableData(placeTableFile, placeTableFormat)
		if err != nil {
			return err
		}

		if len(tableData) == 0 {
			return InvalidArgsError("table data is empty")
		}

		// Perform the table placement
		result, err := performPlaceTable(filePath, placeTableSlide, tableData, mutOpts)
		if err != nil {
			return err
		}

		// Output the result
		if config.Format == "json" {
			return outputPlaceTableJSON(cmd, result)
		}

		return outputPlaceTableText(cmd, result)
	},
}

// loadTableData loads table data from CSV or JSON file
func loadTableData(filePath string, format string) ([][]string, error) {
	file, err := os.Open(filePath)
	if err != nil {
		return nil, fmt.Errorf("failed to open data file: %w", err)
	}
	defer file.Close()

	if format == "csv" {
		return loadTableDataCSV(file)
	} else if format == "json" {
		return loadTableDataJSON(file)
	}

	return nil, fmt.Errorf("unsupported format: %s", format)
}

// loadTableDataCSV loads table data from a CSV file
func loadTableDataCSV(r io.Reader) ([][]string, error) {
	reader := csv.NewReader(r)

	var rows [][]string
	for {
		record, err := reader.Read()
		if err == io.EOF {
			break
		}
		if err != nil {
			return nil, fmt.Errorf("failed to read CSV: %w", err)
		}
		rows = append(rows, record)
	}

	return rows, nil
}

// loadTableDataJSON loads table data from a JSON file
func loadTableDataJSON(r io.Reader) ([][]string, error) {
	var data [][]interface{}
	decoder := json.NewDecoder(r)
	if err := decoder.Decode(&data); err != nil {
		return nil, fmt.Errorf("failed to decode JSON: %w", err)
	}

	rows := make([][]string, len(data))
	for i, row := range data {
		cols := make([]string, len(row))
		for j, cell := range row {
			cols[j] = fmt.Sprintf("%v", cell)
		}
		rows[i] = cols
	}

	return rows, nil
}

// performPlaceTable performs the table placement mutation
func performPlaceTable(
	filePath string,
	slideNumber int,
	tableData [][]string,
	mutOpts *MutationOptions,
) (*placeTableResult, error) {
	// Create mutation writer
	writer, err := NewMutationWriter(filePath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *placeTableResult

	// Perform the mutation
	err = writer.Write(func(pkg opc.PackageSession) error {
		// Parse presentation to get slide references
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return fmt.Errorf("failed to parse presentation: %w", err)
		}

		if slideNumber < 1 || slideNumber > len(graph.Slides) {
			return InvalidArgsError(fmt.Sprintf("slide number %d out of range (1-%d)", slideNumber, len(graph.Slides)))
		}

		slideRef := graph.Slides[slideNumber-1]

		// Create the table insertion request
		insertReq := &mutate.InsertTableRequest{
			Package:         pkg,
			SlideRef:        &slideRef,
			Data:            tableData,
			X:               placeTableX,
			Y:               placeTableY,
			Width:           placeTableWidth,
			Height:          placeTableHeight,
			HasHeader:       placeTableHasHeader,
			HasBandedRows:   placeTableHasBandedRows,
			HeaderFillColor: placeTableHeaderColor,
			BandFill1Color:  placeTableBand1Color,
			BandFill2Color:  placeTableBand2Color,
			DefaultFontSize: placeTableFontSize,
			BorderColor:     placeTableBorderColor,
			BorderWidth:     placeTableBorderWidth,
			ShapeName:       placeTableName,
		}

		// Insert the table
		insertRes, err := mutate.InsertTable(insertReq)
		if err != nil {
			return fmt.Errorf("failed to insert table: %w", err)
		}

		// Build result
		result = &placeTableResult{
			ShapeID:   insertRes.ShapeID,
			ShapeName: insertRes.ShapeName,
			Width:     insertRes.Width,
			Height:    insertRes.Height,
			Rows:      len(tableData),
			Cols:      len(tableData[0]),
		}

		return nil
	})

	if err != nil {
		return nil, err
	}

	return result, nil
}

// placeTableResult holds the result of table placement
type placeTableResult struct {
	ShapeID   int    `json:"shapeId"`
	ShapeName string `json:"shapeName"`
	Width     int64  `json:"width"`
	Height    int64  `json:"height"`
	Rows      int    `json:"rows"`
	Cols      int    `json:"cols"`
}

// outputPlaceTableText outputs the result in text format
func outputPlaceTableText(cmd *cobra.Command, result *placeTableResult) error {
	out := cmd.OutOrStdout()
	fmt.Fprintf(out, "Table placed successfully\n")
	fmt.Fprintf(out, "  Shape ID: %d\n", result.ShapeID)
	fmt.Fprintf(out, "  Shape Name: %s\n", result.ShapeName)
	fmt.Fprintf(out, "  Dimensions: %d×%d EMUs\n", result.Width, result.Height)
	fmt.Fprintf(out, "  Data: %d rows × %d columns\n", result.Rows, result.Cols)
	return nil
}

// outputPlaceTableJSON outputs the result in JSON format
func outputPlaceTableJSON(cmd *cobra.Command, result *placeTableResult) error {
	encoder := json.NewEncoder(cmd.OutOrStdout())
	encoder.SetIndent("", "  ")
	return encoder.Encode(result)
}

// init registers the place table command
func init() {
	// Register as subcommand of 'pptx place'
	// This will be added to the root command in pptx.go

	placeTableCmd.Flags().IntVarP(&placeTableSlide, "slide", "s", 0, "slide number (1-based, required)")
	placeTableCmd.Flags().StringVar(&placeTableFile, "data", "", "path to data file (CSV or JSON, required)")
	placeTableCmd.Flags().StringVar(&placeTableFormat, "format", "csv", "data format: 'csv' or 'json'")
	placeTableCmd.Flags().Int64Var(&placeTableX, "x", 0, "left position in EMUs (default: 0)")
	placeTableCmd.Flags().Int64Var(&placeTableY, "y", 0, "top position in EMUs (default: 0)")
	placeTableCmd.Flags().Int64Var(&placeTableWidth, "cx", 0, "table width in EMUs (required)")
	placeTableCmd.Flags().Int64Var(&placeTableHeight, "cy", 0, "table height in EMUs (optional, auto-calculated if 0)")
	placeTableCmd.Flags().BoolVar(&placeTableHasHeader, "header", false, "first row is header")
	placeTableCmd.Flags().BoolVar(&placeTableHasBandedRows, "banded-rows", false, "alternate row fills")
	placeTableCmd.Flags().StringVar(&placeTableHeaderColor, "header-color", "4472C4", "header background color (hex)")
	placeTableCmd.Flags().StringVar(&placeTableBand1Color, "band1-color", "D9E1F2", "band 1 background color (hex)")
	placeTableCmd.Flags().StringVar(&placeTableBand2Color, "band2-color", "", "band 2 background color (hex, optional)")
	placeTableCmd.Flags().IntVar(&placeTableFontSize, "font-size", 18, "default font size in points")
	placeTableCmd.Flags().StringVar(&placeTableBorderColor, "border-color", "000000", "border color (hex)")
	placeTableCmd.Flags().Int64Var(&placeTableBorderWidth, "border-width", 19050, "border width in EMUs")
	placeTableCmd.Flags().StringVar(&placeTableName, "name", "", "shape name (auto-generated if empty)")

	AddMutationFlags(placeTableCmd)

	// Mark required flags
	placeTableCmd.MarkFlagRequired("slide")
	placeTableCmd.MarkFlagRequired("data")
	placeTableCmd.MarkFlagRequired("cx")
}
