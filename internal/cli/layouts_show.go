package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strconv"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// LayoutShowOutput represents the JSON output for layouts show
type LayoutShowOutput struct {
	ID                   string                  `json:"id"`
	Name                 string                  `json:"name"`
	PartURI              string                  `json:"partUri"`
	MasterID             string                  `json:"masterId,omitempty"`
	ThemeURI             string                  `json:"themeUri,omitempty"`
	Theme                *model.ThemeInfo        `json:"theme,omitempty"`
	Preserve             bool                    `json:"preserve"`
	UserDrawn            bool                    `json:"userDrawn"`
	Placeholders         []model.PlaceholderInfo `json:"placeholders"`
	DefaultTextStyleInfo interface{}             `json:"defaultTextStyleInfo,omitempty"`
}

var (
	layoutShowLayoutFlag string
)

var layoutsShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Show detailed information about a specific layout",
	Long: `Show detailed information about a slide layout, including all placeholders with their normalized keys.

Use --layout with either a layout number (1-based) or layout name.`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]

		// Check if file exists
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		layoutSelector := layoutShowLayoutFlag
		if layoutSelector == "" {
			layoutSelector, _ = cmd.Flags().GetString("layout")
		}
		if layoutSelector == "" {
			return NewCLIErrorf(ExitInvalidArgs, "--layout flag is required")
		}

		// Open package
		pkg, err := openPackageExpectType(filePath, opc.PackageTypePPTX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		// Parse layouts
		layouts, err := ParsePresentationLayouts(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse layouts: %v", err)
		}

		// Find the requested layout
		var layout *LayoutInfo

		// Try parsing as number first
		if num, err := strconv.Atoi(layoutSelector); err == nil {
			layout = GetLayoutByNumber(layouts, num)
		}

		// If not found by number, try by name
		if layout == nil {
			layout = GetLayoutByName(layouts, layoutSelector)
		}

		if layout == nil {
			return NewCLIErrorf(ExitInvalidArgs, "layout not found: %s", layoutSelector)
		}

		// Get global config
		config := GetGlobalConfig(cmd)

		// Format and output
		if config.Format == "json" {
			return outputLayoutShowJSON(cmd, pkg, layout)
		}

		// Default to text output
		return outputLayoutShowText(cmd, layout)
	},
}

// outputLayoutShowText outputs layout details in human-readable format
func outputLayoutShowText(cmd *cobra.Command, layout *LayoutInfo) error {
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

	// Output layout metadata
	fmt.Fprintf(outFile, "Layout: %s\n", layout.Name)
	fmt.Fprintf(outFile, "Part URI: %s\n", layout.PartURI)
	if layout.MasterID != "" {
		fmt.Fprintf(outFile, "Master: %s\n", layout.MasterID)
	}
	if layout.Preserve {
		fmt.Fprintf(outFile, "Preserve: true\n")
	}
	if layout.UserDrawn {
		fmt.Fprintf(outFile, "UserDrawn: true\n")
	}

	fmt.Fprintf(outFile, "\nPlaceholders:\n")
	if len(layout.Placeholders) == 0 {
		fmt.Fprintf(outFile, "  (none)\n")
	} else {
		for _, ph := range layout.Placeholders {
			fmt.Fprintf(outFile, "  %s (role: %s, name: %s", ph.Key, ph.Role, ph.ShapeName)
			if ph.Index > 0 {
				fmt.Fprintf(outFile, ", index: %d", ph.Index)
			}
			fmt.Fprintf(outFile, ")\n")
		}
	}

	return nil
}

// outputLayoutShowJSON outputs layout details in JSON format
func outputLayoutShowJSON(cmd *cobra.Command, pkg opc.PackageSession, layout *LayoutInfo) error {
	config := GetGlobalConfig(cmd)

	output := &LayoutShowOutput{
		ID:           layout.ID,
		Name:         layout.Name,
		PartURI:      layout.PartURI,
		MasterID:     layout.MasterID,
		ThemeURI:     layout.ThemeURI,
		Preserve:     layout.Preserve,
		UserDrawn:    layout.UserDrawn,
		Placeholders: layout.Placeholders,
	}

	// Parse theme information if available
	if layout.ThemeURI != "" {
		if theme, err := inspect.ParseTheme(pkg, layout.ThemeURI); err == nil {
			output.Theme = theme
		}
		// Extract default text style info from theme
		output.DefaultTextStyleInfo = inspect.ExtractDefaultTextStyleInfo(pkg, layout.ThemeURI)
	}

	// Marshal to JSON
	var jsonData []byte
	var err error
	if config.Pretty {
		jsonData, err = json.MarshalIndent(output, "", "  ")
	} else {
		jsonData, err = json.Marshal(output)
	}

	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal JSON: %v", err)
	}

	// Write to output
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

// init registers the layouts show command
func init() {
	layoutsShowCmd.Flags().StringVarP(
		&layoutShowLayoutFlag,
		"layout",
		"l",
		"",
		"layout number (1-based) or name to display",
	)
	layoutsShowCmd.MarkFlagRequired("layout")

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

	layoutsCmd.AddCommand(layoutsShowCmd)
}
