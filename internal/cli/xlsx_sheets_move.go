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

type XLSXSheetsMoveResult struct {
	File           string                         `json:"file"`
	Number         int                            `json:"number"`
	Name           string                         `json:"name"`
	SheetID        string                         `json:"sheetId"`
	RelationshipID string                         `json:"relationshipId"`
	PartURI        string                         `json:"partUri"`
	FromPosition   int                            `json:"fromPosition"`
	ToPosition     int                            `json:"toPosition"`
	IsNoOp         bool                           `json:"isNoOp"`
	Output         string                         `json:"output,omitempty"`
	DryRun         bool                           `json:"dryRun"`
	Destination    *XLSXSheetsMutationDestination `json:"destination,omitempty"`
	XLSXSheetsMutationReadbackCommands
}

var (
	xlsxSheetsMoveSheet  string
	xlsxSheetsMoveTo     int
	xlsxSheetsMoveBefore string
	xlsxSheetsMoveAfter  string
)

var xlsxSheetsMoveCmd = &cobra.Command{
	Use:   "move <file>",
	Short: "Move an XLSX workbook sheet",
	Long:  "Move a workbook sheet in tab order. The worksheet part, sheetId, and relationship ID are preserved.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if xlsxSheetsMoveSheet == "" {
			return InvalidArgsError("--sheet is required")
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		wantReadback := GetGlobalConfig(cmd).Format == "json"
		result, err := performXLSXSheetsMove(filePath, xlsxSheetsMoveSheet, xlsxSheetsMoveTo, xlsxSheetsMoveBefore, xlsxSheetsMoveAfter, mutOpts, cmd, wantReadback)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXSheetsJSON(cmd, result, "sheets move")
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("moved sheet %q from %d to %d", result.Name, result.FromPosition, result.ToPosition)))
	},
}

func performXLSXSheetsMove(filePath, sheetSelector string, to int, before, after string, mutOpts *MutationOptions, cmd *cobra.Command, wantReadback bool) (*XLSXSheetsMoveResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}

	var result *XLSXSheetsMoveResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheetRef, err := selectXLSXSheet(workbook.Sheets, sheetSelector)
		if err != nil {
			return err
		}
		targetPosition, err := resolveXLSXMoveTargetPosition(cmd, workbook.Sheets, sheetRef, to, before, after)
		if err != nil {
			return err
		}
		moveResult, err := xlsxmutate.MoveSheet(&xlsxmutate.MoveSheetRequest{
			Package:        pkg,
			WorkbookURI:    workbook.PartURI,
			ExistingSheets: workbook.Sheets,
			SheetRef:       sheetRef,
			TargetPosition: targetPosition,
		})
		if err != nil {
			return mapXLSXSheetMutationError(err)
		}
		outputPath := mutationOutputPathForResult(filePath, mutOpts)
		var destination *XLSXSheetsMutationDestination
		if wantReadback {
			destination, err = collectXLSXSheetsMutationDestination(pkg, outputPath, moveResult.RelationshipID, moveResult.PartURI)
			if err != nil {
				return err
			}
		}
		result = &XLSXSheetsMoveResult{
			File:           filePath,
			Number:         moveResult.Number,
			Name:           moveResult.Name,
			SheetID:        moveResult.SheetID,
			RelationshipID: moveResult.RelationshipID,
			PartURI:        moveResult.PartURI,
			FromPosition:   moveResult.OldPosition,
			ToPosition:     moveResult.NewPosition,
			IsNoOp:         moveResult.IsNoOp,
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

func resolveXLSXMoveTargetPosition(cmd *cobra.Command, sheets []model.SheetRef, moving model.SheetRef, to int, before, after string) (int, error) {
	selected := 0
	if cmd.Flags().Lookup("to").Changed {
		selected++
	}
	if before != "" {
		selected++
	}
	if after != "" {
		selected++
	}
	if selected != 1 {
		return 0, InvalidArgsError("must specify exactly one of --to, --before, or --after")
	}
	if cmd.Flags().Lookup("to").Changed {
		if to < 1 || to > len(sheets) {
			return 0, InvalidArgsError(fmt.Sprintf("--to must be between 1 and %d", len(sheets)))
		}
		return to, nil
	}
	targetSelector := before
	insertAfter := false
	if after != "" {
		targetSelector = after
		insertAfter = true
	}
	target, err := selectXLSXSheet(sheets, targetSelector)
	if err != nil {
		return 0, err
	}
	if target.RelationshipID == moving.RelationshipID {
		return moving.Number, nil
	}
	order := make([]model.SheetRef, 0, len(sheets)-1)
	for _, sheet := range sheets {
		if sheet.RelationshipID != moving.RelationshipID {
			order = append(order, sheet)
		}
	}
	for index, sheet := range order {
		if sheet.RelationshipID != target.RelationshipID {
			continue
		}
		if insertAfter {
			return index + 2, nil
		}
		return index + 1, nil
	}
	return 0, InvalidArgsError(fmt.Sprintf("sheet not found: %s", targetSelector))
}

func init() {
	xlsxSheetsMoveCmd.Flags().StringVar(&xlsxSheetsMoveSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxSheetsMoveCmd.Flags().IntVar(&xlsxSheetsMoveTo, "to", 0, "final 1-based workbook sheet position")
	xlsxSheetsMoveCmd.Flags().StringVar(&xlsxSheetsMoveBefore, "before", "", "move before sheet number (1-based) or exact sheet name")
	xlsxSheetsMoveCmd.Flags().StringVar(&xlsxSheetsMoveAfter, "after", "", "move after sheet number (1-based) or exact sheet name")
	AddMutationFlags(xlsxSheetsMoveCmd)
	xlsxSheetsCmd.AddCommand(xlsxSheetsMoveCmd)
}
