package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strconv"
	"strings"
	"time"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/template"
)

var (
	captureTemplate     string
	captureAuthor       string
	captureOrganization string
	captureDescription  string
	captureSlides       string
	captureVersion      string
	captureStrictShapes bool
)

var templateCaptureCmd = &cobra.Command{
	Use:   "capture <pptx-file>",
	Short: "Capture archetypes from a PPTX presentation",
	Long: `Capture selected archetype slides from a PPTX presentation to create a template manifest.
The manifest includes slot definitions, static shapes, and layout/master references.

Usage:
  ooxml pptx template capture --name "My Template" --slides 1,2,3 deck.pptx
  ooxml pptx template capture -o template-manifest.json --format json deck.pptx`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]

		// Check if file exists
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		// Open the package
		pkg, err := openPackageExpectType(filePath, opc.PackageTypePPTX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		// Parse presentation
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", err)
		}

		// Get global config
		config := GetGlobalConfig(cmd)

		// Parse slide numbers if provided
		var slideNumbers []int
		if captureSlides != "" {
			parts := strings.Split(captureSlides, ",")
			for _, part := range parts {
				part = strings.TrimSpace(part)
				if part == "" {
					continue
				}

				num, err := strconv.Atoi(part)
				if err != nil {
					return NewCLIErrorf(ExitInvalidArgs, "invalid slide number: %s", part)
				}

				slideNumbers = append(slideNumbers, num)
			}
		}

		// Create capture options
		options := template.CaptureOptions{
			Name:               captureTemplate,
			Description:        captureDescription,
			Author:             captureAuthor,
			Organization:       captureOrganization,
			SlideNumbers:       slideNumbers,
			StrictStaticShapes: captureStrictShapes,
		}

		// Parse version if provided
		if captureVersion != "" {
			parts := strings.Split(captureVersion, ".")
			if len(parts) != 3 {
				return NewCLIErrorf(ExitInvalidArgs, "version must be in format major.minor.patch")
			}

			major, err := strconv.Atoi(parts[0])
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "invalid major version: %s", parts[0])
			}

			minor, err := strconv.Atoi(parts[1])
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "invalid minor version: %s", parts[1])
			}

			patch, err := strconv.Atoi(parts[2])
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "invalid patch version: %s", parts[2])
			}

			options.Version = &template.Version{
				Major:     major,
				Minor:     minor,
				Patch:     patch,
				CreatedAt: time.Now(),
			}
		}

		// Create capture engine
		engine := template.NewCaptureEngine(pkg, graph, options)

		// Capture template
		manifest, err := engine.Capture()
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "template capture failed: %v", err)
		}

		// Format and output results
		if config.Format == "json" {
			return outputTemplateManifestJSON(cmd, manifest, config)
		}

		// Default to text output
		return outputTemplateManifestText(cmd, manifest)
	},
}

// outputTemplateManifestJSON outputs the manifest in JSON format
func outputTemplateManifestJSON(cmd *cobra.Command, manifest *template.TemplateManifest, config *GlobalConfig) error {
	var jsonData []byte
	var err error

	if config.Pretty {
		jsonData, err = json.MarshalIndent(manifest, "", "  ")
	} else {
		jsonData, err = json.Marshal(manifest)
	}

	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal manifest to JSON: %v", err)
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

// outputTemplateManifestText outputs the manifest in human-readable text format
func outputTemplateManifestText(cmd *cobra.Command, manifest *template.TemplateManifest) error {
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
	fmt.Fprintf(outFile, "Template: %s\n", manifest.Name)
	if manifest.Description != "" {
		fmt.Fprintf(outFile, "Description: %s\n", manifest.Description)
	}
	fmt.Fprintf(outFile, "Version: %s\n", manifest.Version.String())
	fmt.Fprintf(outFile, "Created: %s\n", manifest.CreatedAt.Format("2006-01-02 15:04:05"))

	if manifest.Author != "" {
		fmt.Fprintf(outFile, "Author: %s\n", manifest.Author)
	}
	if manifest.Organization != "" {
		fmt.Fprintf(outFile, "Organization: %s\n", manifest.Organization)
	}

	fmt.Fprintf(outFile, "\nArchetypes: %d\n", len(manifest.Archetypes))

	// Print each archetype
	for i, arch := range manifest.Archetypes {
		fmt.Fprintf(outFile, "\n[%d] %s (id=%s)\n", i+1, arch.Name, arch.ID)
		if arch.Description != "" {
			fmt.Fprintf(outFile, "    Description: %s\n", arch.Description)
		}
		if arch.LayoutName != "" {
			fmt.Fprintf(outFile, "    Layout: %s\n", arch.LayoutName)
		}

		// Print slots
		fmt.Fprintf(outFile, "    Slots: %d\n", len(arch.Slots))
		for _, slot := range arch.Slots {
			fmt.Fprintf(outFile, "      - %s (kind=%s, required=%v)\n", slot.Name, slot.Kind, slot.Required)
			if slot.PlaceholderRole != "" {
				fmt.Fprintf(outFile, "        role=%s\n", slot.PlaceholderRole)
			}
			if slot.Kind == "table" && slot.TableRows != nil {
				fmt.Fprintf(outFile, "        table: %dx%d\n", *slot.TableRows, *slot.TableCols)
			}
		}

		// Print static shapes
		if len(arch.StaticShapes) > 0 {
			fmt.Fprintf(outFile, "    Static Shapes: %d\n", len(arch.StaticShapes))
			for _, shape := range arch.StaticShapes {
				fmt.Fprintf(outFile, "      - %s (type=%s)\n", shape.Name, shape.Type)
			}
		}
	}

	return nil
}

// init registers the template capture command
func init() {
	templateCaptureCmd.Flags().StringVar(&captureTemplate, "name", "", "Template name (required)")
	templateCaptureCmd.Flags().StringVar(&captureDescription, "description", "", "Template description")
	templateCaptureCmd.Flags().StringVar(&captureAuthor, "author", "", "Template author")
	templateCaptureCmd.Flags().StringVar(&captureOrganization, "organization", "", "Organization name")
	templateCaptureCmd.Flags().StringVar(&captureSlides, "slides", "", "Comma-separated slide numbers to capture (e.g., '1,2,3'). If empty, all slides are captured.")
	templateCaptureCmd.Flags().StringVar(&captureVersion, "version", "1.0.0", "Template version in format major.minor.patch")
	templateCaptureCmd.Flags().BoolVar(&captureStrictShapes, "strict-shapes", false, "Enable strict validation of static shapes")

	templateCmd.AddCommand(templateCaptureCmd)
}
