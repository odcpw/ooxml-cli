package cli

import (
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	xlsxmutate "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

type XLSXSheetsDeleteResult struct {
	File                  string                         `json:"file"`
	Number                int                            `json:"number"`
	Name                  string                         `json:"name"`
	SheetID               string                         `json:"sheetId"`
	RelationshipID        string                         `json:"relationshipId"`
	PartURI               string                         `json:"partUri"`
	RemovedRelationshipID string                         `json:"removedRelationshipId"`
	RemovedParts          []string                       `json:"removedParts"`
	RemainingSheets       int                            `json:"remainingSheets"`
	Output                string                         `json:"output,omitempty"`
	DryRun                bool                           `json:"dryRun"`
	Deleted               *model.SheetRef                `json:"deleted,omitempty"`
	Destination           *XLSXSheetsMutationDestination `json:"destination,omitempty"`
	XLSXSheetsMutationReadbackCommands
}

var xlsxSheetsDeleteSheet string

var xlsxSheetsDeleteCmd = &cobra.Command{
	Use:   "delete <file>",
	Short: "Delete an XLSX worksheet",
	Long:  "Delete one worksheet from an XLSX workbook and remove its workbook relationship and worksheet part.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if xlsxSheetsDeleteSheet == "" {
			return InvalidArgsError("--sheet is required")
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		wantReadback := GetGlobalConfig(cmd).Format == "json"
		result, err := performXLSXSheetsDelete(filePath, xlsxSheetsDeleteSheet, mutOpts, wantReadback)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXSheetsJSON(cmd, result, "sheets delete")
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("deleted sheet %d %q", result.Number, result.Name)))
	},
}

func performXLSXSheetsDelete(filePath, sheetSelector string, mutOpts *MutationOptions, wantReadback bool) (*XLSXSheetsDeleteResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}

	var result *XLSXSheetsDeleteResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheetRef, err := selectXLSXSheet(workbook.Sheets, sheetSelector)
		if err != nil {
			return err
		}
		if err := requireXLSXWorksheetRef(sheetRef); err != nil {
			return err
		}
		deleteResult, err := xlsxmutate.DeleteSheet(&xlsxmutate.DeleteSheetRequest{
			Package:        pkg,
			WorkbookURI:    workbook.PartURI,
			ExistingSheets: workbook.Sheets,
			SheetRef:       sheetRef,
		})
		if err != nil {
			return mapXLSXSheetMutationError(err)
		}
		outputPath := mutationOutputPathForResult(filePath, mutOpts)
		var destination *XLSXSheetsMutationDestination
		if wantReadback {
			destination, err = collectXLSXSheetsMutationDestination(pkg, outputPath, "", "")
			if err != nil {
				return err
			}
		}
		deleted := sheetRef
		result = &XLSXSheetsDeleteResult{
			File:                  filePath,
			Number:                deleteResult.Number,
			Name:                  deleteResult.Name,
			SheetID:               deleteResult.SheetID,
			RelationshipID:        deleteResult.RelationshipID,
			PartURI:               deleteResult.PartURI,
			RemovedRelationshipID: deleteResult.RemovedRelationshipID,
			RemovedParts:          deleteResult.RemovedParts,
			RemainingSheets:       deleteResult.RemainingSheets,
			Output:                outputPath,
			DryRun:                mutOpts != nil && mutOpts.DryRun,
			Deleted:               &deleted,
			Destination:           destination,
		}
		result.XLSXSheetsMutationReadbackCommands = xlsxSheetsMutationReadbackCommands(destination)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func init() {
	xlsxSheetsDeleteCmd.Flags().StringVar(&xlsxSheetsDeleteSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	AddMutationFlags(xlsxSheetsDeleteCmd)
	xlsxSheetsCmd.AddCommand(xlsxSheetsDeleteCmd)
}
