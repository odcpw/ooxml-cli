package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strconv"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
)

var (
	addLayoutPlaceholderLayout string
	addLayoutPlaceholderType   string
	addLayoutPlaceholderBounds string
	addLayoutPlaceholderIdx    int
	addLayoutPlaceholderIdxSet bool
	addLayoutPlaceholderSize   string
	addLayoutPlaceholderOrient string
)

var layoutsAddPlaceholderCmd = &cobra.Command{
	Use:   "add-placeholder <file>",
	Short: "Add a text or picture placeholder to a layout",
	Long: `Add a text or picture placeholder to an existing layout.

Bounds format: x,y,cx,cy (in EMU units, 914400 EMU = 1 inch)

Examples:
  ooxml pptx layouts add-placeholder deck.pptx --layout "Title and Content" --type text --bounds 914400,914400,8229600,914400
  ooxml pptx layouts add-placeholder deck.pptx --layout 2 --type pic --bounds 1828800,1828800,6400000,4800000 --out out.pptx`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		inputPath := args[0]
		if _, err := os.Stat(inputPath); err != nil {
			return FileNotFoundError(inputPath)
		}
		if strings.TrimSpace(addLayoutPlaceholderLayout) == "" {
			return InvalidArgsError("--layout must be specified")
		}
		if strings.TrimSpace(addLayoutPlaceholderType) == "" {
			return InvalidArgsError("--type must be specified (text or pic)")
		}
		if strings.TrimSpace(addLayoutPlaceholderBounds) == "" {
			return InvalidArgsError("--bounds must be specified in format x,y,cx,cy")
		}

		addLayoutPlaceholderIdxSet = cmd.Flags().Changed("idx")

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performAddLayoutPlaceholder(inputPath, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputAddPlaceholderJSON(cmd, result)
		}
		return outputAddPlaceholderText(cmd, result)
	},
}

type addPlaceholderResult struct {
	File      string `json:"file,omitempty"`
	Output    string `json:"output,omitempty"`
	DryRun    bool   `json:"dryRun"`
	Layout    string `json:"layout"`
	LayoutURI string `json:"layoutUri,omitempty"`
	Type      string `json:"type"`
	ShapeID   int    `json:"shapeId"`
	ShapeName string `json:"shapeName"`
	Idx       int    `json:"idx"`
	PPTXLayoutMutationReadbackCommands
}

func performAddLayoutPlaceholder(inputPath string, mutOpts *MutationOptions) (*addPlaceholderResult, error) {
	writer, err := NewMutationWriter(inputPath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *addPlaceholderResult
	destinationFile := mutationOutputPathForResult(inputPath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		// Parse presentation to get layouts
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return fmt.Errorf("failed to parse presentation: %w", err)
		}

		// Find the layout using resolveLayoutSelector
		layoutURI, err := resolveLayoutSelector(graph, addLayoutPlaceholderLayout)
		if err != nil {
			return missingLayoutSelectorError(graph, addLayoutPlaceholderLayout)
		}

		// Find the layout name by looking through the graph
		var layoutName string
		for _, layout := range graph.Layouts {
			if layout.PartURI == layoutURI {
				layoutName = layout.Name
				break
			}
		}
		if layoutName == "" {
			layoutName = addLayoutPlaceholderLayout
		}

		// Parse bounds
		x, y, cx, cy, err := parseBounds(addLayoutPlaceholderBounds)
		if err != nil {
			return fmt.Errorf("invalid bounds: %w", err)
		}

		// Add placeholder based on type
		phType := strings.ToLower(strings.TrimSpace(addLayoutPlaceholderType))
		if phType == "text" {
			phReq := &mutate.AddTextPlaceholderRequest{
				Package:         pkg,
				LayoutPartURI:   layoutURI,
				PlaceholderType: mutate.PlaceholderTypeBody,
				X:               x,
				Y:               y,
				CX:              cx,
				CY:              cy,
				Size:            addLayoutPlaceholderSize,
				Orient:          addLayoutPlaceholderOrient,
				Idx:             addLayoutPlaceholderIdx,
				ExplicitIdx:     addLayoutPlaceholderIdxSet,
			}
			phResult, err := mutate.AddTextPlaceholder(phReq)
			if err != nil {
				return fmt.Errorf("failed to add text placeholder: %w", err)
			}
			result = &addPlaceholderResult{
				File:      inputPath,
				Output:    destinationFile,
				DryRun:    mutOpts.DryRun,
				Layout:    layoutName,
				LayoutURI: layoutURI,
				Type:      "text",
				ShapeID:   phResult.ShapeID,
				ShapeName: phResult.ShapeName,
				Idx:       phResult.Idx,
			}
			result.PPTXLayoutMutationReadbackCommands = pptxLayoutMutationReadbackCommands(destinationFile, layoutName)
		} else if phType == "pic" {
			phReq := &mutate.AddPicturePlaceholderRequest{
				Package:       pkg,
				LayoutPartURI: layoutURI,
				X:             x,
				Y:             y,
				CX:            cx,
				CY:            cy,
				Size:          addLayoutPlaceholderSize,
				Orient:        addLayoutPlaceholderOrient,
				Idx:           addLayoutPlaceholderIdx,
				ExplicitIdx:   addLayoutPlaceholderIdxSet,
			}
			phResult, err := mutate.AddPicturePlaceholder(phReq)
			if err != nil {
				return fmt.Errorf("failed to add picture placeholder: %w", err)
			}
			result = &addPlaceholderResult{
				File:      inputPath,
				Output:    destinationFile,
				DryRun:    mutOpts.DryRun,
				Layout:    layoutName,
				LayoutURI: layoutURI,
				Type:      "pic",
				ShapeID:   phResult.ShapeID,
				ShapeName: phResult.ShapeName,
				Idx:       phResult.Idx,
			}
			result.PPTXLayoutMutationReadbackCommands = pptxLayoutMutationReadbackCommands(destinationFile, layoutName)
		} else {
			return fmt.Errorf("invalid placeholder type %q (must be 'text' or 'pic')", addLayoutPlaceholderType)
		}

		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to add placeholder")
	}

	return result, nil
}

func missingLayoutSelectorError(graph *inspect.PresentationGraph, selector string) error {
	return SelectorNotFoundError("layout", selector, BuildSelectorCandidates(layoutSelectorCandidates(graph), selector, maxSelectorCandidates), "ooxml --json pptx layouts list <file>")
}

func layoutSelectorCandidates(graph *inspect.PresentationGraph) []SelectorCandidate {
	if graph == nil {
		return nil
	}
	out := make([]SelectorCandidate, 0, len(graph.Layouts))
	for i, layout := range graph.Layouts {
		primary := fmt.Sprintf("%d", i+1)
		selectors := []string{primary}
		if layout.Name != "" {
			selectors = append(selectors, layout.Name)
		}
		out = append(out, SelectorCandidate{Primary: primary, Selectors: selectors})
	}
	return out
}

func parseBounds(boundsStr string) (x, y, cx, cy int64, err error) {
	parts := strings.Split(strings.TrimSpace(boundsStr), ",")
	if len(parts) != 4 {
		return 0, 0, 0, 0, fmt.Errorf("expected 4 comma-separated values, got %d", len(parts))
	}

	values := make([]int64, 4)
	for i, part := range parts {
		val, err := strconv.ParseInt(strings.TrimSpace(part), 10, 64)
		if err != nil {
			return 0, 0, 0, 0, fmt.Errorf("invalid value at position %d: %w", i+1, err)
		}
		values[i] = val
	}

	return values[0], values[1], values[2], values[3], nil
}

func outputAddPlaceholderText(cmd *cobra.Command, result *addPlaceholderResult) error {
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

	fmt.Fprintf(outFile, "Added %s placeholder (idx %d, shape ID %d) to layout %s\n",
		result.Type, result.Idx, result.ShapeID, result.Layout)
	return nil
}

func outputAddPlaceholderJSON(cmd *cobra.Command, result *addPlaceholderResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func init() {
	layoutsAddPlaceholderCmd.Flags().StringVar(&addLayoutPlaceholderLayout, "layout", "", "layout number (1-based) or name")
	layoutsAddPlaceholderCmd.Flags().StringVar(&addLayoutPlaceholderType, "type", "", "placeholder type: text or pic")
	layoutsAddPlaceholderCmd.Flags().StringVar(&addLayoutPlaceholderBounds, "bounds", "", "bounds in EMU units: x,y,cx,cy")
	layoutsAddPlaceholderCmd.Flags().IntVar(&addLayoutPlaceholderIdx, "idx", -1, "placeholder index (-1 = auto-allocate; 0 and above are explicit)")
	layoutsAddPlaceholderCmd.Flags().StringVar(&addLayoutPlaceholderSize, "size", "", "optional placeholder size enum (e.g. 'full', 'half')")
	layoutsAddPlaceholderCmd.Flags().StringVar(&addLayoutPlaceholderOrient, "orient", "", "optional placeholder orientation")

	layoutsAddPlaceholderCmd.MarkFlagRequired("layout")
	layoutsAddPlaceholderCmd.MarkFlagRequired("type")
	layoutsAddPlaceholderCmd.MarkFlagRequired("bounds")

	AddMutationFlags(layoutsAddPlaceholderCmd)

	// layoutsCmd should be created by layouts_list.go init()
	// If not, create it here
	if layoutsCmd == nil {
		layoutsCmd = &cobra.Command{
			Use:   "layouts",
			Short: "Inspect slide layouts",
			Long:  "Commands for inspecting slide layouts and their placeholders.",
		}
		pptxCmd.AddCommand(layoutsCmd)
	}

	layoutsCmd.AddCommand(layoutsAddPlaceholderCmd)
}
