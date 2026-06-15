package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// MasterListItem represents a single master in the list output
type MasterListItem struct {
	Index           int      `json:"index"`
	URI             string   `json:"uri"`
	PrimarySelector string   `json:"primarySelector"`
	Selectors       []string `json:"selectors"`
	Layouts         int      `json:"layouts"`
	Theme           string   `json:"theme,omitempty"`
}

// MasterListResult is the JSON output structure for the masters list command
type MasterListResult struct {
	Masters []MasterListItem `json:"masters"`
}

var mastersListCmd = &cobra.Command{
	Use:   "list <file>",
	Short: "List all slide masters in a presentation",
	Long: `List all slide masters in a PPTX file, showing the number of layouts
associated with each master and its linked theme.`,
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

		// Get global config
		config := GetGlobalConfig(cmd)

		// Convert to list items for output
		items := make([]MasterListItem, len(masters))
		for i, master := range masters {
			items[i] = MasterListItem{
				Index:           master.Index,
				URI:             master.PartURI,
				PrimarySelector: masterPrimarySelector(master.Index),
				Selectors:       masterSelectors(master.Index),
				Layouts:         master.LayoutCount,
				Theme:           master.ThemeURI,
			}
		}

		// Format and output results
		if config.Format == "json" {
			return outputMastersListJSON(cmd, items)
		}

		// Default to text output
		return outputMastersListText(cmd, items)
	},
}

func masterPrimarySelector(index int) string {
	return fmt.Sprintf("%d", index)
}

func masterSelectors(index int) []string {
	return []string{masterPrimarySelector(index)}
}

func outputMastersListJSON(cmd *cobra.Command, masters []MasterListItem) error {
	config := GetGlobalConfig(cmd)

	result := MasterListResult{
		Masters: masters,
	}

	var jsonData []byte
	var err error
	if config.Pretty {
		jsonData, err = json.MarshalIndent(result, "", "  ")
	} else {
		jsonData, err = json.Marshal(result)
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

func outputMastersListText(cmd *cobra.Command, masters []MasterListItem) error {
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
	for _, master := range masters {
		fmt.Fprintf(outFile, "[%d] %s\n", master.Index, master.URI)
		fmt.Fprintf(outFile, "  layouts: %d\n", master.Layouts)
		if master.Theme != "" {
			fmt.Fprintf(outFile, "  theme: %s\n", master.Theme)
		}
	}

	return nil
}

// init registers the masters list command
func init() {
	mastersCmd.AddCommand(mastersListCmd)
}
