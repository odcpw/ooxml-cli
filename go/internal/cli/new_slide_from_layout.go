package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
)

var (
	newSlideLayout           string
	newSlideSetTexts         []string
	newSlideSetRichText      []string
	newSlideSetImages        []string
	newSlideSetImageCoords   []string
	newSlideSetImageSlotKeys []string
	newSlideImageFitMode     string
	newSlideInsertAfter      int
	// Paragraph/bullet mutation flags
	newSlideLevel       int
	newSlideAlignment   string
	newSlideBulletMode  string
	newSlideBulletChar  string
	newSlideAutoNum     string
	newSlideSpaceBefore int64
	newSlideSpaceAfter  int64
	newSlideLineSpacing int64
)

var newSlideFromLayoutCmd = &cobra.Command{
	Use:   "new-slide-from-layout <file>",
	Short: "Create a new slide from an existing layout",
	Long: `Create a new slide from an existing layout template, with optional placeholder population and formatting.

Image insertion modes:
  1. Placeholder-based (--set-image key=path)
     Replace an existing picture shape, or fill a picture placeholder by its
     normalized key on the newly created slide.
  
  2. Coordinate-based (--set-image-coords x,y,cx,cy=path)
     Insert images at precise EMU coordinates with explicit dimensions.
     Example: --set-image-coords "914400,914400,1828800,1371600=/path/to/image.jpg"
  
  3. Slot-based (--set-image-slot slotKey=path)
     Fill normalized layout/master slots, including authored picture placeholders
     such as pic:0, pic:1, etc.

Image fit options:
  --image-fit contain   Fit image within bounds (default)
  --image-fit cover     Fill bounds and crop as needed

Text population:
  - --set-text key=value: Simple text assignment
  - --set-rich-text key=/path/to/json: Rich text with formatting (JSON format)
  
  Placeholder keys: title, subtitle, body, body:N (body:1, body:2, etc.)
  Custom keys from layout metadata are also supported.

Formatting options (applied to all filled text):
  - --level <0-8>: Paragraph indent level
  - --align <l|ctr|r|just|dist>: Text alignment
  - --bullet-mode <buNone|buChar|buAutoNum>: Bullet style
  - --bullet-char <char>: Bullet character (•, -, *, etc.)
  - --auto-num <scheme>: Auto-numbering (e.g. stdAutoNum)
  - --space-before <emu>: Space before paragraph in EMU
  - --space-after <emu>: Space after paragraph in EMU
  - --line-spacing <emu>: Line spacing in EMU

Examples:
  ooxml pptx new-slide-from-layout deck.pptx --layout "Title and Content" --set-text title="Agenda" --out out.pptx
  ooxml pptx new-slide-from-layout deck.pptx --layout 2 --set-text title="Title" --set-text "body:1=Body text" --out out.pptx
  ooxml pptx new-slide-from-layout deck.pptx --layout 2 --set-text body="Item 1" --bullet-mode buChar --bullet-char "•" --out out.pptx
  ooxml pptx new-slide-from-layout deck.pptx --layout "7pictures" --set-image-slot pic:0=/tmp/a.jpg --set-image-slot pic:1=/tmp/b.jpg --out out.pptx
  ooxml pptx new-slide-from-layout deck.pptx --layout 1 --set-image-coords "914400,914400,1828800,1371600=/tmp/photo.jpg" --out out.pptx`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		inputPath := args[0]
		if _, err := os.Stat(inputPath); err != nil {
			return FileNotFoundError(inputPath)
		}
		if strings.TrimSpace(newSlideLayout) == "" {
			return InvalidArgsError("--layout must be specified")
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performNewSlideFromLayout(inputPath, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputNewSlideJSON(cmd, result)
		}
		return outputNewSlideText(cmd, result)
	},
}

type newSlideResult struct {
	File                      string                 `json:"file"`
	Output                    string                 `json:"output,omitempty"`
	DryRun                    bool                   `json:"dryRun"`
	Layout                    string                 `json:"layout"`
	InsertAfter               int                    `json:"insertAfter,omitempty"`
	NewSlideNumber            int                    `json:"newSlideNumber"`
	NewSlideID                uint32                 `json:"newSlideId"`
	NewSlideURI               string                 `json:"newSlideUri"`
	Destination               *cloneSlideDestination `json:"destination,omitempty"`
	ReadbackCommand           string                 `json:"readbackCommand,omitempty"`
	SlidesListCommand         string                 `json:"slidesListCommand,omitempty"`
	ValidateCommand           string                 `json:"validateCommand,omitempty"`
	RenderCommand             string                 `json:"renderCommand,omitempty"`
	ReadbackCommandTemplate   string                 `json:"readbackCommandTemplate,omitempty"`
	SlidesListCommandTemplate string                 `json:"slidesListCommandTemplate,omitempty"`
	ValidateCommandTemplate   string                 `json:"validateCommandTemplate,omitempty"`
	RenderCommandTemplate     string                 `json:"renderCommandTemplate,omitempty"`
}

func performNewSlideFromLayout(inputPath string, mutOpts *MutationOptions) (*newSlideResult, error) {
	writer, err := NewMutationWriter(inputPath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *newSlideResult
	destinationFile := mutationOutputPathForResult(inputPath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return err
		}
		layoutURI, err := resolveLayoutSelector(graph, newSlideLayout)
		if err != nil {
			return err
		}
		setTexts, err := parseTextAssignments(newSlideSetTexts)
		if err != nil {
			return err
		}
		setRichTexts, err := parseRichTextAssignments(newSlideSetRichText)
		if err != nil {
			return err
		}
		imageFitMode, err := mutate.ParseFitMode(newSlideImageFitMode)
		if err != nil {
			return InvalidArgsError(err.Error())
		}

		setImages, err := parseImageAssignments(newSlideSetImages, imageFitMode)
		if err != nil {
			return err
		}

		// Parse coordinate-based image insertions
		coordImages, err := parseImageCoordinateAssignments(newSlideSetImageCoords, imageFitMode)
		if err != nil {
			return err
		}
		setImages = append(setImages, coordImages...)

		// Parse slot-key-based image insertions
		slotImages, err := parseImageSlotAssignments(newSlideSetImageSlotKeys, imageFitMode)
		if err != nil {
			return err
		}
		setImages = append(setImages, slotImages...)

		// Build paragraph and bullet options from CLI flags
		var paraOpts *mutate.ParagraphMutationOptions
		var bulletOpts *mutate.BulletMutationOptions

		if newSlideLevel >= 0 || newSlideAlignment != "" || newSlideSpaceBefore != 0 || newSlideSpaceAfter != 0 || newSlideLineSpacing != 0 {
			paraOpts = &mutate.ParagraphMutationOptions{}
			if newSlideLevel >= 0 {
				level := int32(newSlideLevel)
				paraOpts.Level = &level
			}
			if newSlideAlignment != "" {
				paraOpts.Alignment = &newSlideAlignment
			}
			if newSlideSpaceBefore != 0 {
				paraOpts.SpaceBefore = &newSlideSpaceBefore
			}
			if newSlideSpaceAfter != 0 {
				paraOpts.SpaceAfter = &newSlideSpaceAfter
			}
			if newSlideLineSpacing != 0 {
				paraOpts.LineSpacing = &newSlideLineSpacing
			}
		}

		if newSlideBulletMode != "" || newSlideBulletChar != "" || newSlideAutoNum != "" {
			bulletOpts = &mutate.BulletMutationOptions{}
			if newSlideBulletMode != "" {
				bulletOpts.Mode = newSlideBulletMode
			}
			if newSlideBulletChar != "" {
				bulletOpts.Character = &newSlideBulletChar
			}
			if newSlideAutoNum != "" {
				bulletOpts.AutoNumberingScheme = &newSlideAutoNum
			}
		}

		created, err := mutate.NewSlideFromLayout(&mutate.NewSlideFromLayoutRequest{
			Package:          pkg,
			LayoutPartURI:    layoutURI,
			InsertAfter:      newSlideInsertAfter,
			SetTexts:         setTexts,
			SetRichTexts:     setRichTexts,
			SetImages:        setImages,
			ParagraphOptions: paraOpts,
			BulletOptions:    bulletOpts,
		})
		if err != nil {
			return err
		}
		graph, err = inspect.ParsePresentation(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse presentation after creating slide: %v", err)
		}
		destination, err := collectCloneSlideDestination(pkg, graph, destinationFile, created.NewSlideNumber)
		if err != nil {
			return err
		}
		result = &newSlideResult{
			File:           inputPath,
			Output:         destinationFile,
			DryRun:         mutOpts.DryRun,
			Layout:         newSlideLayout,
			InsertAfter:    newSlideInsertAfter,
			NewSlideNumber: created.NewSlideNumber,
			NewSlideID:     created.NewSlideID,
			NewSlideURI:    created.NewSlideURI,
			Destination:    destination,
		}
		if destinationFile == "" {
			placeholder := outputPlaceholder()
			result.ReadbackCommandTemplate = pptxSlideReadbackCommand(placeholder, created.NewSlideNumber)
			result.SlidesListCommandTemplate = cloneSlideSlidesListCommand(placeholder)
			result.ValidateCommandTemplate = pptxValidateCommand(placeholder)
			result.RenderCommandTemplate = pptxRenderCommand(placeholder)
		} else {
			result.ReadbackCommand = pptxSlideReadbackCommand(destinationFile, created.NewSlideNumber)
			result.SlidesListCommand = cloneSlideSlidesListCommand(destinationFile)
			result.ValidateCommand = pptxValidateCommand(destinationFile)
			result.RenderCommand = pptxRenderCommand(destinationFile)
		}
		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to create slide from layout")
	}
	return result, nil
}

func resolveLayoutSelector(graph *inspect.PresentationGraph, selector string) (string, error) {
	if graph == nil {
		return "", fmt.Errorf("presentation graph is required")
	}
	if index, err := strconv.Atoi(selector); err == nil {
		if index < 1 || index > len(graph.Layouts) {
			return "", fmt.Errorf("layout %d not found", index)
		}
		return graph.Layouts[index-1].PartURI, nil
	}
	for _, layout := range graph.Layouts {
		if layout.Name == selector {
			return layout.PartURI, nil
		}
	}
	return "", fmt.Errorf("layout %q not found", selector)
}

func parseTextAssignments(values []string) (map[string]string, error) {
	result := map[string]string{}
	for _, value := range values {
		if value == "" || value == "[]" {
			continue
		}
		parts := strings.SplitN(value, "=", 2)
		if len(parts) != 2 || strings.TrimSpace(parts[0]) == "" {
			return nil, fmt.Errorf("invalid --set-text value %q (expected key=value)", value)
		}
		result[strings.TrimSpace(parts[0])] = parts[1]
	}
	return result, nil
}

func parseRichTextAssignments(values []string) ([]mutate.NewSlideRichTextFill, error) {
	fills := make([]mutate.NewSlideRichTextFill, 0, len(values))
	for _, value := range values {
		if value == "" || value == "[]" {
			continue
		}
		parts := strings.SplitN(value, "=", 2)
		if len(parts) != 2 || strings.TrimSpace(parts[0]) == "" {
			return nil, fmt.Errorf("invalid --set-rich-text value %q (expected key=path)", value)
		}
		jsonPath := parts[1]
		data, err := os.ReadFile(jsonPath)
		if err != nil {
			return nil, FileNotFoundError(jsonPath)
		}
		var richText model.TextBlockInfo
		if err := json.Unmarshal(data, &richText); err != nil {
			return nil, fmt.Errorf("failed to parse rich text JSON from %q: %w", jsonPath, err)
		}
		fills = append(fills, mutate.NewSlideRichTextFill{
			Target:   strings.TrimSpace(parts[0]),
			RichText: &richText,
		})
	}
	return fills, nil
}

func parseImageAssignments(values []string, fitMode mutate.FitMode) ([]mutate.NewSlideImageFill, error) {
	fills := make([]mutate.NewSlideImageFill, 0, len(values))
	for _, value := range values {
		if value == "" || value == "[]" {
			continue
		}
		parts := strings.SplitN(value, "=", 2)
		if len(parts) != 2 || strings.TrimSpace(parts[0]) == "" {
			return nil, fmt.Errorf("invalid --set-image value %q (expected key=path)", value)
		}
		imagePath := parts[1]
		data, err := os.ReadFile(imagePath)
		if err != nil {
			return nil, FileNotFoundError(imagePath)
		}
		contentType, err := getImageContentType(imagePath)
		if err != nil {
			return nil, err
		}
		fills = append(fills, mutate.NewSlideImageFill{
			Target:      strings.TrimSpace(parts[0]),
			ImageData:   data,
			ContentType: contentType,
			FitMode:     fitMode,
		})
	}
	return fills, nil
}

func parseImageCoordinateAssignments(values []string, fitMode mutate.FitMode) ([]mutate.NewSlideImageFill, error) {
	fills := make([]mutate.NewSlideImageFill, 0, len(values))
	for _, value := range values {
		if value == "" || value == "[]" {
			continue
		}
		parts := strings.SplitN(value, "=", 2)
		if len(parts) != 2 {
			return nil, fmt.Errorf("invalid --set-image-coords value %q (expected x,y,cx,cy=path)", value)
		}

		coordStr := strings.TrimSpace(parts[0])
		imagePath := parts[1]

		coords := strings.Split(coordStr, ",")
		if len(coords) != 4 {
			return nil, fmt.Errorf("invalid --set-image-coords coordinates %q (expected x,y,cx,cy)", coordStr)
		}

		var x, y, cx, cy int64
		var err error
		if x, err = strconv.ParseInt(strings.TrimSpace(coords[0]), 10, 64); err != nil {
			return nil, fmt.Errorf("invalid x coordinate %q: %w", coords[0], err)
		}
		if y, err = strconv.ParseInt(strings.TrimSpace(coords[1]), 10, 64); err != nil {
			return nil, fmt.Errorf("invalid y coordinate %q: %w", coords[1], err)
		}
		if cx, err = strconv.ParseInt(strings.TrimSpace(coords[2]), 10, 64); err != nil {
			return nil, fmt.Errorf("invalid cx (width) %q: %w", coords[2], err)
		}
		if cy, err = strconv.ParseInt(strings.TrimSpace(coords[3]), 10, 64); err != nil {
			return nil, fmt.Errorf("invalid cy (height) %q: %w", coords[3], err)
		}

		data, err := os.ReadFile(imagePath)
		if err != nil {
			return nil, FileNotFoundError(imagePath)
		}
		contentType, err := getImageContentType(imagePath)
		if err != nil {
			return nil, err
		}

		fills = append(fills, mutate.NewSlideImageFill{
			Target:           "coords",
			ImageData:        data,
			ContentType:      contentType,
			FitMode:          fitMode,
			CoordinateX:      x,
			CoordinateY:      y,
			CoordinateWidth:  cx,
			CoordinateHeight: cy,
		})
	}
	return fills, nil
}

func parseImageSlotAssignments(values []string, fitMode mutate.FitMode) ([]mutate.NewSlideImageFill, error) {
	fills := make([]mutate.NewSlideImageFill, 0, len(values))
	for _, value := range values {
		if value == "" || value == "[]" {
			continue
		}
		parts := strings.SplitN(value, "=", 2)
		if len(parts) != 2 || strings.TrimSpace(parts[0]) == "" {
			return nil, fmt.Errorf("invalid --set-image-slot value %q (expected slotKey=path)", value)
		}

		slotKey := strings.TrimSpace(parts[0])
		imagePath := parts[1]
		data, err := os.ReadFile(imagePath)
		if err != nil {
			return nil, FileNotFoundError(imagePath)
		}
		contentType, err := getImageContentType(imagePath)
		if err != nil {
			return nil, err
		}

		fills = append(fills, mutate.NewSlideImageFill{
			Target:      "slot:" + slotKey,
			ImageData:   data,
			ContentType: contentType,
			FitMode:     fitMode,
		})
	}
	return fills, nil
}

func outputNewSlideJSON(cmd *cobra.Command, result *newSlideResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal new-slide JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputNewSlideText(cmd *cobra.Command, result *newSlideResult) error {
	text := fmt.Sprintf("Created slide %d from layout %s\n", result.NewSlideNumber, result.Layout)
	return writeCLIOutput(cmd, []byte(text))
}

func init() {
	newSlideFromLayoutCmd.Flags().StringVar(&newSlideLayout, "layout", "", "layout number or exact layout name")
	newSlideFromLayoutCmd.Flags().StringArrayVar(&newSlideSetTexts, "set-text", nil, "placeholder text assignment (repeatable key=value)")
	newSlideFromLayoutCmd.Flags().StringArrayVar(&newSlideSetRichText, "set-rich-text", nil, "placeholder rich text assignment from JSON file (repeatable key=path)")
	newSlideFromLayoutCmd.Flags().StringArrayVar(&newSlideSetImages, "set-image", nil, "placeholder image assignment (repeatable key=path)")
	newSlideFromLayoutCmd.Flags().StringArrayVar(&newSlideSetImageCoords, "set-image-coords", nil, "coordinate-based image insertion (repeatable x,y,cx,cy=path)")
	newSlideFromLayoutCmd.Flags().StringArrayVar(&newSlideSetImageSlotKeys, "set-image-slot", nil, "layout/master slot-based image insertion, including authored picture placeholders (repeatable slotKey=path)")
	newSlideFromLayoutCmd.Flags().StringVar(&newSlideImageFitMode, "image-fit", "contain", "image fit mode: contain or cover")
	newSlideFromLayoutCmd.Flags().IntVar(&newSlideInsertAfter, "insert-after", 0, "insert after this 1-based slide number (default: append at end)")

	// Paragraph/bullet mutation flags
	newSlideFromLayoutCmd.Flags().IntVar(&newSlideLevel, "level", -1, "paragraph indent level (0-8, -1 to skip)")
	newSlideFromLayoutCmd.Flags().StringVar(&newSlideAlignment, "align", "", "paragraph alignment (l, ctr, r, just, dist)")
	newSlideFromLayoutCmd.Flags().StringVar(&newSlideBulletMode, "bullet-mode", "", "bullet mode (buNone, buChar, buAutoNum)")
	newSlideFromLayoutCmd.Flags().StringVar(&newSlideBulletChar, "bullet-char", "", "bullet character (e.g. •, -, *)")
	newSlideFromLayoutCmd.Flags().StringVar(&newSlideAutoNum, "auto-num", "", "auto-numbering scheme (e.g. stdAutoNum)")
	newSlideFromLayoutCmd.Flags().Int64Var(&newSlideSpaceBefore, "space-before", 0, "spacing before paragraph in EMU (0 to skip)")
	newSlideFromLayoutCmd.Flags().Int64Var(&newSlideSpaceAfter, "space-after", 0, "spacing after paragraph in EMU (0 to skip)")
	newSlideFromLayoutCmd.Flags().Int64Var(&newSlideLineSpacing, "line-spacing", 0, "line spacing in EMU (0 to skip)")

	newSlideFromLayoutCmd.MarkFlagRequired("layout")
	AddMutationFlags(newSlideFromLayoutCmd)
	pptxCmd.AddCommand(newSlideFromLayoutCmd)
}
