package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
)

var (
	cloneSlideNumber int
	cloneInsertAfter int
)

var cloneSlideCmd = &cobra.Command{
	Use:   "clone-slide <file>",
	Short: "Clone a slide within a presentation",
	Long: `Clone a slide within a presentation and return the new slide handles.

JSON output includes source and destination slide summaries, slide counts,
readback commands for written outputs, and readback command templates for
dry-runs.`,
	Example: `  ooxml --json pptx clone-slide deck.pptx --slide 1 --out edited.pptx
  ooxml --json pptx clone-slide deck.pptx --slide 1 --insert-after 3 --out edited.pptx
  ooxml --json pptx clone-slide deck.pptx --slide 1 --dry-run`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		inputPath := args[0]
		if _, err := os.Stat(inputPath); err != nil {
			return FileNotFoundError(inputPath)
		}
		if cloneSlideNumber < 1 {
			return InvalidArgsError("--slide must be >= 1")
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performCloneSlide(inputPath, mutOpts)
		if err != nil {
			return err
		}

		if GetGlobalConfig(cmd).Format == "json" {
			return outputCloneSlideJSON(cmd, result)
		}
		return outputCloneSlideText(cmd, result)
	},
}

type cloneSlideResult struct {
	File                      string                 `json:"file"`
	Output                    string                 `json:"output,omitempty"`
	DryRun                    bool                   `json:"dryRun"`
	SourceSlide               int                    `json:"sourceSlide"`
	InsertAfter               int                    `json:"insertAfter"`
	SlideCountBefore          int                    `json:"slideCountBefore"`
	SlideCountAfter           int                    `json:"slideCountAfter"`
	NewSlideNumber            int                    `json:"newSlideNumber"`
	NewSlideID                uint32                 `json:"newSlideId"`
	NewSlideURI               string                 `json:"newSlideUri"`
	NotesURI                  string                 `json:"notesUri,omitempty"`
	Source                    *cloneSlideDestination `json:"source,omitempty"`
	Destination               *cloneSlideDestination `json:"destination,omitempty"`
	ReadbackCommand           string                 `json:"readbackCommand,omitempty"`
	SlidesListCommand         string                 `json:"slidesListCommand,omitempty"`
	ValidateCommand           string                 `json:"validateCommand,omitempty"`
	RenderCommand             string                 `json:"renderCommand,omitempty"`
	ReadbackCommandTemplate   string                 `json:"readbackCommandTemplate,omitempty"`
	SlidesListCommandTemplate string                 `json:"slidesListCommandTemplate,omitempty"`
}

type cloneSlideDestination struct {
	File          string `json:"file,omitempty"`
	Number        int    `json:"number"`
	PartURI       string `json:"partUri"`
	Layout        string `json:"layout,omitempty"`
	LayoutPartURI string `json:"layoutPartUri,omitempty"`
	NotesPartURI  string `json:"notesPartUri,omitempty"`
	TextShapes    int    `json:"textShapes"`
	Images        int    `json:"images"`
	Tables        int    `json:"tables"`
	Notes         bool   `json:"notes"`
}

func performCloneSlide(inputPath string, mutOpts *MutationOptions) (*cloneSlideResult, error) {
	writer, err := NewMutationWriter(inputPath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *cloneSlideResult
	destinationFile := mutationOutputPathForResult(inputPath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		beforeGraph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse source presentation: %v", err)
		}
		source, err := collectCloneSlideDestination(pkg, beforeGraph, inputPath, cloneSlideNumber)
		if err != nil {
			return err
		}
		cloned, err := mutate.CloneSlide(&mutate.CloneSlideRequest{
			Package:     pkg,
			SlideNumber: cloneSlideNumber,
			InsertAfter: cloneInsertAfter,
		})
		if err != nil {
			return mapCloneSlideMutationError(err)
		}
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse cloned presentation: %v", err)
		}
		destination, err := collectCloneSlideDestination(pkg, graph, destinationFile, cloned.NewSlideNumber)
		if err != nil {
			return err
		}
		insertAfter := cloneInsertAfter
		if insertAfter == 0 {
			insertAfter = cloneSlideNumber
		}
		result = &cloneSlideResult{
			File:             inputPath,
			Output:           destinationFile,
			DryRun:           mutOpts.DryRun,
			SourceSlide:      cloneSlideNumber,
			InsertAfter:      insertAfter,
			NewSlideNumber:   cloned.NewSlideNumber,
			NewSlideID:       cloned.NewSlideID,
			NewSlideURI:      cloned.NewSlideURI,
			NotesURI:         cloned.NotesURI,
			SlideCountBefore: len(beforeGraph.Slides),
			SlideCountAfter:  len(graph.Slides),
			Source:           source,
			Destination:      destination,
		}
		if destinationFile == "" {
			result.ReadbackCommandTemplate = cloneSlideReadbackCommand(outputPlaceholder(), cloned.NewSlideNumber)
			result.SlidesListCommandTemplate = cloneSlideSlidesListCommand(outputPlaceholder())
		} else {
			result.ReadbackCommand = cloneSlideReadbackCommand(destinationFile, cloned.NewSlideNumber)
			result.SlidesListCommand = cloneSlideSlidesListCommand(destinationFile)
			result.ValidateCommand = fmt.Sprintf("ooxml validate --strict %s", pptxXLSXCommandArg(destinationFile))
			result.RenderCommand = fmt.Sprintf("ooxml pptx render %s --out render-check", pptxXLSXCommandArg(destinationFile))
		}
		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to clone slide")
	}
	return result, nil
}

func cloneSlideReadbackCommand(filePath string, slideNumber int) string {
	return fmt.Sprintf("ooxml --json pptx slides show %s --slide %d --include-text --include-bounds", pptxXLSXCommandArg(filePath), slideNumber)
}

func cloneSlideSlidesListCommand(filePath string) string {
	return fmt.Sprintf("ooxml --json pptx slides list %s", pptxXLSXCommandArg(filePath))
}

func collectCloneSlideDestination(pkg opc.PackageSession, graph *inspect.PresentationGraph, destinationFile string, slideNumber int) (*cloneSlideDestination, error) {
	if graph == nil {
		return nil, NewCLIErrorf(ExitUnexpected, "presentation graph is nil")
	}
	if slideNumber < 1 || slideNumber > len(graph.Slides) {
		return nil, TargetNotFoundError(fmt.Sprintf("slide %d", slideNumber))
	}
	slideRef := graph.Slides[slideNumber-1]
	slideDoc, err := pkg.ReadXMLPart(slideRef.PartURI)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to read cloned slide: %v", err)
	}
	textShapes, images, tables := 0, 0, 0
	if spTree := findPPTXShapeTree(slideDoc.Root()); spTree != nil {
		textShapes, images, tables = countPPTXSlideShapeTypes(spTree)
	}
	layoutName := ""
	for _, layout := range graph.Layouts {
		if layout.PartURI == slideRef.LayoutPartURI {
			layoutName = layout.Name
			break
		}
	}
	return &cloneSlideDestination{
		File:          destinationFile,
		Number:        slideRef.SlideNumber,
		PartURI:       slideRef.PartURI,
		Layout:        layoutName,
		LayoutPartURI: slideRef.LayoutPartURI,
		NotesPartURI:  slideRef.NotesPartURI,
		TextShapes:    textShapes,
		Images:        images,
		Tables:        tables,
		Notes:         slideRef.NotesPartURI != "",
	}, nil
}

func mapCloneSlideMutationError(err error) error {
	if err == nil {
		return nil
	}
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	msg := err.Error()
	switch {
	case strings.Contains(msg, "slide number must be"), strings.Contains(msg, "insert-after"):
		return InvalidArgsError(msg)
	case strings.Contains(msg, "slide ") && strings.Contains(msg, " not found"):
		return TargetNotFoundError(msg)
	default:
		return err
	}
}

func outputCloneSlideJSON(cmd *cobra.Command, result *cloneSlideResult) error {
	data, err := marshalWithConfig(GetGlobalConfig(cmd), result)
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal clone-slide JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputCloneSlideText(cmd *cobra.Command, result *cloneSlideResult) error {
	text := fmt.Sprintf("Cloned slide %d to slide %d\n", result.SourceSlide, result.NewSlideNumber)
	return writeCLIOutput(cmd, []byte(text))
}

func init() {
	cloneSlideCmd.Flags().IntVar(&cloneSlideNumber, "slide", 0, "1-based source slide number")
	cloneSlideCmd.Flags().IntVar(&cloneInsertAfter, "insert-after", 0, "insert the clone after this 1-based slide number (defaults to the source slide)")
	cloneSlideCmd.MarkFlagRequired("slide")
	AddMutationFlags(cloneSlideCmd)
	pptxCmd.AddCommand(cloneSlideCmd)
}
