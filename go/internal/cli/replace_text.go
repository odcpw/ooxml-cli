package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxhandle "github.com/ooxml-cli/ooxml-cli/pkg/pptx/handle"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	pptselectors "github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
)

var (
	replaceTextSlide        int
	replaceTextTarget       string
	replaceTextValue        string
	replaceTextFilePath     string
	replaceTextMode         string
	replaceRichTextFilePath string
	replaceTextForSlides    string
	// Bullet/list mutation flags
	replaceTextLevel       int
	replaceTextAlignment   string
	replaceTextBulletMode  string
	replaceTextBulletChar  string
	replaceTextAutoNum     string
	replaceTextSpaceBefore int64
	replaceTextSpaceAfter  int64
	replaceTextLineSpacing int64
)

var replaceTextCmd = &cobra.Command{
	Use:   "text <file>",
	Short: "Replace text in a presentation",
	Long: `Replace the text content of a targeted shape or normalized placeholder in a PPTX presentation.

Modes:
  - plain-text (default): Replace all content with plain text, stripping formatting
  - preserve-format: Replace text while preserving paragraph structure and formatting (bold, color, bullets)
  - rich-text: Apply full structured text with properties from a JSON file

Usage:
  ooxml pptx replace text <file> --target <selector-or-handle> (--text <value> | --text-file <path>) [--slide <n> | --for-slides <spec>] [--mode <mode>] [--rich-text-file <path>] [--out <output> | --in-place]

Slide Targeting:
  --slide <n>            Single slide (1-based number)
  --for-slides <spec>    Multiple slides: "1,3,5-7" (ranges and lists supported)
  --target <handle>      Stable shape handle H:pptx/s:<sldId>/shape:n:<id>; supplies slide scope
  --slide or --for-slides is required unless --target is a stable shape handle.

Examples:
  ooxml pptx replace text deck.pptx --slide 1 --target title --text "Quarterly Update" --out out.pptx
  ooxml pptx replace text deck.pptx --slide 2 --target body:1 --text-file body.txt --mode preserve-format --in-place
  ooxml pptx replace text deck.pptx --slide 1 --target title --mode rich-text --rich-text-file content.json --out out.pptx
  ooxml pptx replace text deck.pptx --for-slides "1-3,5" --target title --text "Updated Title" --out out.pptx
  ooxml pptx replace text deck.pptx --target H:pptx/s:257/shape:n:2 --text "Stable target" --out out.pptx`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		slideNumber, err := cmd.Flags().GetInt("slide")
		if err != nil {
			return err
		}
		forSlides, err := cmd.Flags().GetString("for-slides")
		if err != nil {
			return err
		}
		target, err := cmd.Flags().GetString("target")
		if err != nil {
			return err
		}
		mode, err := cmd.Flags().GetString("mode")
		if err != nil {
			return err
		}

		// Check that exactly one of --slide or --for-slides is provided
		slideSpecified := cmd.Flags().Lookup("slide").Changed
		forSlidesSpecified := cmd.Flags().Lookup("for-slides").Changed

		if strings.TrimSpace(target) == "" {
			return InvalidArgsError("--target must be specified")
		}

		// Handle target: the handle's sldId selects the slide, so --slide is
		// optional and ignored for resolution. A handle is single-shape, so
		// --for-slides is meaningless with it.
		targetIsHandle := pptxhandle.IsHandle(target)
		if targetIsHandle {
			if forSlidesSpecified {
				return InvalidArgsError("--for-slides cannot be combined with a handle target (a handle addresses one shape)")
			}
			result, err := performReplaceTextWithMode(filePath, slideNumber, target, mode, cmd, mutOpts)
			if err != nil {
				return err
			}
			config := GetGlobalConfig(cmd)
			if config.Format == "json" {
				return outputReplaceTextJSON(cmd, result)
			}
			return outputReplaceTextText(cmd, result)
		}

		if !slideSpecified && !forSlidesSpecified {
			return InvalidArgsError("must specify either --slide or --for-slides")
		}
		if slideSpecified && forSlidesSpecified {
			return InvalidArgsError("cannot specify both --slide and --for-slides")
		}

		// Single-slide operation
		if slideSpecified {
			if slideNumber < 1 {
				return InvalidArgsError("--slide must be >= 1")
			}
			result, err := performReplaceTextWithMode(filePath, slideNumber, target, mode, cmd, mutOpts)
			if err != nil {
				return err
			}

			config := GetGlobalConfig(cmd)
			if config.Format == "json" {
				return outputReplaceTextJSON(cmd, result)
			}
			return outputReplaceTextText(cmd, result)
		}

		// Batch operation
		return performBatchReplaceText(filePath, forSlides, target, mode, cmd, mutOpts)
	},
}

func resolveReplacementText(cmd *cobra.Command) (string, error) {
	textFlag := cmd.Flags().Lookup("text")
	textFileFlag := cmd.Flags().Lookup("text-file")
	hasInline := textFlag != nil && textFlag.Changed
	hasFile := textFileFlag != nil && textFileFlag.Changed

	if hasInline == hasFile {
		return "", InvalidArgsError("must specify exactly one of --text or --text-file")
	}

	if hasInline {
		value, err := cmd.Flags().GetString("text")
		if err != nil {
			return "", err
		}
		return value, nil
	}

	textFilePath, err := cmd.Flags().GetString("text-file")
	if err != nil {
		return "", err
	}
	data, err := os.ReadFile(textFilePath)
	if err != nil {
		return "", FileNotFoundError(textFilePath)
	}
	return string(data), nil
}

type replaceTextResult struct {
	File        string                `json:"file"`
	Output      string                `json:"output,omitempty"`
	DryRun      bool                  `json:"dryRun"`
	SlideNumber int                   `json:"slideNumber"`
	Target      string                `json:"target"`
	NewText     string                `json:"newText"`
	Mode        string                `json:"mode"`
	Destination *PPTXShapeDestination `json:"destination,omitempty"`
	PPTXBridgeReadbackCommands
}

func performReplaceTextWithMode(filePath string, slideNumber int, target string, mode string, cmd *cobra.Command, mutOpts *MutationOptions) (*replaceTextResult, error) {
	// Default to plain-text mode
	if mode == "" {
		mode = "plain-text"
	}

	writer, err := NewMutationWriter(filePath, mutOpts)
	if err != nil {
		return nil, err
	}

	request := &mutate.ReplaceTextRequest{
		SlideNumber: slideNumber,
		Target:      target,
		Mode:        mode,
	}

	// Resolve text content based on mode
	var resultText string
	switch mode {
	case "plain-text":
		text, err := resolveReplacementText(cmd)
		if err != nil {
			return nil, err
		}
		request.NewText = text
		resultText = text

	case "preserve-format":
		text, err := resolveReplacementText(cmd)
		if err != nil {
			return nil, err
		}
		request.NewText = text
		resultText = text

	case "rich-text":
		richTextFile, err := cmd.Flags().GetString("rich-text-file")
		if err != nil {
			return nil, err
		}
		if richTextFile == "" {
			return nil, InvalidArgsError("--rich-text-file must be specified when using --mode rich-text")
		}
		richTextData, err := os.ReadFile(richTextFile)
		if err != nil {
			return nil, FileNotFoundError(richTextFile)
		}
		var richText model.TextBlockInfo
		if err := json.Unmarshal(richTextData, &richText); err != nil {
			return nil, NewCLIErrorf(ExitUnexpected, "invalid rich-text-file JSON: %v", err)
		}
		request.RichText = &richText
		resultText = richText.PlainText

	default:
		return nil, InvalidArgsError(fmt.Sprintf("unknown mode: %s (must be 'plain-text', 'preserve-format', or 'rich-text')", mode))
	}

	// Build paragraph options from flags
	var paraOpts *mutate.ParagraphMutationOptions
	var bulletOpts *mutate.BulletMutationOptions

	// Check if any paragraph flags were provided
	if replaceTextLevel >= 0 || replaceTextAlignment != "" || replaceTextSpaceBefore > 0 || replaceTextSpaceAfter > 0 || replaceTextLineSpacing > 0 {
		paraOpts = &mutate.ParagraphMutationOptions{}
		if replaceTextLevel >= 0 {
			level := int32(replaceTextLevel)
			if level > 8 {
				return nil, InvalidArgsError(fmt.Sprintf("invalid level: %d (must be 0-8)", level))
			}
			paraOpts.Level = &level
		}
		if replaceTextAlignment != "" {
			paraOpts.Alignment = &replaceTextAlignment
		}
		if replaceTextSpaceBefore > 0 {
			paraOpts.SpaceBefore = &replaceTextSpaceBefore
		}
		if replaceTextSpaceAfter > 0 {
			paraOpts.SpaceAfter = &replaceTextSpaceAfter
		}
		if replaceTextLineSpacing > 0 {
			paraOpts.LineSpacing = &replaceTextLineSpacing
		}
	}

	// Check if any bullet flags were provided
	if replaceTextBulletMode != "" || replaceTextBulletChar != "" || replaceTextAutoNum != "" {
		bulletOpts = &mutate.BulletMutationOptions{
			Mode: replaceTextBulletMode,
		}
		if replaceTextBulletChar != "" {
			bulletOpts.Character = &replaceTextBulletChar
		}
		if replaceTextAutoNum != "" {
			bulletOpts.AutoNumberingScheme = &replaceTextAutoNum
		}
	}

	request.ParagraphOptions = paraOpts
	request.BulletOptions = bulletOpts

	var destination *PPTXShapeDestination
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		request.Package = pkg
		if err := mutate.ReplaceText(request); err != nil {
			if pptxhandle.IsCode(err, pptxhandle.CodeMalformed) ||
				pptxhandle.IsCode(err, pptxhandle.CodeScopeStale) ||
				pptxhandle.IsCode(err, pptxhandle.CodeStale) ||
				pptxhandle.IsCode(err, pptxhandle.CodeAmbiguous) ||
				pptxhandle.IsCode(err, pptxhandle.CodeFormatMismatch) {
				return mapPPTXHandleError(err)
			}
			if strings.Contains(err.Error(), "target not found") {
				catalog, _ := pptselectors.BuildSlideCatalog(pkg, slideNumber)
				return mapPPTXShapeResolveError(err, catalog, target, slideNumber)
			}
			if strings.Contains(err.Error(), "ambiguous target") {
				return InvalidArgsError(err.Error())
			}
			return err
		}
		var err error
		destination, err = collectPPTXShapeDestination(pkg, slideNumber, target, destinationFile, true, true)
		if err != nil {
			return err
		}
		return nil
	}); err != nil {
		if cliErr, ok := AsCLIError(err); ok {
			return nil, cliErr
		}
		return nil, NewCLIErrorf(ExitUnexpected, "failed to replace text: %v", err)
	}

	effectiveSlide := slideNumber
	if destination != nil && destination.Slide > 0 {
		// For a handle target the slide is resolved from the handle's sldId.
		effectiveSlide = destination.Slide
	}
	result := &replaceTextResult{
		File:        filePath,
		Output:      destinationFile,
		DryRun:      mutOpts.DryRun,
		SlideNumber: effectiveSlide,
		Target:      target,
		NewText:     resultText,
		Mode:        mode,
		Destination: destination,
	}
	result.PPTXBridgeReadbackCommands = pptxShapeMutationReadbackCommands(destination, true, true)
	return result, nil
}

func outputReplaceTextJSON(cmd *cobra.Command, result *replaceTextResult) error {
	data, err := marshalWithConfig(GetGlobalConfig(cmd), result)
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputReplaceTextText(cmd *cobra.Command, result *replaceTextResult) error {
	var builder strings.Builder
	builder.WriteString(fmt.Sprintf("Replaced text on slide %d target %s\n", result.SlideNumber, result.Target))
	if result.Output != "" {
		builder.WriteString(fmt.Sprintf("Output: %s\n", result.Output))
	}
	if result.Destination != nil {
		builder.WriteString(fmt.Sprintf("Selector: %s\n", result.Destination.PrimarySelector))
	}
	return writeCLIOutput(cmd, []byte(builder.String()))
}

func performBatchReplaceText(filePath string, forSlides string, target string, mode string, cmd *cobra.Command, mutOpts *MutationOptions) error {
	// Parse slide specification (e.g., "1-3,5,7-9")
	slideNums, err := parseSlideSpec(forSlides)
	if err != nil {
		return InvalidArgsError(fmt.Sprintf("invalid slide specification: %v", err))
	}

	if len(slideNums) == 0 {
		return InvalidArgsError("no valid slides specified in --for-slides")
	}

	// Default to plain-text mode
	if mode == "" {
		mode = "plain-text"
	}

	writer, err := NewMutationWriter(filePath, mutOpts)
	if err != nil {
		return err
	}

	// Resolve replacement text
	var resultText string
	switch mode {
	case "plain-text", "preserve-format":
		text, err := resolveReplacementText(cmd)
		if err != nil {
			return err
		}
		resultText = text

	case "rich-text":
		richTextFile, err := cmd.Flags().GetString("rich-text-file")
		if err != nil {
			return err
		}
		if richTextFile == "" {
			return InvalidArgsError("--rich-text-file must be specified when using --mode rich-text")
		}
		richTextData, err := os.ReadFile(richTextFile)
		if err != nil {
			return FileNotFoundError(richTextFile)
		}
		var richText model.TextBlockInfo
		if err := json.Unmarshal(richTextData, &richText); err != nil {
			return NewCLIErrorf(ExitUnexpected, "invalid rich-text-file JSON: %v", err)
		}
		resultText = richText.PlainText

	default:
		return InvalidArgsError(fmt.Sprintf("unknown mode: %s (must be 'plain-text', 'preserve-format', or 'rich-text')", mode))
	}

	// Build paragraph options from flags
	var paraOpts *mutate.ParagraphMutationOptions
	var bulletOpts *mutate.BulletMutationOptions

	if replaceTextLevel >= 0 || replaceTextAlignment != "" || replaceTextSpaceBefore > 0 || replaceTextSpaceAfter > 0 || replaceTextLineSpacing > 0 {
		paraOpts = &mutate.ParagraphMutationOptions{}
		if replaceTextLevel >= 0 {
			level := int32(replaceTextLevel)
			if level > 8 {
				return InvalidArgsError(fmt.Sprintf("invalid level: %d (must be 0-8)", level))
			}
			paraOpts.Level = &level
		}
		if replaceTextAlignment != "" {
			paraOpts.Alignment = &replaceTextAlignment
		}
		if replaceTextSpaceBefore > 0 {
			paraOpts.SpaceBefore = &replaceTextSpaceBefore
		}
		if replaceTextSpaceAfter > 0 {
			paraOpts.SpaceAfter = &replaceTextSpaceAfter
		}
		if replaceTextLineSpacing > 0 {
			paraOpts.LineSpacing = &replaceTextLineSpacing
		}
	}

	if replaceTextBulletMode != "" || replaceTextBulletChar != "" || replaceTextAutoNum != "" {
		bulletOpts = &mutate.BulletMutationOptions{
			Mode: replaceTextBulletMode,
		}
		if replaceTextBulletChar != "" {
			bulletOpts.Character = &replaceTextBulletChar
		}
		if replaceTextAutoNum != "" {
			bulletOpts.AutoNumberingScheme = &replaceTextAutoNum
		}
	}

	var batchResult *mutate.BatchTextReplaceResult

	// Perform the batch mutation
	err = writer.Write(func(pkg opc.PackageSession) error {
		request := &mutate.BatchTextReplaceRequest{
			Package:          pkg,
			SlideNumbers:     slideNums,
			Target:           target,
			NewText:          resultText,
			Mode:             mode,
			ParagraphOptions: paraOpts,
			BulletOptions:    bulletOpts,
		}

		batchResult = mutate.BatchTextReplace(request)
		if batchResult.FatalError != "" {
			return fmt.Errorf(batchResult.FatalError)
		}

		return nil
	})

	if err != nil {
		return err
	}

	// Output results
	config := GetGlobalConfig(cmd)
	if config.Format == "json" {
		return outputBatchReplaceTextJSON(cmd, batchResult, target)
	}
	return outputBatchReplaceTextText(cmd, batchResult, target)
}

func outputBatchReplaceTextJSON(cmd *cobra.Command, result *mutate.BatchTextReplaceResult, target string) error {
	config := GetGlobalConfig(cmd)

	batchOutput := map[string]interface{}{
		"target":        target,
		"totalSlides":   result.TotalSlides,
		"successCount":  result.SuccessCount,
		"notFoundCount": result.NotFoundCount,
		"errorCount":    result.ErrorCount,
		"results":       result.Results,
	}

	var data []byte
	var err error
	if config.Pretty {
		data, err = json.MarshalIndent(batchOutput, "", "  ")
	} else {
		data, err = json.Marshal(batchOutput)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputBatchReplaceTextText(cmd *cobra.Command, result *mutate.BatchTextReplaceResult, target string) error {
	var message string
	message = fmt.Sprintf("Batch text replacement on target %s\n", target)
	message += fmt.Sprintf("Total slides: %d\n", result.TotalSlides)
	message += fmt.Sprintf("Succeeded: %d\n", result.SuccessCount)
	message += fmt.Sprintf("Not found: %d\n", result.NotFoundCount)
	message += fmt.Sprintf("Errors: %d\n", result.ErrorCount)

	if result.ErrorCount > 0 {
		message += "\nErrors:\n"
		for _, r := range result.Results {
			if r.Error != "" {
				message += fmt.Sprintf("  Slide %d: %s\n", r.SlideNumber, r.Error)
			}
		}
	}

	return writeCLIOutput(cmd, []byte(message))
}

func init() {
	replaceTextCmd.Flags().IntVar(&replaceTextSlide, "slide", 0, "1-based slide number (optional when --target is a shape handle)")
	replaceTextCmd.Flags().StringVar(&replaceTextForSlides, "for-slides", "", "slide specification for batch operations (e.g., '1-3,5,7-9')")
	replaceTextCmd.Flags().StringVar(&replaceTextTarget, "target", "", "target selector or stable shape handle (e.g. title, body:1, shape:3, ~Title 1, or H:pptx/s:<sldId>/shape:n:<id>)")
	replaceTextCmd.Flags().StringVar(&replaceTextValue, "text", "", "replacement text value")
	replaceTextCmd.Flags().StringVar(&replaceTextFilePath, "text-file", "", "path to a file containing replacement text")
	replaceTextCmd.Flags().StringVar(&replaceTextMode, "mode", "plain-text", "replacement mode: plain-text (default), preserve-format, or rich-text")
	replaceTextCmd.Flags().StringVar(&replaceRichTextFilePath, "rich-text-file", "", "path to JSON file containing rich text content (required when --mode is rich-text)")

	// Paragraph/bullet mutation flags
	replaceTextCmd.Flags().IntVar(&replaceTextLevel, "level", -1, "paragraph indent level (0-8, -1 to skip)")
	replaceTextCmd.Flags().StringVar(&replaceTextAlignment, "align", "", "paragraph alignment (l, ctr, r, just, dist)")
	replaceTextCmd.Flags().StringVar(&replaceTextBulletMode, "bullet-mode", "", "bullet mode (buNone, buChar, buAutoNum)")
	replaceTextCmd.Flags().StringVar(&replaceTextBulletChar, "bullet-char", "", "bullet character (e.g. •, -, *)")
	replaceTextCmd.Flags().StringVar(&replaceTextAutoNum, "auto-num", "", "auto-numbering scheme (e.g. stdAutoNum)")
	replaceTextCmd.Flags().Int64Var(&replaceTextSpaceBefore, "space-before", 0, "spacing before paragraph in EMU (0 to skip)")
	replaceTextCmd.Flags().Int64Var(&replaceTextSpaceAfter, "space-after", 0, "spacing after paragraph in EMU (0 to skip)")
	replaceTextCmd.Flags().Int64Var(&replaceTextLineSpacing, "line-spacing", 0, "line spacing in EMU (0 to skip)")

	replaceTextCmd.MarkFlagRequired("target")
	AddMutationFlags(replaceTextCmd)
	replaceCmd.AddCommand(replaceTextCmd)
}
