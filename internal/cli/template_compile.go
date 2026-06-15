package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"github.com/spf13/cobra"
	"gopkg.in/yaml.v3"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/template"
)

var (
	compileArchetype     string
	compileOutput        string
	compileContinueError bool
	compileImageBaseDir  string
	compileFormat        string
)

var templateCompileCmd = &cobra.Command{
	Use:   "compile <manifest> <spec> [flags]",
	Short: "Compile a presentation from a template manifest and specification",
	Long: `Compile a PPTX presentation from a template manifest and specification file.
Reads the manifest JSON, spec YAML, archetype PPTX, and generates an output PPTX
with content filled from the specification.

Usage:
  ooxml pptx template compile manifest.json spec.yaml --archetype deck.pptx --out out.pptx
  ooxml pptx template compile manifest.json spec.yaml --archetype deck.pptx --out out.pptx --image-base-dir ./images
  ooxml pptx template compile manifest.json spec.yaml --archetype deck.pptx --out out.pptx --continue-on-error`,
	Args: cobra.ExactArgs(2),
	RunE: func(cmd *cobra.Command, args []string) error {
		manifestPath := args[0]
		specPath := args[1]

		// Validate required flags
		if compileArchetype == "" {
			return NewCLIErrorf(ExitInvalidArgs, "--archetype is required")
		}
		if compileOutput == "" {
			return NewCLIErrorf(ExitInvalidArgs, "--out is required")
		}

		// Check if files exist
		if _, err := os.Stat(manifestPath); err != nil {
			return FileNotFoundError(manifestPath)
		}
		if _, err := os.Stat(specPath); err != nil {
			return FileNotFoundError(specPath)
		}
		if _, err := os.Stat(compileArchetype); err != nil {
			return FileNotFoundError(compileArchetype)
		}

		// Read manifest file
		manifestData, err := os.ReadFile(manifestPath)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to read manifest file: %v", err)
		}

		// Parse manifest
		var manifest template.TemplateManifest
		if err := json.Unmarshal(manifestData, &manifest); err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse manifest: %v", err)
		}

		// Validate manifest
		if err := manifest.ValidateManifest(); err != nil {
			return NewCLIErrorf(ExitUnexpected, "manifest validation failed: %v", err)
		}

		// Read spec file
		specData, err := os.ReadFile(specPath)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to read spec file: %v", err)
		}

		// Parse spec (YAML)
		spec := &template.CompilationSpec{}
		if err := yaml.Unmarshal(specData, spec); err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse spec: %v", err)
		}

		// Validate spec against manifest
		if err := template.ValidateCompilationSpec(spec, &manifest); err != nil {
			return NewCLIErrorf(ExitUnexpected, "spec validation failed: %v", err)
		}

		imageBaseDir := compileImageBaseDir
		if imageBaseDir == "" {
			imageBaseDir = filepath.Dir(specPath)
		}

		// Create compile options
		options := template.CompileOptions{
			ArchetypePath:   compileArchetype,
			OutputPath:      compileOutput,
			ContinueOnError: compileContinueError,
			ImageBaseDir:    imageBaseDir,
		}

		// Create compiler engine
		engine := template.NewCompilerEngine(&manifest, spec, options)

		// Compile presentation
		result, err := engine.Compile()
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "compilation failed: %v", err)
		}

		// Get global config
		config := GetGlobalConfig(cmd)

		// Output in requested format
		if config.Format == "json" {
			return outputCompileResultJSON(cmd, result, config)
		}

		// Default to text output
		return outputCompileResultText(cmd, result)
	},
}

// outputCompileResultJSON outputs compilation result in JSON format
func outputCompileResultJSON(cmd *cobra.Command, result *template.CompileResult, config *GlobalConfig) error {
	resultData := map[string]interface{}{
		"outputPath":     result.OutputPath,
		"slideCount":     result.SlideCount,
		"slotsAttempted": result.SlotsAttempted,
		"slotsSucceeded": result.SlotsSucceeded,
		"startedAt":      result.StartedAt,
		"completedAt":    result.CompletedAt,
		"duration":       result.CompletedAt.Sub(result.StartedAt).String(),
		"errors":         []map[string]interface{}{},
	}

	// Add error information
	for _, err := range result.Errors {
		errData := map[string]interface{}{
			"slideIndex": err.SlideIndex,
			"message":    err.Message,
		}
		if err.SlotID != "" {
			errData["slotID"] = err.SlotID
		}
		resultData["errors"] = append(resultData["errors"].([]map[string]interface{}), errData)
	}

	var jsonData []byte
	var err error

	if config.Pretty {
		jsonData, err = json.MarshalIndent(resultData, "", "  ")
	} else {
		jsonData, err = json.Marshal(resultData)
	}

	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal result to JSON: %v", err)
	}

	outWriter := cmd.OutOrStdout()
	if config.Output != "" {
		file, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer file.Close()
		outWriter = file
	}

	fmt.Fprintf(outWriter, "%s\n", string(jsonData))
	return nil
}

// outputCompileResultText outputs compilation result in human-readable text format
func outputCompileResultText(cmd *cobra.Command, result *template.CompileResult) error {
	config := GetGlobalConfig(cmd)

	outWriter := cmd.OutOrStdout()
	if config.Output != "" {
		file, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer file.Close()
		outWriter = file
	}

	// Print result header
	fmt.Fprintf(outWriter, "Template Compilation Result\n")
	fmt.Fprintf(outWriter, "==========================\n\n")

	fmt.Fprintf(outWriter, "Output: %s\n", result.OutputPath)
	fmt.Fprintf(outWriter, "Slides: %d\n", result.SlideCount)
	fmt.Fprintf(outWriter, "Slots: %d attempted, %d succeeded\n", result.SlotsAttempted, result.SlotsSucceeded)
	fmt.Fprintf(outWriter, "Started: %s\n", result.StartedAt.Format("2006-01-02 15:04:05"))
	fmt.Fprintf(outWriter, "Completed: %s\n", result.CompletedAt.Format("2006-01-02 15:04:05"))
	fmt.Fprintf(outWriter, "Duration: %s\n", result.CompletedAt.Sub(result.StartedAt).String())

	if len(result.Errors) > 0 {
		fmt.Fprintf(outWriter, "\nErrors: %d\n", len(result.Errors))
		for i, err := range result.Errors {
			fmt.Fprintf(outWriter, "  [%d] %s\n", i+1, err.Error())
		}
	} else {
		fmt.Fprintf(outWriter, "\nStatus: SUCCESS\n")
	}

	return nil
}

// init registers the template compile command
func init() {
	templateCompileCmd.Flags().StringVar(&compileArchetype, "archetype", "", "Path to archetype PPTX file (required)")
	templateCompileCmd.MarkFlagRequired("archetype")

	templateCompileCmd.Flags().StringVar(&compileOutput, "out", "", "Path to output PPTX file (required)")
	templateCompileCmd.MarkFlagRequired("out")

	templateCompileCmd.Flags().BoolVar(&compileContinueError, "continue-on-error", false, "Continue compilation even if individual slots fail to fill")
	templateCompileCmd.Flags().StringVar(&compileImageBaseDir, "image-base-dir", "", "Base directory for relative image paths in spec")

	templateCmd.AddCommand(templateCompileCmd)
}
