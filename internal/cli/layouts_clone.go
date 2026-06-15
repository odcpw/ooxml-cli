package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
)

var (
	layoutCloneLayout string
	layoutCloneName   string
)

type cloneLayoutOutput struct {
	File         string `json:"file"`
	Output       string `json:"output,omitempty"`
	DryRun       bool   `json:"dryRun"`
	SourceLayout string `json:"sourceLayout"`
	SourceURI    string `json:"sourceUri"`
	NewLayout    string `json:"newLayout"`
	NewURI       string `json:"newUri"`
	MasterURI    string `json:"masterUri"`
	Relationship string `json:"relationshipId"`
	LayoutID     uint32 `json:"layoutId"`
	PPTXLayoutMutationReadbackCommands
}

var layoutsCloneCmd = &cobra.Command{
	Use:   "clone <file>",
	Short: "Clone an existing layout under the same master",
	Long: `Clone an existing slide layout, keep it attached to the same master,
and assign a new layout name.

Examples:
  ooxml pptx layouts clone deck.pptx --layout "Title and Content" --name "7pictures" --out out.pptx
  ooxml pptx layouts clone deck.pptx --layout 2 --name "Image Grid" --in-place`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		inputPath := args[0]
		if _, err := os.Stat(inputPath); err != nil {
			return FileNotFoundError(inputPath)
		}
		if strings.TrimSpace(layoutCloneLayout) == "" {
			return InvalidArgsError("--layout must be specified")
		}
		if strings.TrimSpace(layoutCloneName) == "" {
			return InvalidArgsError("--name must be specified")
		}

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performCloneLayout(inputPath, mutOpts)
		if err != nil {
			return err
		}

		data, err := marshalWithConfig(GetGlobalConfig(cmd), result)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to marshal clone result: %v", err)
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeCLIOutput(cmd, data)
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("Cloned layout %q to %q\n", result.SourceLayout, result.NewLayout)))
	},
}

func performCloneLayout(inputPath string, mutOpts *MutationOptions) (*cloneLayoutOutput, error) {
	writer, err := NewMutationWriter(inputPath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *cloneLayoutOutput
	destinationFile := mutationOutputPathForResult(inputPath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return fmt.Errorf("failed to parse presentation: %w", err)
		}
		layoutURI, err := resolveLayoutSelector(graph, layoutCloneLayout)
		if err != nil {
			return err
		}
		if layoutNameExists(graph, layoutCloneName, "") {
			return fmt.Errorf("layout name already exists: %s", layoutCloneName)
		}
		cloned, err := mutate.CloneLayout(&mutate.CloneLayoutRequest{
			Package:       pkg,
			LayoutPartURI: layoutURI,
			NewName:       layoutCloneName,
		})
		if err != nil {
			return err
		}
		result = &cloneLayoutOutput{
			File:         inputPath,
			Output:       destinationFile,
			DryRun:       mutOpts.DryRun,
			SourceLayout: cloned.OldName,
			SourceURI:    cloned.SourceLayoutURI,
			NewLayout:    cloned.NewName,
			NewURI:       cloned.NewLayoutURI,
			MasterURI:    cloned.MasterPartURI,
			Relationship: cloned.RelationshipID,
			LayoutID:     cloned.LayoutID,
		}
		result.PPTXLayoutMutationReadbackCommands = pptxLayoutMutationReadbackCommands(destinationFile, cloned.NewName)
		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to clone layout")
	}
	return result, nil
}

func init() {
	layoutsCloneCmd.Flags().StringVar(&layoutCloneLayout, "layout", "", "layout number (1-based) or exact layout name")
	layoutsCloneCmd.Flags().StringVar(&layoutCloneName, "name", "", "new layout name")
	layoutsCloneCmd.MarkFlagRequired("layout")
	layoutsCloneCmd.MarkFlagRequired("name")
	AddMutationFlags(layoutsCloneCmd)
	layoutsCmd.AddCommand(layoutsCloneCmd)
}

func marshalWithConfig(config *GlobalConfig, value any) ([]byte, error) {
	if config != nil && config.Pretty {
		return json.MarshalIndent(value, "", "  ")
	}
	return json.Marshal(value)
}
