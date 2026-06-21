package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
)

// ThemeUpdateResult represents the result of a theme update operation
type ThemeUpdateResult struct {
	UpdatedColors []ThemeUpdateColorResult `json:"colors,omitempty"`
	UpdatedFonts  *ThemeUpdateFontResult   `json:"fonts,omitempty"`
	Message       string                   `json:"message,omitempty"`
	NoOpMessage   string                   `json:"noOpMessage,omitempty"`
}

type ThemeUpdateColorResult struct {
	ColorName string `json:"colorName"`
	HexValue  string `json:"hexValue"`
	Mode      string `json:"mode"`
}

type ThemeUpdateFontResult struct {
	MajorFont string `json:"majorFont,omitempty"`
	MinorFont string `json:"minorFont,omitempty"`
	Mode      string `json:"mode"`
}

var (
	themeUpdateColorsFlag []string
	themeUpdateMajorFont  string
	themeUpdateMinorFont  string
	themeUpdateMode       string
	themeUpdateForSlides  string
	themeUpdateSlide      int
)

var themeUpdateCmd = &cobra.Command{
	Use:   "update <file>",
	Short: "Update theme colors and fonts",
	Long: `Update the theme colors and/or fonts in a PPTX presentation.

The update can be applied to:
  - The entire deck (deck mode, default): Changes the theme itself, affecting all slides
  - Specific slides (slide mode): Applies color overrides to individual slides

Examples:
  ooxml pptx theme update deck.pptx --color "accent1=FF0000" --out out.pptx
  ooxml pptx theme update deck.pptx --slide 1 --color "accent1=FF0000" --mode slide --out out.pptx`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		inputPath := args[0]
		if _, err := os.Stat(inputPath); err != nil {
			return FileNotFoundError(inputPath)
		}

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		if len(themeUpdateColorsFlag) == 0 && themeUpdateMajorFont == "" && themeUpdateMinorFont == "" {
			return InvalidArgsError("no updates specified; use --color, --major-font, or --minor-font")
		}

		mode := "deck"
		if cmd.Flags().Lookup("mode").Changed {
			mode = themeUpdateMode
		}

		if mode == "slide" {
			slideSpecified := cmd.Flags().Lookup("slide").Changed
			forSlidesSpecified := cmd.Flags().Lookup("for-slides").Changed

			if !slideSpecified && !forSlidesSpecified {
				return InvalidArgsError("slide mode requires either --slide or --for-slides")
			}
			if slideSpecified && forSlidesSpecified {
				return InvalidArgsError("cannot specify both --slide and --for-slides")
			}
		} else if mode != "deck" {
			return InvalidArgsError("mode must be 'deck' or 'slide'")
		}

		result, err := performThemeUpdate(inputPath, mode, themeUpdateSlide, themeUpdateForSlides, mutOpts)
		if err != nil {
			return err
		}

		if GetGlobalConfig(cmd).Format == "json" {
			return outputThemeUpdateJSON(cmd, result)
		}

		return outputThemeUpdateText(cmd, result)
	},
}

func performThemeUpdate(inputPath string, mode string, slideNum int, forSlidesStr string, mutOpts *MutationOptions) (*ThemeUpdateResult, error) {
	result := &ThemeUpdateResult{}

	writer, err := NewMutationWriter(inputPath, mutOpts)
	if err != nil {
		return nil, err
	}

	if err := writer.Write(func(pkg opc.PackageSession) error {
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", err)
		}

		if graph == nil {
			return NewCLIErrorf(ExitUnexpected, "empty presentation graph")
		}

		// Get theme URI from first master
		themeURI := "/ppt/theme/theme1.xml" // default fallback
		if len(graph.Masters) > 0 {
			if graph.Masters[0].ThemeURI != "" {
				themeURI = graph.Masters[0].ThemeURI
			}
		}

		for _, colorStr := range themeUpdateColorsFlag {
			parts := strings.Split(colorStr, "=")
			if len(parts) != 2 {
				return InvalidArgsError(fmt.Sprintf("invalid color format: %s", colorStr))
			}

			colorName := strings.TrimSpace(parts[0])
			hexValue := strings.TrimSpace(parts[1])

			if mode == "deck" {
				req := &mutate.UpdateThemeColorRequest{
					Package:   pkg,
					ThemeURI:  themeURI,
					ColorName: colorName,
					HexValue:  hexValue,
				}

				if err := mutate.UpdateThemeColor(req); err != nil {
					return NewCLIErrorf(ExitInvalidArgs, "failed to update theme color %s: %v", colorName, err)
				}

				result.UpdatedColors = append(result.UpdatedColors, ThemeUpdateColorResult{
					ColorName: colorName,
					HexValue:  hexValue,
					Mode:      "deck",
				})
			} else {
				slideURIs, err := getTargetSlideURIsForTheme(graph, slideNum, forSlidesStr)
				if err != nil {
					return err
				}

				for _, slideURI := range slideURIs {
					req := &mutate.SlideColorOverrideRequest{
						Package:   pkg,
						SlideURI:  slideURI,
						ColorName: colorName,
						HexValue:  hexValue,
					}

					if err := mutate.ApplySlideColorOverride(req); err != nil {
						return NewCLIErrorf(ExitInvalidArgs, "failed to apply color override: %v", err)
					}
				}

				result.UpdatedColors = append(result.UpdatedColors, ThemeUpdateColorResult{
					ColorName: colorName,
					HexValue:  hexValue,
					Mode:      "slide",
				})
			}
		}

		if themeUpdateMajorFont != "" || themeUpdateMinorFont != "" {
			if mode == "deck" {
				req := &mutate.UpdateThemeFontRequest{
					Package:   pkg,
					ThemeURI:  themeURI,
					MajorFont: themeUpdateMajorFont,
					MinorFont: themeUpdateMinorFont,
				}

				if err := mutate.UpdateThemeFont(req); err != nil {
					return NewCLIErrorf(ExitInvalidArgs, "failed to update theme fonts: %v", err)
				}

				result.UpdatedFonts = &ThemeUpdateFontResult{
					MajorFont: themeUpdateMajorFont,
					MinorFont: themeUpdateMinorFont,
					Mode:      "deck",
				}
			} else {
				return NewCLIErrorf(ExitInvalidArgs, "font updates are only supported in deck mode")
			}
		}

		return nil
	}); err != nil {
		return nil, err
	}

	if len(result.UpdatedColors) == 0 && result.UpdatedFonts == nil {
		result.NoOpMessage = "no updates applied"
	} else {
		result.Message = "theme update completed successfully"
	}

	return result, nil
}

func getTargetSlideURIsForTheme(graph *inspect.PresentationGraph, slideNum int, forSlidesStr string) ([]string, error) {
	slideNumbers := []int{}

	if slideNum > 0 {
		slideNumbers = append(slideNumbers, slideNum)
	} else if forSlidesStr != "" {
		slides, err := parseSlideRange(forSlidesStr, len(graph.Slides))
		if err != nil {
			return nil, err
		}
		slideNumbers = slides
	}

	slideURIs := []string{}
	for _, sn := range slideNumbers {
		if sn < 1 || sn > len(graph.Slides) {
			return nil, NewCLIErrorf(ExitInvalidArgs, "slide number %d out of range", sn)
		}
		slideURIs = append(slideURIs, graph.Slides[sn-1].PartURI)
	}

	return slideURIs, nil
}

func parseSlideRange(spec string, maxSlides int) ([]int, error) {
	slideSet := make(map[int]bool)

	parts := strings.Split(spec, ",")
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part == "" {
			continue
		}

		if strings.Contains(part, "-") {
			rangeParts := strings.Split(part, "-")
			if len(rangeParts) != 2 {
				return nil, InvalidArgsError(fmt.Sprintf("invalid range: %s", part))
			}

			var start, end int
			if _, err := fmt.Sscanf(strings.TrimSpace(rangeParts[0]), "%d", &start); err != nil {
				return nil, InvalidArgsError(fmt.Sprintf("invalid number: %s", rangeParts[0]))
			}
			if _, err := fmt.Sscanf(strings.TrimSpace(rangeParts[1]), "%d", &end); err != nil {
				return nil, InvalidArgsError(fmt.Sprintf("invalid number: %s", rangeParts[1]))
			}

			for i := start; i <= end; i++ {
				if i < 1 || i > maxSlides {
					return nil, InvalidArgsError(fmt.Sprintf("slide %d out of range", i))
				}
				slideSet[i] = true
			}
		} else {
			var sn int
			if _, err := fmt.Sscanf(part, "%d", &sn); err != nil {
				return nil, InvalidArgsError(fmt.Sprintf("invalid number: %s", part))
			}
			if sn < 1 || sn > maxSlides {
				return nil, InvalidArgsError(fmt.Sprintf("slide %d out of range", sn))
			}
			slideSet[sn] = true
		}
	}

	slideNumbers := []int{}
	for i := 1; i <= maxSlides; i++ {
		if slideSet[i] {
			slideNumbers = append(slideNumbers, i)
		}
	}

	return slideNumbers, nil
}

func outputThemeUpdateText(cmd *cobra.Command, result *ThemeUpdateResult) error {
	config := GetGlobalConfig(cmd)
	var out io.Writer
	if config.Output != "" {
		file, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer file.Close()
		out = file
	} else {
		out = cmd.OutOrStdout()
	}

	if result.NoOpMessage != "" {
		fmt.Fprintf(out, "No-op: %s\n", result.NoOpMessage)
		return nil
	}

	if result.Message != "" {
		fmt.Fprintf(out, "%s\n", result.Message)
	}

	if len(result.UpdatedColors) > 0 {
		fmt.Fprintf(out, "\nUpdated colors:\n")
		for _, color := range result.UpdatedColors {
			fmt.Fprintf(out, "  %s: #%s (%s)\n", color.ColorName, color.HexValue, color.Mode)
		}
	}

	if result.UpdatedFonts != nil {
		fmt.Fprintf(out, "\nUpdated fonts:\n")
		if result.UpdatedFonts.MajorFont != "" {
			fmt.Fprintf(out, "  Major font: %s\n", result.UpdatedFonts.MajorFont)
		}
		if result.UpdatedFonts.MinorFont != "" {
			fmt.Fprintf(out, "  Minor font: %s\n", result.UpdatedFonts.MinorFont)
		}
	}

	return nil
}

func outputThemeUpdateJSON(cmd *cobra.Command, result *ThemeUpdateResult) error {
	config := GetGlobalConfig(cmd)
	var out io.Writer
	if config.Output != "" {
		file, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer file.Close()
		out = file
	} else {
		out = cmd.OutOrStdout()
	}

	encoder := json.NewEncoder(out)
	if config.Pretty {
		encoder.SetIndent("", "  ")
	}

	return encoder.Encode(result)
}

// themeCmd represents the theme command group
var themeCmd = &cobra.Command{
	Use:   "theme",
	Short: "Inspect and modify presentation themes",
	Long:  "Commands for inspecting and modifying presentation themes (colors and fonts).",
	Args:  cobra.NoArgs,
	RunE:  showHelp,
}

func init() {
	themeCmd.AddCommand(themeUpdateCmd)
	pptxCmd.AddCommand(themeCmd)
	AddMutationFlags(themeUpdateCmd)

	themeUpdateCmd.Flags().StringSliceVar(
		&themeUpdateColorsFlag,
		"color",
		[]string{},
		`color update: "colorName=hexValue" (repeatable)`,
	)

	themeUpdateCmd.Flags().StringVar(
		&themeUpdateMajorFont,
		"major-font",
		"",
		"major (title) font typeface",
	)

	themeUpdateCmd.Flags().StringVar(
		&themeUpdateMinorFont,
		"minor-font",
		"",
		"minor (body) font typeface",
	)

	themeUpdateCmd.Flags().StringVar(
		&themeUpdateMode,
		"mode",
		"deck",
		`update mode: "deck" or "slide"`,
	)

	themeUpdateCmd.Flags().IntVar(
		&themeUpdateSlide,
		"slide",
		0,
		"slide number (1-based) for slide mode",
	)

	themeUpdateCmd.Flags().StringVar(
		&themeUpdateForSlides,
		"for-slides",
		"",
		`slide targeting for slide mode: "1,3,5-7"`,
	)
}
