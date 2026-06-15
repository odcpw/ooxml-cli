package cli

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
)

var (
	importMasterSourcePath  string
	importMasterIndex       int
	importMasterThemePolicy string
)

type importMasterResult struct {
	File            string `json:"file"`
	Output          string `json:"output,omitempty"`
	DryRun          bool   `json:"dryRun"`
	TargetMasterURI string `json:"targetMasterUri"`
	TargetMaster    int    `json:"targetMaster"`
	ThemeURI        string `json:"themeUri,omitempty"`
	Imported        bool   `json:"imported"`
	LayoutCount     int    `json:"layoutCount"`
	PPTXMasterMutationReadbackCommands
}

var importMasterCmd = &cobra.Command{
	Use:   "import <target-file>",
	Short: "Import a slide master and its layouts from another presentation",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		targetPath := args[0]
		if _, err := os.Stat(targetPath); err != nil {
			return FileNotFoundError(targetPath)
		}
		if importMasterSourcePath == "" {
			return InvalidArgsError("--source is required")
		}
		if _, err := os.Stat(importMasterSourcePath); err != nil {
			return FileNotFoundError(importMasterSourcePath)
		}
		if importMasterIndex < 1 {
			return InvalidArgsError("--master must be >= 1")
		}

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performImportMaster(targetPath, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputImportMasterJSON(cmd, result)
		}
		return outputImportMasterText(cmd, result)
	},
}

func performImportMaster(targetPath string, mutOpts *MutationOptions) (*importMasterResult, error) {
	sourcePkg, err := openPackageExpectType(importMasterSourcePath, opc.PackageTypePPTX)
	if err != nil {
		return nil, err
	}
	defer sourcePkg.Close()

	sourceMasters, err := ParsePresentationMasters(sourcePkg)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to inspect source masters: %v", err)
	}
	sourceMaster := GetMasterByIndex(sourceMasters, importMasterIndex)
	if sourceMaster == nil {
		return nil, NewCLIErrorf(ExitInvalidArgs, "master %d is out of range (1-%d)", importMasterIndex, len(sourceMasters))
	}

	var result *importMasterResult
	destinationFile := mutationOutputPathForResult(targetPath, mutOpts)
	writer, err := NewMutationWriter(targetPath, mutOpts)
	if err != nil {
		return nil, err
	}
	if err := writer.Write(func(pkg opc.PackageSession) error {
		beforeMasters, err := ParsePresentationMasters(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to inspect target masters: %v", err)
		}
		imported, err := mutate.ImportMaster(&mutate.ImportMasterRequest{
			TargetPackage:   pkg,
			SourcePackage:   sourcePkg,
			SourceMasterURI: sourceMaster.PartURI,
			ThemePolicy:     importMasterThemePolicy,
		})
		if err != nil {
			return err
		}
		targetMasterIndex := len(beforeMasters) + 1
		if !imported.Imported {
			targetMasters, err := ParsePresentationMasters(pkg)
			if err == nil {
				for _, master := range targetMasters {
					if master.PartURI == imported.TargetMasterURI {
						targetMasterIndex = master.Index
						break
					}
				}
			}
		}
		result = &importMasterResult{
			File:            targetPath,
			Output:          destinationFile,
			DryRun:          mutOpts.DryRun,
			TargetMasterURI: imported.TargetMasterURI,
			TargetMaster:    targetMasterIndex,
			ThemeURI:        imported.ThemeURI,
			Imported:        imported.Imported,
			LayoutCount:     len(imported.Layouts),
		}
		result.PPTXMasterMutationReadbackCommands = pptxMasterMutationReadbackCommands(destinationFile, targetMasterIndex)
		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to import master")
	}

	return result, nil
}

func outputImportMasterJSON(cmd *cobra.Command, result *importMasterResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal import-master JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputImportMasterText(cmd *cobra.Command, result *importMasterResult) error {
	status := "reused"
	if result.Imported {
		status = "imported"
	}
	text := fmt.Sprintf("%s master %s with %d layout(s)\n", status, result.TargetMasterURI, result.LayoutCount)
	return writeCLIOutput(cmd, []byte(text))
}

func init() {
	importMasterCmd.Flags().StringVar(&importMasterSourcePath, "source", "", "path to the source presentation file")
	importMasterCmd.Flags().IntVar(&importMasterIndex, "master", 0, "1-based master number in source presentation")
	importMasterCmd.Flags().StringVar(&importMasterThemePolicy, "theme-policy", "reuse", "theme handling: 'reuse' or 'import'")
	importMasterCmd.MarkFlagRequired("source")
	importMasterCmd.MarkFlagRequired("master")
	AddMutationFlags(importMasterCmd)
	mastersCmd.AddCommand(importMasterCmd)
}
