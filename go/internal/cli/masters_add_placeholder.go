package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
)

var (
	addMasterPlaceholderMaster string
	addMasterPlaceholderType   string
	addMasterPlaceholderBounds string
	addMasterPlaceholderIdx    int
	addMasterPlaceholderIdxSet bool
	addMasterPlaceholderSize   string
	addMasterPlaceholderOrient string
)

type addMasterPlaceholderResult struct {
	File      string `json:"file"`
	Output    string `json:"output,omitempty"`
	DryRun    bool   `json:"dryRun"`
	Layout    string `json:"layout"`
	Type      string `json:"type"`
	ShapeID   int    `json:"shapeId"`
	ShapeName string `json:"shapeName"`
	Idx       int    `json:"idx"`
	Master    int    `json:"master"`
	MasterURI string `json:"masterUri,omitempty"`
	PPTXMasterMutationReadbackCommands
}

var mastersAddPlaceholderCmd = &cobra.Command{
	Use:   "add-placeholder <file>",
	Short: "Add a text or picture placeholder to a master",
	Long: `Add a text or picture placeholder to an existing slide master.

Bounds format: x,y,cx,cy (in EMU units, 914400 EMU = 1 inch)

Examples:
  ooxml pptx masters add-placeholder deck.pptx --master 1 --type text --bounds 914400,914400,8229600,914400
  ooxml pptx masters add-placeholder deck.pptx --master 1 --type pic --bounds 1828800,1828800,6400000,4800000 --out out.pptx`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		inputPath := args[0]
		if _, err := os.Stat(inputPath); err != nil {
			return FileNotFoundError(inputPath)
		}
		if strings.TrimSpace(addMasterPlaceholderMaster) == "" {
			return InvalidArgsError("--master must be specified")
		}
		if strings.TrimSpace(addMasterPlaceholderType) == "" {
			return InvalidArgsError("--type must be specified (text or pic)")
		}
		if strings.TrimSpace(addMasterPlaceholderBounds) == "" {
			return InvalidArgsError("--bounds must be specified in format x,y,cx,cy")
		}

		addMasterPlaceholderIdxSet = cmd.Flags().Changed("idx")

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performAddMasterPlaceholder(inputPath, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputAddMasterPlaceholderJSON(cmd, result)
		}
		return outputAddMasterPlaceholderText(cmd, result)
	},
}

func performAddMasterPlaceholder(inputPath string, mutOpts *MutationOptions) (*addMasterPlaceholderResult, error) {
	writer, err := NewMutationWriter(inputPath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *addMasterPlaceholderResult
	destinationFile := mutationOutputPathForResult(inputPath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		// Parse masters
		masters, err := ParsePresentationMasters(pkg)
		if err != nil {
			return fmt.Errorf("failed to parse masters: %w", err)
		}

		// Find the master by index (masters are typically identified by 1-based index)
		var masterNum int
		_, err = fmt.Sscanf(addMasterPlaceholderMaster, "%d", &masterNum)
		if err != nil || masterNum <= 0 {
			return InvalidArgsError(fmt.Sprintf("invalid master index (must be positive integer): %s", addMasterPlaceholderMaster))
		}

		master := GetMasterByIndex(masters, masterNum)
		if master == nil {
			return missingMasterSelectorError(addMasterPlaceholderMaster, len(masters))
		}

		// Parse bounds
		x, y, cx, cy, err := parseBounds(addMasterPlaceholderBounds)
		if err != nil {
			return fmt.Errorf("invalid bounds: %w", err)
		}

		// Add placeholder based on type
		phType := strings.ToLower(strings.TrimSpace(addMasterPlaceholderType))
		if phType == "text" {
			phReq := &mutate.AddTextPlaceholderToMasterRequest{
				Package:         pkg,
				MasterPartURI:   master.PartURI,
				PlaceholderType: mutate.PlaceholderTypeBody,
				X:               x,
				Y:               y,
				CX:              cx,
				CY:              cy,
				Size:            addMasterPlaceholderSize,
				Orient:          addMasterPlaceholderOrient,
				Idx:             addMasterPlaceholderIdx,
				ExplicitIdx:     addMasterPlaceholderIdxSet,
			}
			phResult, err := mutate.AddTextPlaceholderToMaster(phReq)
			if err != nil {
				return fmt.Errorf("failed to add text placeholder: %w", err)
			}
			result = &addMasterPlaceholderResult{
				File:      inputPath,
				Output:    destinationFile,
				DryRun:    mutOpts.DryRun,
				Layout:    fmt.Sprintf("Master %d", masterNum),
				Type:      "text",
				ShapeID:   phResult.ShapeID,
				ShapeName: phResult.ShapeName,
				Idx:       phResult.Idx,
				Master:    masterNum,
				MasterURI: master.PartURI,
			}
			result.PPTXMasterMutationReadbackCommands = pptxMasterMutationReadbackCommands(destinationFile, masterNum)
		} else if phType == "pic" {
			phReq := &mutate.AddPicturePlaceholderToMasterRequest{
				Package:       pkg,
				MasterPartURI: master.PartURI,
				X:             x,
				Y:             y,
				CX:            cx,
				CY:            cy,
				Size:          addMasterPlaceholderSize,
				Orient:        addMasterPlaceholderOrient,
				Idx:           addMasterPlaceholderIdx,
				ExplicitIdx:   addMasterPlaceholderIdxSet,
			}
			phResult, err := mutate.AddPicturePlaceholderToMaster(phReq)
			if err != nil {
				return fmt.Errorf("failed to add picture placeholder: %w", err)
			}
			result = &addMasterPlaceholderResult{
				File:      inputPath,
				Output:    destinationFile,
				DryRun:    mutOpts.DryRun,
				Layout:    fmt.Sprintf("Master %d", masterNum),
				Type:      "pic",
				ShapeID:   phResult.ShapeID,
				ShapeName: phResult.ShapeName,
				Idx:       phResult.Idx,
				Master:    masterNum,
				MasterURI: master.PartURI,
			}
			result.PPTXMasterMutationReadbackCommands = pptxMasterMutationReadbackCommands(destinationFile, masterNum)
		} else {
			return fmt.Errorf("invalid placeholder type %q (must be 'text' or 'pic')", addMasterPlaceholderType)
		}

		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to add placeholder")
	}

	return result, nil
}

func missingMasterSelectorError(selector string, count int) error {
	candidates := make([]string, 0, count)
	for i := 1; i <= count; i++ {
		candidates = append(candidates, fmt.Sprintf("%d", i))
	}
	return SelectorNotFoundError("master", selector, candidates, "ooxml --json pptx masters list <file>")
}

func outputAddMasterPlaceholderJSON(cmd *cobra.Command, result *addMasterPlaceholderResult) error {
	config := GetGlobalConfig(cmd)
	data, err := marshalWithConfig(config, result)
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal add-master-placeholder JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputAddMasterPlaceholderText(cmd *cobra.Command, result *addMasterPlaceholderResult) error {
	return writeCLIOutput(cmd, []byte(fmt.Sprintf("Added %s placeholder (idx %d, shape ID %d) to master %d\n",
		result.Type, result.Idx, result.ShapeID, result.Master)))
}

func init() {
	mastersAddPlaceholderCmd.Flags().StringVar(&addMasterPlaceholderMaster, "master", "", "master index (1-based)")
	mastersAddPlaceholderCmd.Flags().StringVar(&addMasterPlaceholderType, "type", "", "placeholder type: text or pic")
	mastersAddPlaceholderCmd.Flags().StringVar(&addMasterPlaceholderBounds, "bounds", "", "bounds in EMU units: x,y,cx,cy")
	mastersAddPlaceholderCmd.Flags().IntVar(&addMasterPlaceholderIdx, "idx", -1, "placeholder index (-1 = auto-allocate; 0 and above are explicit)")
	mastersAddPlaceholderCmd.Flags().StringVar(&addMasterPlaceholderSize, "size", "", "optional placeholder size enum (e.g. 'full', 'half')")
	mastersAddPlaceholderCmd.Flags().StringVar(&addMasterPlaceholderOrient, "orient", "", "optional placeholder orientation")

	mastersAddPlaceholderCmd.MarkFlagRequired("master")
	mastersAddPlaceholderCmd.MarkFlagRequired("type")
	mastersAddPlaceholderCmd.MarkFlagRequired("bounds")

	AddMutationFlags(mastersAddPlaceholderCmd)

	mastersCmd.AddCommand(mastersAddPlaceholderCmd)
}
