package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"sort"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptselectors "github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
)

var slidesSelectorsSlide int

// SlideSelectorsOutput represents the JSON output for slides selectors.
type SlideSelectorsOutput struct {
	File       string                             `json:"file"`
	Slide      int                                `json:"slide"`
	PartURI    string                             `json:"partUri"`
	LayoutName string                             `json:"layoutName,omitempty"`
	LayoutURI  string                             `json:"layoutPartUri,omitempty"`
	Targets    []pptselectors.SlideSelectorTarget `json:"targets"`
}

var slidesSelectorsCmd = &cobra.Command{
	Use:   "selectors <file>",
	Short: "List targetable selectors for a slide",
	Long: `List the targetable selectors for a specific slide, including semantic placeholder keys,
shape selectors, shape names, placeholder metadata, and text capability.

Use this before replace-text or other semantic mutations when you need to know the exact
published selector surface for a real slide.`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if slidesSelectorsSlide < 1 {
			return InvalidArgsError("--slide must be >= 1")
		}

		pkg, err := openPackageExpectType(filePath, opc.PackageTypePPTX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		catalog, err := pptselectors.BuildSlideCatalog(pkg, slidesSelectorsSlide)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to build selector catalog: %v", err)
		}

		output := SlideSelectorsOutput{
			File:       filePath,
			Slide:      catalog.SlideNumber,
			PartURI:    catalog.SlidePartURI,
			LayoutName: catalog.LayoutName,
			LayoutURI:  catalog.LayoutPartURI,
			Targets:    catalog.Targets,
		}

		if GetGlobalConfig(cmd).Format == "json" {
			return outputSlidesSelectorsJSON(cmd, output)
		}
		return outputSlidesSelectorsText(cmd, output)
	},
}

func outputSlidesSelectorsJSON(cmd *cobra.Command, output SlideSelectorsOutput) error {
	config := GetGlobalConfig(cmd)
	var (
		data []byte
		err  error
	)
	if config.Pretty {
		data, err = json.MarshalIndent(output, "", "  ")
	} else {
		data, err = json.Marshal(output)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal selector catalog JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputSlidesSelectorsText(cmd *cobra.Command, output SlideSelectorsOutput) error {
	var builder strings.Builder
	builder.WriteString(fmt.Sprintf("Slide %d: %s\n", output.Slide, output.PartURI))
	if output.LayoutName != "" {
		builder.WriteString(fmt.Sprintf("Layout: %s\n", output.LayoutName))
	}
	builder.WriteString("Selectors:\n")
	for _, target := range output.Targets {
		builder.WriteString(fmt.Sprintf("  [%d] %s", target.Order, target.PrimarySelector))
		if target.ShapeName != "" {
			builder.WriteString(fmt.Sprintf(" (%s)", target.ShapeName))
		}
		builder.WriteString(fmt.Sprintf(" type=%s kind=%s text=%t\n", target.ShapeType, target.TargetKind, target.TextCapable))
		selectors := append([]string(nil), target.Selectors...)
		sort.Strings(selectors)
		builder.WriteString(fmt.Sprintf("      selectors: %s\n", strings.Join(selectors, ", ")))
		builder.WriteString(fmt.Sprintf("      shapeId: %d\n", target.ShapeID))
		if target.Placeholder != nil {
			builder.WriteString(fmt.Sprintf("      placeholder: key=%s", target.Placeholder.Key))
			if target.Placeholder.Role != "" {
				builder.WriteString(fmt.Sprintf(" role=%s", target.Placeholder.Role))
			}
			if target.Placeholder.Index != nil {
				builder.WriteString(fmt.Sprintf(" idx=%d", *target.Placeholder.Index))
			}
			if target.Placeholder.TypeSource != "" {
				builder.WriteString(fmt.Sprintf(" source=%s", target.Placeholder.TypeSource))
			}
			builder.WriteString("\n")
		}
		if target.TextPreview != "" {
			builder.WriteString(fmt.Sprintf("      text: %q\n", target.TextPreview))
		}
	}
	return writeCLIOutput(cmd, []byte(builder.String()))
}

func init() {
	slidesSelectorsCmd.Flags().IntVar(&slidesSelectorsSlide, "slide", 0, "1-based slide number to inspect")
	slidesSelectorsCmd.MarkFlagRequired("slide")
	slidesCmd.AddCommand(slidesSelectorsCmd)
}
