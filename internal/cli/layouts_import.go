package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strconv"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
)

var (
	importLayoutSourcePath  string
	importLayoutSelector    string
	importLayoutThemePolicy string
)

type importLayoutResult struct {
	File            string `json:"file"`
	Output          string `json:"output,omitempty"`
	DryRun          bool   `json:"dryRun"`
	TargetLayoutURI string `json:"targetLayoutUri"`
	TargetMasterURI string `json:"targetMasterUri"`
	ThemeURI        string `json:"themeUri,omitempty"`
	Name            string `json:"name,omitempty"`
	Imported        bool   `json:"imported"`
	MasterImported  bool   `json:"masterImported"`
	PPTXLayoutMutationReadbackCommands
}

var importLayoutCmd = &cobra.Command{
	Use:   "import <target-file>",
	Short: "Import one layout from another presentation",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		targetPath := args[0]
		if _, err := os.Stat(targetPath); err != nil {
			return FileNotFoundError(targetPath)
		}
		if importLayoutSourcePath == "" {
			return InvalidArgsError("--source is required")
		}
		if _, err := os.Stat(importLayoutSourcePath); err != nil {
			return FileNotFoundError(importLayoutSourcePath)
		}
		if importLayoutSelector == "" {
			return InvalidArgsError("--layout is required")
		}

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performImportLayout(targetPath, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputImportLayoutJSON(cmd, result)
		}
		return outputImportLayoutText(cmd, result)
	},
}

func performImportLayout(targetPath string, mutOpts *MutationOptions) (*importLayoutResult, error) {
	sourcePkg, err := openPackageExpectType(importLayoutSourcePath, opc.PackageTypePPTX)
	if err != nil {
		return nil, err
	}
	defer sourcePkg.Close()

	sourceLayouts, err := ParsePresentationLayouts(sourcePkg)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to inspect source layouts: %v", err)
	}
	layout := resolveImportLayoutSelector(sourceLayouts, importLayoutSelector)
	if layout == nil {
		return nil, NewCLIErrorf(ExitInvalidArgs, "layout not found: %s", importLayoutSelector)
	}

	var result *importLayoutResult
	destinationFile := mutationOutputPathForResult(targetPath, mutOpts)
	writer, err := NewMutationWriter(targetPath, mutOpts)
	if err != nil {
		return nil, err
	}
	if err := writer.Write(func(pkg opc.PackageSession) error {
		imported, err := mutate.ImportLayout(&mutate.ImportLayoutRequest{
			TargetPackage:   pkg,
			SourcePackage:   sourcePkg,
			SourceLayoutURI: layout.PartURI,
			ThemePolicy:     importLayoutThemePolicy,
		})
		if err != nil {
			return err
		}
		result = &importLayoutResult{
			File:            targetPath,
			Output:          destinationFile,
			DryRun:          mutOpts.DryRun,
			TargetLayoutURI: imported.TargetLayoutURI,
			TargetMasterURI: imported.TargetMasterURI,
			ThemeURI:        imported.ThemeURI,
			Name:            imported.Name,
			Imported:        imported.Imported,
			MasterImported:  imported.MasterImported,
		}
		result.PPTXLayoutMutationReadbackCommands = pptxLayoutMutationReadbackCommands(destinationFile, imported.Name)
		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to import layout")
	}

	return result, nil
}

func resolveImportLayoutSelector(layouts []*LayoutInfo, selector string) *LayoutInfo {
	if num, err := strconv.Atoi(selector); err == nil {
		return GetLayoutByNumber(layouts, num)
	}
	return GetLayoutByName(layouts, selector)
}

func outputImportLayoutJSON(cmd *cobra.Command, result *importLayoutResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal import-layout JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputImportLayoutText(cmd *cobra.Command, result *importLayoutResult) error {
	status := "reused"
	if result.Imported {
		status = "imported"
	}
	text := fmt.Sprintf("%s layout %s under master %s\n", status, result.TargetLayoutURI, result.TargetMasterURI)
	return writeCLIOutput(cmd, []byte(text))
}

func init() {
	importLayoutCmd.Flags().StringVar(&importLayoutSourcePath, "source", "", "path to the source presentation file")
	importLayoutCmd.Flags().StringVar(&importLayoutSelector, "layout", "", "layout number (1-based) or exact layout name in the source presentation")
	importLayoutCmd.Flags().StringVar(&importLayoutThemePolicy, "theme-policy", "reuse", "theme handling: 'reuse' or 'import'")
	importLayoutCmd.MarkFlagRequired("source")
	importLayoutCmd.MarkFlagRequired("layout")
	AddMutationFlags(importLayoutCmd)
	layoutsCmd.AddCommand(importLayoutCmd)
}
