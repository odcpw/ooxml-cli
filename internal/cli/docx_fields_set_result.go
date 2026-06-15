package cli

import (
	"fmt"
	"os"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

// DOCXFieldsSetResultResult is the JSON readback after setting a field result.
type DOCXFieldsSetResultResult struct {
	File            string `json:"file"`
	Operation       string `json:"operation"`
	PartURI         string `json:"partUri"`
	BlockIndex      int    `json:"blockIndex"`
	FieldIndex      int    `json:"fieldIndex"`
	FieldType       string `json:"fieldType"`
	Instruction     string `json:"instruction"`
	PreviousResult  string `json:"previousResult"`
	CachedResult    string `json:"cachedResult"`
	Location        string `json:"location"`
	Note            string `json:"note"`
	ListCommand     string `json:"listCommand"`
	ValidateCommand string `json:"validateCommand"`
}

func newDOCXFieldsSetResultCmd() *cobra.Command {
	var (
		selector   string
		result     string
		expectHash string
	)
	cmd := &cobra.Command{
		Use:   "set-result <file>",
		Short: "Set a field's cached result text (w:fldSimple or complex w:fldChar field)",
		Long: "Set the cached result text of an existing field selected by --selector part:block:field " +
			"(e.g. body:1:0 or header1:1:0). The result is a cache only; Word recomputes it on recalculation. " +
			"Fields nested inside body tables are listed by 'docx fields list' with editable=false and are " +
			"NOT addressable by this selector; targeting one returns a clear error rather than mis-editing.",
		Args: cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			filePath := args[0]
			if _, err := os.Stat(filePath); err != nil {
				return FileNotFoundError(filePath)
			}
			if selector == "" {
				return InvalidArgsError("--selector is required (e.g. body:1:0 or header1:1:0)")
			}
			if !cmd.Flags().Lookup("result").Changed {
				return InvalidArgsError("--result is required")
			}
			loc, err := parseFieldLocation(selector)
			if err != nil {
				return err
			}
			if !loc.hasField {
				return InvalidArgsError(fmt.Sprintf("invalid selector %q: a field index is required (e.g. body:1:0)", selector))
			}
			mutOpts, err := GetValidatedMutationOptions(cmd)
			if err != nil {
				return err
			}

			res, err := performDOCXFieldsSetResult(filePath, loc, result, expectHash, mutOpts)
			if err != nil {
				return err
			}
			if GetGlobalConfig(cmd).Format == "json" {
				return outputDOCXFieldJSON(cmd, res, "fields set-result")
			}
			return writeCLIOutput(cmd, []byte(fmt.Sprintf("set %s field %s result = %q at %s", res.FieldType, res.Instruction, res.CachedResult, res.Location)))
		},
	}
	cmd.Flags().StringVar(&selector, "selector", "", "field selector part:block:field (e.g. body:1:0 or header1:1:0)")
	cmd.Flags().StringVar(&result, "result", "", "new cached result text")
	cmd.Flags().StringVar(&expectHash, "expect-hash", "", "guard: sha256 of the field's instruction+result before mutation")
	AddMutationFlags(cmd)
	return cmd
}

func performDOCXFieldsSetResult(filePath string, loc *fieldLocation, result, expectHash string, mutOpts *MutationOptions) (*DOCXFieldsSetResultResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	destination := mutOpts.OutPath
	if mutOpts.InPlace {
		destination = filePath
	}

	var out *DOCXFieldsSetResultResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}
		hfListing, err := docxinspect.ListHeadersFooters(pkg, documentURI)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to resolve headers/footers: %v", err)
		}
		partURI, err := resolvePartURIForLocation(loc, documentURI, headerFooterLabelMap(hfListing))
		if err != nil {
			return err
		}

		setResult, err := docxmutate.SetFieldResult(&docxmutate.SetFieldResultRequest{
			Package:      pkg,
			DocumentURI:  documentURI,
			PartURI:      partURI,
			BlockIndex:   loc.blockIndex,
			FieldIndex:   loc.fieldIndex,
			Result:       result,
			ExpectedHash: expectHash,
		})
		if err != nil {
			return mapDOCXFieldMutationError(err)
		}

		out = &DOCXFieldsSetResultResult{
			File:           filePath,
			Operation:      "set-result",
			PartURI:        setResult.PartURI,
			BlockIndex:     setResult.BlockIndex,
			FieldIndex:     setResult.FieldIndex,
			FieldType:      setResult.FieldType,
			Instruction:    setResult.Instruction,
			PreviousResult: setResult.PreviousResult,
			CachedResult:   setResult.CachedResult,
			Location:       setResult.Location,
			Note:           "cachedResult is a cache; Word recomputes the live value on field recalculation",
		}
		return nil
	}); err != nil {
		return nil, err
	}

	readbackTarget := destination
	if readbackTarget == "" {
		readbackTarget = filePath
	}
	out.ListCommand = docxFieldsListCommand(readbackTarget)
	out.ValidateCommand = docxValidateStrictCommand(readbackTarget)
	return out, nil
}

func init() {
	docxFieldsCmd.AddCommand(newDOCXFieldsSetResultCmd())
}
