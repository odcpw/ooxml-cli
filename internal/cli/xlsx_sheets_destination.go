package cli

import (
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
)

type XLSXSheetsMutationDestination struct {
	File       string           `json:"file,omitempty"`
	Sheet      *model.SheetRef  `json:"sheet,omitempty"`
	Sheets     []model.SheetRef `json:"sheets"`
	SheetCount int              `json:"sheetCount"`
}

func collectXLSXSheetsMutationDestination(pkg opc.PackageSession, outputPath, affectedRelationshipID, affectedPartURI string) (*XLSXSheetsMutationDestination, error) {
	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to parse workbook readback: %v", err)
	}

	sheets := make([]model.SheetRef, len(workbook.Sheets))
	copy(sheets, workbook.Sheets)
	destination := &XLSXSheetsMutationDestination{
		File:       outputPath,
		Sheets:     sheets,
		SheetCount: len(sheets),
	}
	for idx := range sheets {
		sheet := sheets[idx]
		if affectedRelationshipID != "" && sheet.RelationshipID == affectedRelationshipID {
			destination.Sheet = &sheets[idx]
			break
		}
		if affectedPartURI != "" && sheet.PartURI == affectedPartURI {
			destination.Sheet = &sheets[idx]
			break
		}
	}
	return destination, nil
}
