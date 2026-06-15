package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
)

var (
	addTextboxSlide      int
	addTextboxText       string
	addTextboxX          int64
	addTextboxY          int64
	addTextboxCX         int64
	addTextboxCY         int64
	addTextboxName       string
	addTextboxMode       string // "plain" or "rich"
	addTextboxFontSize   float64
	addTextboxFontFamily string
	addTextboxBold       bool
	addTextboxItalic     bool
	addTextboxColor      string
	addTextboxLevel      int
	addTextboxAlign      string
)

type AddTextboxCLIResult struct {
	File        string                `json:"file"`
	Output      string                `json:"output,omitempty"`
	DryRun      bool                  `json:"dryRun"`
	ShapeID     int                   `json:"shapeId"`
	ShapeName   string                `json:"shapeName"`
	CreatedAt   string                `json:"createdAt"`
	Destination *PPTXShapeDestination `json:"destination,omitempty"`
	PPTXBridgeReadbackCommands
}

var addTextboxCmd = &cobra.Command{
	Use:   "add-textbox <file>",
	Short: "Insert a text box at specific coordinates on a slide",
	Long: `Insert a new text box shape on a slide at the specified EMU coordinates.

The text box is created with the provided text content, positioned at (x, y) with
dimensions (cx, cy) in EMUs (English Metric Units, 1/914400 inch).

Supports plain text or rich text with optional formatting (font, size, color, bold, italic).

Flags:
  --slide <n>       Slide number (1-based, required)
  --text <string>   Text content (required)
  --x <emu>         Left position in EMUs (default: 0)
  --y <emu>         Top position in EMUs (default: 0)
  --cx <emu>        Width in EMUs (required)
  --cy <emu>        Height in EMUs (required)
  --name <string>   Text box name (default: auto-generated)
  --font-size <pt>  Font size in points (default: 18)
  --font <family>   Font family (default: Calibri)
  --bold            Make text bold
  --italic          Make text italic
  --color <rgb>     Text color as RGB hex (e.g., FF0000 for red)
  --level <n>       Paragraph indent level (0-8)
  --align <type>    Text alignment: l/ctr/r/just (default: l)`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]

		if addTextboxSlide < 1 {
			return NewCLIErrorf(ExitInvalidArgs, "--slide must be >= 1")
		}
		if addTextboxText == "" {
			return NewCLIErrorf(ExitInvalidArgs, "--text is required")
		}
		if addTextboxCX <= 0 || addTextboxCY <= 0 {
			return NewCLIErrorf(ExitInvalidArgs, "--cx and --cy must be positive")
		}
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		var result *mutate.InsertTextBoxResult
		var destination *PPTXShapeDestination
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		writer, err := NewMutationWriter(filePath, mutOpts)
		if err != nil {
			return mutationWriteError(err, "failed to prepare mutation writer")
		}

		if err := writer.Write(func(session opc.PackageSession) error {
			graph, err := inspect.ParsePresentation(session)
			if err != nil {
				return fmt.Errorf("failed to parse presentation: %w", err)
			}
			if addTextboxSlide > len(graph.Slides) {
				return fmt.Errorf("slide %d not found (presentation has %d slides)", addTextboxSlide, len(graph.Slides))
			}

			slideRef := graph.Slides[addTextboxSlide-1]
			richText := buildRichTextFromFlags()

			if addTextboxFontFamily == "" {
				addTextboxFontFamily = "Calibri"
			}
			if len(richText.Paragraphs) > 0 {
				for _, para := range richText.Paragraphs {
					for _, run := range para.Runs {
						switch r := run.(type) {
						case *model.TextRun:
							if r != nil && r.Properties != nil {
								applyFlagsToRunProperties(r.Properties)
							}
						}
					}
				}
			}

			textBoxReq := &mutate.InsertTextBoxRequest{
				Package:   session,
				SlideRef:  &slideRef,
				RichText:  richText,
				X:         addTextboxX,
				Y:         addTextboxY,
				CX:        addTextboxCX,
				CY:        addTextboxCY,
				ShapeName: addTextboxName,
			}

			result, err = mutate.InsertTextBox(textBoxReq)
			if err != nil {
				return fmt.Errorf("failed to insert text box: %w", err)
			}
			destination, err = collectPPTXShapeDestination(session, addTextboxSlide, fmt.Sprintf("shape:%d", result.ShapeID), destinationFile, true, true)
			if err != nil {
				return err
			}
			return nil
		}); err != nil {
			return mutationWriteError(err, "failed to insert text box")
		}

		cliResult := &AddTextboxCLIResult{
			File:        filePath,
			Output:      destinationFile,
			DryRun:      mutOpts.DryRun,
			ShapeID:     result.ShapeID,
			ShapeName:   result.ShapeName,
			CreatedAt:   result.CreatedAt.Format("2006-01-02T15:04:05Z07:00"),
			Destination: destination,
		}
		cliResult.PPTXBridgeReadbackCommands = pptxShapeMutationReadbackCommands(destination, true, true)

		config := GetGlobalConfig(cmd)
		if config.Format == "json" {
			data, err := marshalWithConfig(config, cliResult)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to marshal add-textbox JSON: %v", err)
			}
			return writeCLIOutput(cmd, data)
		}

		var builder strings.Builder
		builder.WriteString("Text box inserted\n")
		builder.WriteString(fmt.Sprintf("  Shape ID: %d\n", cliResult.ShapeID))
		builder.WriteString(fmt.Sprintf("  Name: %s\n", cliResult.ShapeName))
		builder.WriteString(fmt.Sprintf("  Position: (%d, %d) EMUs\n", addTextboxX, addTextboxY))
		builder.WriteString(fmt.Sprintf("  Size: %d x %d EMUs\n", addTextboxCX, addTextboxCY))
		if cliResult.Output != "" {
			builder.WriteString(fmt.Sprintf("  Output: %s\n", cliResult.Output))
		}
		if cliResult.Destination != nil {
			builder.WriteString(fmt.Sprintf("  Selector: %s\n", cliResult.Destination.PrimarySelector))
		}

		return writeCLIOutput(cmd, []byte(builder.String()))
	},
}

// buildRichTextFromFlags creates a simple TextBlockInfo with one paragraph from the text flag
func buildRichTextFromFlags() *model.TextBlockInfo {
	textInfo := &model.TextBlockInfo{
		Paragraphs: []model.Paragraph{},
		PlainText:  addTextboxText,
	}

	// Create one paragraph
	para := model.Paragraph{
		Runs: []interface{}{},
	}

	// Set paragraph properties
	level := int32(addTextboxLevel)
	if addTextboxLevel > 0 {
		para.Properties = &model.ParagraphProperties{
			Level: &level,
		}
	}

	if addTextboxAlign != "" {
		if para.Properties == nil {
			para.Properties = &model.ParagraphProperties{}
		}
		para.Properties.Alignment = addTextboxAlign
	}

	// Create one text run with the text
	fontSize := addTextboxFontSize
	if fontSize <= 0 {
		fontSize = 18 // Default to 18pt
	}

	bold := addTextboxBold
	italic := addTextboxItalic

	run := &model.TextRun{
		Text: addTextboxText,
		Properties: &model.RunProperties{
			FontSize:   &fontSize,
			FontFamily: addTextboxFontFamily,
			Bold:       &bold,
			Italic:     &italic,
			Color:      addTextboxColor,
			Language:   "en-US",
		},
	}

	para.Runs = append(para.Runs, run)
	textInfo.Paragraphs = append(textInfo.Paragraphs, para)

	return textInfo
}

// applyFlagsToRunProperties applies CLI flags to a RunProperties struct
func applyFlagsToRunProperties(props *model.RunProperties) {
	if addTextboxFontSize > 0 {
		props.FontSize = &addTextboxFontSize
	}

	if addTextboxFontFamily != "" {
		props.FontFamily = addTextboxFontFamily
	}

	if addTextboxBold {
		t := true
		props.Bold = &t
	}

	if addTextboxItalic {
		t := true
		props.Italic = &t
	}

	if addTextboxColor != "" {
		props.Color = addTextboxColor
	}
}

// init registers the add-textbox command and its flags
func init() {
	addTextboxCmd.Flags().IntVar(&addTextboxSlide, "slide", 0, "Slide number (1-based, required)")
	addTextboxCmd.Flags().StringVar(&addTextboxText, "text", "", "Text content (required)")
	addTextboxCmd.Flags().Int64Var(&addTextboxX, "x", 0, "Left position in EMUs (default: 0)")
	addTextboxCmd.Flags().Int64Var(&addTextboxY, "y", 0, "Top position in EMUs (default: 0)")
	addTextboxCmd.Flags().Int64Var(&addTextboxCX, "cx", 0, "Width in EMUs (required)")
	addTextboxCmd.Flags().Int64Var(&addTextboxCY, "cy", 0, "Height in EMUs (required)")
	addTextboxCmd.Flags().StringVar(&addTextboxName, "name", "", "Text box name (default: auto-generated)")
	addTextboxCmd.Flags().StringVar(&addTextboxMode, "mode", "plain", "Mode: plain or rich (default: plain)")
	addTextboxCmd.Flags().Float64Var(&addTextboxFontSize, "font-size", 18, "Font size in points (default: 18)")
	addTextboxCmd.Flags().StringVar(&addTextboxFontFamily, "font", "Calibri", "Font family (default: Calibri)")
	addTextboxCmd.Flags().BoolVar(&addTextboxBold, "bold", false, "Make text bold")
	addTextboxCmd.Flags().BoolVar(&addTextboxItalic, "italic", false, "Make text italic")
	addTextboxCmd.Flags().StringVar(&addTextboxColor, "color", "", "Text color as RGB hex (e.g., FF0000)")
	addTextboxCmd.Flags().IntVar(&addTextboxLevel, "level", 0, "Paragraph indent level (0-8)")
	addTextboxCmd.Flags().StringVar(&addTextboxAlign, "align", "", "Text alignment: l/ctr/r/just")

	// Mark required flags
	addTextboxCmd.MarkFlagRequired("slide")
	addTextboxCmd.MarkFlagRequired("text")
	addTextboxCmd.MarkFlagRequired("cx")
	addTextboxCmd.MarkFlagRequired("cy")

	AddMutationFlags(addTextboxCmd)

	// Add to pptx command group
	pptxCmd.AddCommand(addTextboxCmd)
}
