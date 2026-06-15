package cli

import (
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
	placeImageSlide   int
	placeImageFile    string
	placeImageX       int64
	placeImageY       int64
	placeImageCX      int64
	placeImageCY      int64
	placeImageName    string
	placeImageFitMode string
)

var placeImageCmd = &cobra.Command{
	Use:   "image <file>",
	Short: "Place an image on a slide at specific coordinates",
	Long: `Place a new image on a slide at exact EMU coordinates.

Usage:
  ooxml pptx place image <file> --slide <n> --image <path> --x <emus> --y <emus> --cx <emus> --cy <emus> [--name <name>] [--fit-mode <mode>] [--out <output>] [--in-place] [--backup <backup>]

Coordinates:
  All coordinates (x, y, cx, cy) are specified in EMUs (English Metric Units).
  - 1 inch = 914400 EMUs
  - 1 cm = 360000 EMUs
  - x, y: position from top-left
  - cx, cy: width and height

Fit Modes:
  contain (default)    - Fit the image within the specified bounds (stretch to fit)
  cover                - Tile the image to cover the specified bounds (crop as needed)

Output Options:
  --out <path>         - Write to output file (mutually exclusive with --in-place)
  --in-place           - Modify the input file directly (mutually exclusive with --out)
  --backup <path>      - Create backup when using --in-place (optional)

Flags:
  --no-validate        - Skip validation after writing (default: validate)

Examples:
  # Place image at 1 inch from left, 0.5 inch from top, 2x3 inches
  ooxml pptx place image deck.pptx --slide 1 --image photo.png --x 914400 --y 457200 --cx 1828800 --cy 2743200 --out out.pptx
  
  # Place with a custom name
  ooxml pptx place image deck.pptx --slide 2 --image graphic.jpg --x 0 --y 0 --cx 5000000 --cy 5000000 --name "MyGraphic" --in-place`,
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
		if placeImageSlide < 1 {
			return InvalidArgsError("--slide must be >= 1")
		}

		// Validate image file
		if placeImageFile == "" {
			return InvalidArgsError("--image must be specified")
		}

		if _, err := os.Stat(placeImageFile); err != nil {
			return FileNotFoundError(placeImageFile)
		}

		// Validate dimensions
		if placeImageCX <= 0 || placeImageCY <= 0 {
			return InvalidArgsError(fmt.Sprintf("dimensions must be positive: cx=%d, cy=%d", placeImageCX, placeImageCY))
		}

		// Parse fit mode
		fitMode, err := mutate.ParseFitMode(placeImageFitMode)
		if err != nil {
			return InvalidArgsError(err.Error())
		}

		// Get global config
		config := GetGlobalConfig(cmd)

		// Read the image data
		imageData, err := os.ReadFile(placeImageFile)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to read image file: %v", err)
		}

		// Determine content type based on file extension
		contentType, err := getImageContentType(placeImageFile)
		if err != nil {
			return err
		}

		// Perform the image placement
		result, err := performPlaceImage(filePath, placeImageSlide, imageData, contentType, fitMode, placeImageName, mutOpts)
		if err != nil {
			return err
		}

		// Output the result
		if config.Format == "json" {
			return outputPlaceImageJSON(cmd, result)
		}

		return outputPlaceImageText(cmd, result)
	},
}

// performPlaceImage performs the image placement mutation
func performPlaceImage(
	filePath string,
	slideNumber int,
	imageData []byte,
	contentType string,
	fitMode mutate.FitMode,
	shapeName string,
	mutOpts *MutationOptions,
) (*placeImageResult, error) {
	// Create mutation writer
	writer, err := NewMutationWriter(filePath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *placeImageResult
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)

	// Perform the mutation
	err = writer.Write(func(pkg opc.PackageSession) error {
		// Parse presentation to get slide references
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return fmt.Errorf("failed to parse presentation: %w", err)
		}

		// Validate slide number
		if slideNumber < 1 || slideNumber > len(graph.Slides) {
			return fmt.Errorf("invalid slide number %d (presentation has %d slides)", slideNumber, len(graph.Slides))
		}

		// Get the target slide
		slideRef := graph.Slides[slideNumber-1]

		// Create the insert request
		req := &mutate.InsertImageRequest{
			Package:       pkg,
			SlideRef:      &slideRef,
			ImageData:     imageData,
			ContentType:   contentType,
			FitMode:       fitMode,
			X:             placeImageX,
			Y:             placeImageY,
			CX:            placeImageCX,
			CY:            placeImageCY,
			Name:          shapeName,
			InsertAfterID: 0, // Append to end
		}

		// Perform the insertion
		insertResult, err := mutate.InsertImage(req)
		if err != nil {
			return fmt.Errorf("failed to insert image: %w", err)
		}
		destination, err := collectPPTXShapeDestination(pkg, slideNumber, fmt.Sprintf("shape:%d", insertResult.ShapeID), destinationFile, false, true)
		if err != nil {
			return err
		}

		// Store the result
		result = &placeImageResult{
			File:           filePath,
			Output:         destinationFile,
			DryRun:         mutOpts.DryRun,
			SlideNumber:    slideNumber,
			ShapeID:        insertResult.ShapeID,
			ShapeName:      insertResult.ShapeName,
			TargetURI:      insertResult.TargetURI,
			ContentType:    insertResult.ContentType,
			RelationshipID: insertResult.RelationshipID,
			X:              placeImageX,
			Y:              placeImageY,
			CX:             placeImageCX,
			CY:             placeImageCY,
			FitMode:        string(fitMode),
			Destination:    destination,
		}
		result.PPTXBridgeReadbackCommands = pptxShapeMutationReadbackCommands(destination, false, true)

		return nil
	})

	if err != nil {
		return nil, mutationWriteError(err, "failed to place image")
	}

	return result, nil
}

// placeImageResult holds the result of a successful image placement
type placeImageResult struct {
	File           string                `json:"file"`
	Output         string                `json:"output,omitempty"`
	DryRun         bool                  `json:"dryRun"`
	SlideNumber    int                   `json:"slideNumber"`
	ShapeID        int                   `json:"shapeId"`
	ShapeName      string                `json:"shapeName"`
	TargetURI      string                `json:"targetUri"`
	ContentType    string                `json:"contentType"`
	RelationshipID string                `json:"relationshipId"`
	X              int64                 `json:"x"`
	Y              int64                 `json:"y"`
	CX             int64                 `json:"cx"`
	CY             int64                 `json:"cy"`
	FitMode        string                `json:"fitMode"`
	Destination    *PPTXShapeDestination `json:"destination,omitempty"`
	PPTXBridgeReadbackCommands
}

// outputPlaceImageJSON outputs the result in JSON format
func outputPlaceImageJSON(cmd *cobra.Command, result *placeImageResult) error {
	config := GetGlobalConfig(cmd)

	var jsonData []byte
	var err error
	if config.Pretty {
		jsonData, err = json.MarshalIndent(result, "", "  ")
	} else {
		jsonData, err = json.Marshal(result)
	}

	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal JSON: %v", err)
	}

	var outFile io.Writer
	if config.Output != "" {
		file, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer file.Close()
		outFile = file
	} else {
		outFile = cmd.OutOrStdout()
	}

	fmt.Fprintf(outFile, "%s\n", string(jsonData))
	return nil
}

// outputPlaceImageText outputs the result in text format
func outputPlaceImageText(cmd *cobra.Command, result *placeImageResult) error {
	config := GetGlobalConfig(cmd)

	var outFile io.Writer
	if config.Output != "" {
		file, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer file.Close()
		outFile = file
	} else {
		outFile = cmd.OutOrStdout()
	}

	fmt.Fprintf(outFile, "Image placed successfully!\n\n")
	fmt.Fprintf(outFile, "Slide:                %d\n", result.SlideNumber)
	fmt.Fprintf(outFile, "Shape:                %s (ID: %d)\n", result.ShapeName, result.ShapeID)
	fmt.Fprintf(outFile, "Relationship ID:      %s\n", result.RelationshipID)
	fmt.Fprintf(outFile, "Content Type:         %s\n\n", result.ContentType)
	if result.Output != "" {
		fmt.Fprintf(outFile, "Output:               %s\n", result.Output)
	}
	if result.Destination != nil {
		fmt.Fprintf(outFile, "Selector:             %s\n", result.Destination.PrimarySelector)
	}
	fmt.Fprintf(outFile, "Position and Size:\n")
	fmt.Fprintf(outFile, "  X:                  %d EMUs\n", result.X)
	fmt.Fprintf(outFile, "  Y:                  %d EMUs\n", result.Y)
	fmt.Fprintf(outFile, "  Width (cx):         %d EMUs\n", result.CX)
	fmt.Fprintf(outFile, "  Height (cy):        %d EMUs\n", result.CY)
	fmt.Fprintf(outFile, "  Fit Mode:           %s\n", result.FitMode)

	return nil
}

// init registers the place image command
func init() {
	placeImageCmd.Flags().IntVar(
		&placeImageSlide,
		"slide",
		0,
		"1-based slide number",
	)
	placeImageCmd.MarkFlagRequired("slide")

	placeImageCmd.Flags().StringVar(
		&placeImageFile,
		"image",
		"",
		"path to the image file",
	)
	placeImageCmd.MarkFlagRequired("image")

	placeImageCmd.Flags().Int64Var(
		&placeImageX,
		"x",
		0,
		"x position in EMUs",
	)
	placeImageCmd.MarkFlagRequired("x")

	placeImageCmd.Flags().Int64Var(
		&placeImageY,
		"y",
		0,
		"y position in EMUs",
	)
	placeImageCmd.MarkFlagRequired("y")

	placeImageCmd.Flags().Int64Var(
		&placeImageCX,
		"cx",
		0,
		"width in EMUs",
	)
	placeImageCmd.MarkFlagRequired("cx")

	placeImageCmd.Flags().Int64Var(
		&placeImageCY,
		"cy",
		0,
		"height in EMUs",
	)
	placeImageCmd.MarkFlagRequired("cy")

	placeImageCmd.Flags().StringVar(
		&placeImageName,
		"name",
		"",
		"optional name for the image shape",
	)

	placeImageCmd.Flags().StringVar(
		&placeImageFitMode,
		"fit-mode",
		"contain",
		"fit mode: 'contain' (fit within bounds, stretch as needed) or 'cover' (tile to cover bounds)",
	)

	// Add mutation flags
	AddMutationFlags(placeImageCmd)

	placeCmd.AddCommand(placeImageCmd)
}
