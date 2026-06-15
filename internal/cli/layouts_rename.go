package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
)

var (
	layoutRenameLayout string
	layoutRenameName   string
)

type renameLayoutOutput struct {
	File      string `json:"file"`
	Output    string `json:"output,omitempty"`
	DryRun    bool   `json:"dryRun"`
	LayoutURI string `json:"layoutUri"`
	OldName   string `json:"oldName"`
	NewName   string `json:"newName"`
	PPTXLayoutMutationReadbackCommands
}

var layoutsRenameCmd = &cobra.Command{
	Use:   "rename <file>",
	Short: "Rename an existing layout",
	Long: `Rename an existing slide layout by updating its p:cSld@name.

Examples:
  ooxml pptx layouts rename deck.pptx --layout "Title and Content" --name "Image Grid" --out out.pptx
  ooxml pptx layouts rename deck.pptx --layout 2 --name "7pictures" --in-place`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		inputPath := args[0]
		if _, err := os.Stat(inputPath); err != nil {
			return FileNotFoundError(inputPath)
		}
		if strings.TrimSpace(layoutRenameLayout) == "" {
			return InvalidArgsError("--layout must be specified")
		}
		if strings.TrimSpace(layoutRenameName) == "" {
			return InvalidArgsError("--name must be specified")
		}

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performRenameLayout(inputPath, mutOpts)
		if err != nil {
			return err
		}

		data, err := marshalWithConfig(GetGlobalConfig(cmd), result)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to marshal rename result: %v", err)
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeCLIOutput(cmd, data)
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("Renamed layout %q to %q\n", result.OldName, result.NewName)))
	},
}

func performRenameLayout(inputPath string, mutOpts *MutationOptions) (*renameLayoutOutput, error) {
	writer, err := NewMutationWriter(inputPath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *renameLayoutOutput
	destinationFile := mutationOutputPathForResult(inputPath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return fmt.Errorf("failed to parse presentation: %w", err)
		}
		layoutURI, err := resolveLayoutSelector(graph, layoutRenameLayout)
		if err != nil {
			return err
		}
		if layoutNameExists(graph, layoutRenameName, layoutURI) {
			return fmt.Errorf("layout name already exists: %s", layoutRenameName)
		}
		renamed, err := mutate.RenameLayout(&mutate.RenameLayoutRequest{
			Package:       pkg,
			LayoutPartURI: layoutURI,
			NewName:       layoutRenameName,
		})
		if err != nil {
			return err
		}
		result = &renameLayoutOutput{
			File:      inputPath,
			Output:    destinationFile,
			DryRun:    mutOpts.DryRun,
			LayoutURI: renamed.LayoutPartURI,
			OldName:   renamed.OldName,
			NewName:   renamed.NewName,
		}
		result.PPTXLayoutMutationReadbackCommands = pptxLayoutMutationReadbackCommands(destinationFile, renamed.NewName)
		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to rename layout")
	}
	return result, nil
}

func init() {
	layoutsRenameCmd.Flags().StringVar(&layoutRenameLayout, "layout", "", "layout number (1-based) or exact layout name")
	layoutsRenameCmd.Flags().StringVar(&layoutRenameName, "name", "", "new layout name")
	layoutsRenameCmd.MarkFlagRequired("layout")
	layoutsRenameCmd.MarkFlagRequired("name")
	AddMutationFlags(layoutsRenameCmd)
	layoutsCmd.AddCommand(layoutsRenameCmd)
}
