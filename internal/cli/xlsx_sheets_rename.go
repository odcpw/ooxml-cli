package cli

import (
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	xlsxmutate "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

type XLSXSheetsRenameResult struct {
	File           string                         `json:"file"`
	Number         int                            `json:"number"`
	Name           string                         `json:"name"`
	PreviousName   string                         `json:"previousName"`
	SheetID        string                         `json:"sheetId"`
	RelationshipID string                         `json:"relationshipId"`
	PartURI        string                         `json:"partUri"`
	Output         string                         `json:"output,omitempty"`
	DryRun         bool                           `json:"dryRun"`
	Destination    *XLSXSheetsMutationDestination `json:"destination,omitempty"`
	XLSXSheetsMutationReadbackCommands
}

var (
	xlsxSheetsRenameSheet string
	xlsxSheetsRenameName  string
)

var xlsxSheetsRenameCmd = &cobra.Command{
	Use:   "rename <file>",
	Short: "Rename an XLSX worksheet",
	Long:  "Rename a workbook sheet by sheet number or exact sheet name.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if xlsxSheetsRenameSheet == "" {
			return InvalidArgsError("--sheet is required")
		}
		if xlsxSheetsRenameName == "" {
			return InvalidArgsError("--name is required")
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		wantReadback := GetGlobalConfig(cmd).Format == "json"
		result, err := performXLSXSheetsRename(filePath, xlsxSheetsRenameSheet, xlsxSheetsRenameName, mutOpts, wantReadback)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXSheetsJSON(cmd, result, "sheets rename")
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("renamed sheet %d %q -> %q", result.Number, result.PreviousName, result.Name)))
	},
}

func performXLSXSheetsRename(filePath, sheetSelector, name string, mutOpts *MutationOptions, wantReadback bool) (*XLSXSheetsRenameResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}

	var result *XLSXSheetsRenameResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheetRef, err := selectXLSXSheet(workbook.Sheets, sheetSelector)
		if err != nil {
			return err
		}
		renameResult, err := xlsxmutate.RenameSheet(&xlsxmutate.RenameSheetRequest{
			Package:        pkg,
			WorkbookURI:    workbook.PartURI,
			ExistingSheets: workbook.Sheets,
			SheetRef:       sheetRef,
			Name:           name,
		})
		if err != nil {
			return mapXLSXSheetMutationError(err)
		}
		outputPath := mutationOutputPathForResult(filePath, mutOpts)
		var destination *XLSXSheetsMutationDestination
		if wantReadback {
			destination, err = collectXLSXSheetsMutationDestination(pkg, outputPath, sheetRef.RelationshipID, renameResult.PartURI)
			if err != nil {
				return err
			}
		}
		result = &XLSXSheetsRenameResult{
			File:           filePath,
			Number:         renameResult.Number,
			Name:           renameResult.Name,
			PreviousName:   renameResult.PreviousName,
			SheetID:        renameResult.SheetID,
			RelationshipID: sheetRef.RelationshipID,
			PartURI:        renameResult.PartURI,
			Output:         outputPath,
			DryRun:         mutOpts != nil && mutOpts.DryRun,
			Destination:    destination,
		}
		result.XLSXSheetsMutationReadbackCommands = xlsxSheetsMutationReadbackCommands(destination)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func init() {
	xlsxSheetsRenameCmd.Flags().StringVar(&xlsxSheetsRenameSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxSheetsRenameCmd.Flags().StringVar(&xlsxSheetsRenameName, "name", "", "new worksheet name")
	AddMutationFlags(xlsxSheetsRenameCmd)
	xlsxSheetsCmd.AddCommand(xlsxSheetsRenameCmd)
}
