package cli

import (
	"fmt"
	"os"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

// DOCXFieldsInsertResult is the JSON readback after inserting a field.
type DOCXFieldsInsertResult struct {
	File            string `json:"file"`
	Operation       string `json:"operation"`
	PartURI         string `json:"partUri"`
	BlockIndex      int    `json:"blockIndex"`
	FieldIndex      int    `json:"fieldIndex"`
	FieldType       string `json:"fieldType"`
	Instruction     string `json:"instruction"`
	CachedResult    string `json:"cachedResult"`
	Location        string `json:"location"`
	ParagraphText   string `json:"paragraphText"`
	KnownCode       bool   `json:"knownCode"`
	Warning         string `json:"warning,omitempty"`
	ListCommand     string `json:"listCommand"`
	ValidateCommand string `json:"validateCommand"`
}

func newDOCXFieldsInsertCmd() *cobra.Command {
	var (
		location  string
		fieldCode string
		result    string
	)
	cmd := &cobra.Command{
		Use:   "insert <file>",
		Short: "Insert a field code into a paragraph (PAGE, NUMPAGES, DATE)",
		Long:  "Insert a simple field (w:fldSimple) into a body block or header/footer paragraph selected by --location (e.g. body:2 or header1:1).",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			filePath := args[0]
			if _, err := os.Stat(filePath); err != nil {
				return FileNotFoundError(filePath)
			}
			if fieldCode == "" {
				return InvalidArgsError("--field-code is required (e.g. PAGE)")
			}
			if location == "" {
				return InvalidArgsError("--location is required (e.g. body:2 or header1:1)")
			}
			loc, err := parseFieldLocation(location)
			if err != nil {
				return err
			}
			mutOpts, err := GetValidatedMutationOptions(cmd)
			if err != nil {
				return err
			}

			res, err := performDOCXFieldsInsert(filePath, loc, fieldCode, result, mutOpts)
			if err != nil {
				return err
			}
			if GetGlobalConfig(cmd).Format == "json" {
				return outputDOCXFieldJSON(cmd, res, "fields insert")
			}
			return writeCLIOutput(cmd, []byte(fmt.Sprintf("inserted %s field %s at %s", res.FieldType, res.Instruction, res.Location)))
		},
	}
	cmd.Flags().StringVar(&location, "location", "", "target location part:block (e.g. body:2 or header1:1)")
	cmd.Flags().StringVar(&fieldCode, "field-code", "", "field instruction (e.g. PAGE, NUMPAGES, DATE)")
	cmd.Flags().StringVar(&result, "result", "", "initial cached result text (optional)")
	AddMutationFlags(cmd)
	return cmd
}

func performDOCXFieldsInsert(filePath string, loc *fieldLocation, fieldCode, result string, mutOpts *MutationOptions) (*DOCXFieldsInsertResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	destination := mutOpts.OutPath
	if mutOpts.InPlace {
		destination = filePath
	}

	var out *DOCXFieldsInsertResult
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

		inserted, err := docxmutate.InsertField(&docxmutate.InsertFieldRequest{
			Package:     pkg,
			DocumentURI: documentURI,
			PartURI:     partURI,
			BlockIndex:  loc.blockIndex,
			FieldCode:   fieldCode,
			ResultText:  result,
		})
		if err != nil {
			return mapDOCXFieldMutationError(err)
		}

		out = &DOCXFieldsInsertResult{
			File:          filePath,
			Operation:     "inserted",
			PartURI:       inserted.PartURI,
			BlockIndex:    inserted.BlockIndex,
			FieldIndex:    inserted.FieldIndex,
			FieldType:     inserted.FieldType,
			Instruction:   inserted.Instruction,
			CachedResult:  inserted.CachedResult,
			Location:      inserted.Location,
			ParagraphText: inserted.ParagraphText,
			KnownCode:     inserted.KnownCode,
		}
		if !inserted.KnownCode {
			out.Warning = fmt.Sprintf("field code %q is not a recognized instruction; inserted as-is (switches are not parsed)", inserted.Instruction)
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
	docxFieldsCmd.AddCommand(newDOCXFieldsInsertCmd())
}
