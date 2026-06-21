package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// LayoutListOutput represents the JSON output for layouts list
type LayoutListOutput struct {
	File    string         `json:"file"`
	Layouts []*LayoutEntry `json:"layouts"`
}

// LayoutEntry represents one layout in the list
type LayoutEntry struct {
	ID               string   `json:"id"`
	Number           int      `json:"number"`
	Name             string   `json:"name"`
	PartURI          string   `json:"partUri"`
	MasterID         string   `json:"masterId,omitempty"`
	PrimarySelector  string   `json:"primarySelector"`
	Selectors        []string `json:"selectors"`
	PlaceholderCount int      `json:"placeholderCount"`
	Placeholders     []string `json:"placeholders"`
}

var (
	layoutsListMasterFlag int
)

var layoutsListCmd = &cobra.Command{
	Use:   "list <file>",
	Short: "List slide layouts with their placeholders",
	Long: `List all slide layouts in a presentation with placeholder key summaries.

If --master is specified, only layouts associated with that master are shown.`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]

		// Check if file exists
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		// Open package
		pkg, err := openPackageExpectType(filePath, opc.PackageTypePPTX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		// Get global config
		config := GetGlobalConfig(cmd)

		// Parse layouts
		layouts, err := ParsePresentationLayouts(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse layouts: %v", err)
		}

		// Filter by master if specified
		if layoutsListMasterFlag > 0 {
			masterID := fmt.Sprintf("master-%d", layoutsListMasterFlag)
			layouts = FilterLayoutsByMaster(layouts, masterID)
		}

		// Format and output
		if config.Format == "json" {
			return outputLayoutsListJSON(cmd, filePath, layouts)
		}

		// Default to text output
		return outputLayoutsListText(cmd, layouts)
	},
}

// outputLayoutsListText outputs the layout list in human-readable format
func outputLayoutsListText(cmd *cobra.Command, layouts []*LayoutInfo) error {
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

	for i, layout := range layouts {
		num := i + 1
		keyList := FormatPlaceholderKeyList(layout.Placeholders)
		fmt.Fprintf(outFile, "[%d] %-30s placeholders: %s\n", num, layout.Name, keyList)
	}

	return nil
}

// outputLayoutsListJSON outputs the layout list in JSON format
func outputLayoutsListJSON(cmd *cobra.Command, filePath string, layouts []*LayoutInfo) error {
	config := GetGlobalConfig(cmd)

	entries := make([]*LayoutEntry, len(layouts))
	for i, layout := range layouts {
		keys := make([]string, len(layout.Placeholders))
		for j := range layout.Placeholders {
			keys[j] = layout.Placeholders[j].Key
		}

		entries[i] = &LayoutEntry{
			ID:               layout.ID,
			Number:           i + 1,
			Name:             layout.Name,
			PartURI:          layout.PartURI,
			MasterID:         layout.MasterID,
			PrimarySelector:  layoutPrimarySelector(i + 1),
			Selectors:        layoutSelectors(i+1, layout.Name),
			PlaceholderCount: len(layout.Placeholders),
			Placeholders:     keys,
		}
	}

	output := &LayoutListOutput{
		File:    filePath,
		Layouts: entries,
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

func layoutPrimarySelector(number int) string {
	return fmt.Sprintf("%d", number)
}

func layoutSelectors(number int, name string) []string {
	selectors := []string{layoutPrimarySelector(number)}
	if name != "" && name != selectors[0] {
		selectors = append(selectors, name)
	}
	return selectors
}

// init registers the layouts list command
func init() {
	// Create the layouts parent command if it doesn't exist
	if layoutsCmd == nil {
		layoutsCmd = &cobra.Command{
			Use:   "layouts",
			Short: "Inspect slide layouts",
			Long:  "Commands for inspecting slide layouts and their placeholders.",
		}
		pptxCmd.AddCommand(layoutsCmd)
	}

	layoutsListCmd.Flags().IntVarP(
		&layoutsListMasterFlag,
		"master",
		"",
		0,
		"filter layouts by master number (1-based)",
	)

	layoutsCmd.AddCommand(layoutsListCmd)
}
