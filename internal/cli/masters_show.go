package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// TextStyleSummary represents a summary of text styling
type TextStyleSummary struct {
	PlaceholderType string `json:"placeholderType"`
	SampleText      string `json:"sampleText,omitempty"`
}

// MasterDetail represents detailed information about a master
type MasterDetail struct {
	URI                  string                      `json:"uri"`
	Index                int                         `json:"index"`
	Layouts              []string                    `json:"layouts"`
	LayoutCount          int                         `json:"layoutCount"`
	ThemeURI             string                      `json:"themeUri,omitempty"`
	Theme                *model.ThemeInfo            `json:"theme,omitempty"`
	Shapes               int                         `json:"shapes"`
	Placeholders         []model.PlaceholderInfo     `json:"placeholders,omitempty"`
	TextStyles           map[string]TextStyleSummary `json:"textStyles,omitempty"`
	DefaultTextStyleInfo interface{}                 `json:"defaultTextStyleInfo,omitempty"`
}

var (
	mastersShowMasterFlag int
)

var mastersShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Show details of a specific slide master",
	Long: `Show detailed information about a slide master in a PPTX file, including
linked layouts, theme reference, and text style information.`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]

		// Check if file exists
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		// Open the PPTX file
		pkg, err := openPackageExpectType(filePath, opc.PackageTypePPTX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		// Parse presentation to get masters
		masters, err := ParsePresentationMasters(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", err)
		}

		// Get the requested master
		master := GetMasterByIndex(masters, mastersShowMasterFlag)
		if master == nil {
			return NewCLIErrorf(ExitInvalidArgs, "master %d not found", mastersShowMasterFlag)
		}

		// Get master details
		detail, err := getMasterDetail(pkg, master)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to get master detail: %v", err)
		}

		// Get global config
		config := GetGlobalConfig(cmd)

		// Format and output results
		if config.Format == "json" {
			return outputMasterShowJSON(cmd, detail)
		}

		// Default to text output
		return outputMasterShowText(cmd, detail)
	},
}

func getMasterDetail(pkg opc.PackageSession, master *MasterInfo) (*MasterDetail, error) {
	detail := &MasterDetail{
		URI:          master.PartURI,
		Index:        master.Index,
		Layouts:      master.Layouts,
		LayoutCount:  master.LayoutCount,
		ThemeURI:     master.ThemeURI,
		Placeholders: master.Placeholders,
		TextStyles:   make(map[string]TextStyleSummary),
	}

	// Parse theme information if available
	if master.ThemeURI != "" {
		if theme, err := inspect.ParseTheme(pkg, master.ThemeURI); err == nil {
			detail.Theme = theme
		}
		// Extract default text style info from theme
		detail.DefaultTextStyleInfo = inspect.ExtractDefaultTextStyleInfo(pkg, master.ThemeURI)
	}

	// Count shapes in the master
	shapes, err := CountShapesInMaster(pkg, master.PartURI)
	if err != nil {
		// Log but continue - not a fatal error
		shapes = 0
	}
	detail.Shapes = shapes

	// Read the master XML to extract text styles
	masterXML, err := pkg.ReadXMLPart(master.PartURI)
	if err == nil {
		// Extract text style information
		xmlStr, _ := masterXML.WriteToString()
		extractTextStyleSummaries(detail, xmlStr)
	}

	return detail, nil
}

func extractTextStyleSummaries(detail *MasterDetail, masterXML string) {
	// Extract text from shapes that represent text styles
	// This is a simplified version that looks for common style indicators

	// Title style
	if contains(masterXML, "title") || contains(masterXML, "Title") {
		detail.TextStyles["title"] = TextStyleSummary{
			PlaceholderType: "title",
		}
	}

	// Body style
	if contains(masterXML, "body") || contains(masterXML, "Body") {
		detail.TextStyles["body"] = TextStyleSummary{
			PlaceholderType: "body",
		}
	}

	// Center title style
	if contains(masterXML, "ctrTitle") || contains(masterXML, "centerTitle") {
		detail.TextStyles["centerTitle"] = TextStyleSummary{
			PlaceholderType: "centerTitle",
		}
	}

	// Subtitle style
	if contains(masterXML, "subTitle") || contains(masterXML, "subtitle") {
		detail.TextStyles["subtitle"] = TextStyleSummary{
			PlaceholderType: "subtitle",
		}
	}
}

// contains checks if a string contains a substring (case-sensitive)
func contains(str, substr string) bool {
	for i := 0; i <= len(str)-len(substr); i++ {
		match := true
		for j := 0; j < len(substr); j++ {
			if str[i+j] != substr[j] {
				match = false
				break
			}
		}
		if match {
			return true
		}
	}
	return false
}

func outputMasterShowJSON(cmd *cobra.Command, detail *MasterDetail) error {
	config := GetGlobalConfig(cmd)

	var jsonData []byte
	var err error
	if config.Pretty {
		jsonData, err = json.MarshalIndent(detail, "", "  ")
	} else {
		jsonData, err = json.Marshal(detail)
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

func outputMasterShowText(cmd *cobra.Command, detail *MasterDetail) error {
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

	// Format text output
	fmt.Fprintf(outFile, "Master: %s\n", detail.URI)
	fmt.Fprintf(outFile, "Layouts: %d\n", detail.LayoutCount)
	for _, layout := range detail.Layouts {
		fmt.Fprintf(outFile, "  %s\n", layout)
	}

	if detail.ThemeURI != "" {
		fmt.Fprintf(outFile, "Theme: %s\n", detail.ThemeURI)
	}

	fmt.Fprintf(outFile, "Shapes: %d\n", detail.Shapes)

	if len(detail.TextStyles) > 0 {
		fmt.Fprintf(outFile, "Text Styles:\n")
		for style, summary := range detail.TextStyles {
			fmt.Fprintf(outFile, "  %s: %s\n", style, summary.PlaceholderType)
		}
	}

	return nil
}

// init registers the masters show command
func init() {
	mastersShowCmd.Flags().IntVarP(
		&mastersShowMasterFlag,
		"master",
		"m",
		1,
		"master index (1-based, default: 1)",
	)

	mastersCmd.AddCommand(mastersShowCmd)
}
