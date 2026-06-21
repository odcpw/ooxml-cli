package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/template"
)

var (
	inspectFormat string
)

var templateInspectCmd = &cobra.Command{
	Use:   "inspect <manifest-file>",
	Short: "Inspect a captured template manifest",
	Long: `Inspect a captured template manifest file and display information about
available archetypes, slots, and static shapes.

Usage:
  ooxml pptx template inspect template-manifest.json
  ooxml pptx template inspect -f json template-manifest.json`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		manifestPath := args[0]

		// Check if file exists
		if _, err := os.Stat(manifestPath); err != nil {
			return FileNotFoundError(manifestPath)
		}

		// Read manifest file
		data, err := os.ReadFile(manifestPath)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to read manifest file: %v", err)
		}

		// Parse manifest
		var manifest template.TemplateManifest
		if err := json.Unmarshal(data, &manifest); err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse manifest: %v", err)
		}

		// Validate manifest
		if err := manifest.ValidateManifest(); err != nil {
			return NewCLIErrorf(ExitUnexpected, "manifest validation failed: %v", err)
		}

		// Get global config
		config := GetGlobalConfig(cmd)

		// Output in requested format
		if config.Format == "json" {
			return outputManifestInspectJSON(cmd, &manifest, config)
		}

		// Default to text output
		return outputManifestInspectText(cmd, &manifest)
	},
}

// outputManifestInspectJSON outputs manifest inspection in JSON format
func outputManifestInspectJSON(cmd *cobra.Command, manifest *template.TemplateManifest, config *GlobalConfig) error {
	// Create a simplified inspection result for JSON output
	result := map[string]interface{}{
		"name":         manifest.Name,
		"description":  manifest.Description,
		"version":      manifest.Version.String(),
		"author":       manifest.Author,
		"organization": manifest.Organization,
		"createdAt":    manifest.CreatedAt,
		"modifiedAt":   manifest.ModifiedAt,
		"archetypes":   []map[string]interface{}{},
	}

	// Add archetype information
	archetypesData := make([]map[string]interface{}, len(manifest.Archetypes))
	for i, arch := range manifest.Archetypes {
		archData := map[string]interface{}{
			"id":           arch.ID,
			"name":         arch.Name,
			"description":  arch.Description,
			"layoutName":   arch.LayoutName,
			"masterName":   arch.MasterName,
			"slots":        []map[string]interface{}{},
			"staticShapes": []map[string]interface{}{},
		}

		// Add slot information
		slotsData := make([]map[string]interface{}, len(arch.Slots))
		for j, slot := range arch.Slots {
			slotData := map[string]interface{}{
				"id":       slot.ID,
				"name":     slot.Name,
				"kind":     slot.Kind,
				"required": slot.Required,
			}
			if slot.PlaceholderRole != "" {
				slotData["placeholderRole"] = slot.PlaceholderRole
			}
			if slot.Kind == "table" {
				if slot.TableRows != nil {
					slotData["tableRows"] = *slot.TableRows
				}
				if slot.TableCols != nil {
					slotData["tableCols"] = *slot.TableCols
				}
			}
			slotsData[j] = slotData
		}
		archData["slots"] = slotsData

		// Add static shapes information
		shapesData := make([]map[string]interface{}, len(arch.StaticShapes))
		for j, shape := range arch.StaticShapes {
			shapeData := map[string]interface{}{
				"id":   shape.ID,
				"name": shape.Name,
				"type": shape.Type,
			}
			shapesData[j] = shapeData
		}
		archData["staticShapes"] = shapesData

		archetypesData[i] = archData
	}
	result["archetypes"] = archetypesData

	var jsonData []byte
	var err error
	if config.Pretty {
		jsonData, err = json.MarshalIndent(result, "", "  ")
	} else {
		jsonData, err = json.Marshal(result)
	}

	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal inspection result: %v", err)
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

// outputManifestInspectText outputs manifest inspection in human-readable text format
func outputManifestInspectText(cmd *cobra.Command, manifest *template.TemplateManifest) error {
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

	// Print manifest header
	fmt.Fprintf(outFile, "═══════════════════════════════════════════════════════════\n")
	fmt.Fprintf(outFile, "Template Manifest\n")
	fmt.Fprintf(outFile, "═══════════════════════════════════════════════════════════\n\n")

	fmt.Fprintf(outFile, "Name:          %s\n", manifest.Name)
	fmt.Fprintf(outFile, "Version:       %s\n", manifest.Version.String())
	fmt.Fprintf(outFile, "Created:       %s\n", manifest.CreatedAt.Format("2006-01-02 15:04:05"))
	fmt.Fprintf(outFile, "Last Modified: %s\n", manifest.ModifiedAt.Format("2006-01-02 15:04:05"))

	if manifest.Description != "" {
		fmt.Fprintf(outFile, "Description:   %s\n", manifest.Description)
	}
	if manifest.Author != "" {
		fmt.Fprintf(outFile, "Author:        %s\n", manifest.Author)
	}
	if manifest.Organization != "" {
		fmt.Fprintf(outFile, "Organization:  %s\n", manifest.Organization)
	}

	fmt.Fprintf(outFile, "\n───────────────────────────────────────────────────────────\n")
	fmt.Fprintf(outFile, "Archetypes (%d total)\n", len(manifest.Archetypes))
	fmt.Fprintf(outFile, "───────────────────────────────────────────────────────────\n\n")

	// Print each archetype
	for i, arch := range manifest.Archetypes {
		fmt.Fprintf(outFile, "[%d] %s\n", i+1, arch.Name)
		fmt.Fprintf(outFile, "    ID: %s\n", arch.ID)
		if arch.Description != "" {
			fmt.Fprintf(outFile, "    Description: %s\n", arch.Description)
		}
		if arch.LayoutName != "" {
			fmt.Fprintf(outFile, "    Layout: %s\n", arch.LayoutName)
		}
		if arch.MasterName != "" {
			fmt.Fprintf(outFile, "    Master: %s\n", arch.MasterName)
		}

		// Print slots
		fmt.Fprintf(outFile, "\n    Slots (%d):\n", len(arch.Slots))
		for _, slot := range arch.Slots {
			required := "optional"
			if slot.Required {
				required = "required"
			}
			fmt.Fprintf(outFile, "      • %s\n", slot.Name)
			fmt.Fprintf(outFile, "        ID: %s | Kind: %s | %s\n", slot.ID, slot.Kind, required)

			if slot.PlaceholderRole != "" {
				fmt.Fprintf(outFile, "        Placeholder Role: %s\n", slot.PlaceholderRole)
			}

			if slot.Kind == "table" && (slot.TableRows != nil || slot.TableCols != nil) {
				rows := 0
				cols := 0
				if slot.TableRows != nil {
					rows = *slot.TableRows
				}
				if slot.TableCols != nil {
					cols = *slot.TableCols
				}
				fmt.Fprintf(outFile, "        Table Dimensions: %d rows × %d cols\n", rows, cols)
			}

			if slot.AspectRatio != nil {
				fmt.Fprintf(outFile, "        Aspect Ratio: %.2f\n", *slot.AspectRatio)
			}

			if slot.Notes != "" {
				fmt.Fprintf(outFile, "        Notes: %s\n", slot.Notes)
			}
		}

		// Print static shapes
		if len(arch.StaticShapes) > 0 {
			fmt.Fprintf(outFile, "\n    Static Shapes (%d):\n", len(arch.StaticShapes))
			for _, shape := range arch.StaticShapes {
				fmt.Fprintf(outFile, "      • %s (type: %s, id: %s)\n", shape.Name, shape.Type, shape.ID)
			}
		}

		fmt.Fprintf(outFile, "\n")
	}

	fmt.Fprintf(outFile, "═══════════════════════════════════════════════════════════\n")
	return nil
}

// init registers the template inspect command
func init() {
	templateInspectCmd.Flags().StringVar(&inspectFormat, "format", "text", "Output format: 'text' or 'json'")

	templateCmd.AddCommand(templateInspectCmd)
}
