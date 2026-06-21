package cli

import (
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	xlsxtable "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/table"
)

type XLSXTableAppendDestination struct {
	File                 string                `json:"file,omitempty"`
	Table                string                `json:"table"`
	TablePrimarySelector string                `json:"tablePrimarySelector,omitempty"`
	TableSelectors       []string              `json:"tableSelectors,omitempty"`
	TablePartURI         string                `json:"tablePartUri,omitempty"`
	RelationshipID       string                `json:"relationshipId,omitempty"`
	Sheet                string                `json:"sheet"`
	SheetNumber          int                   `json:"sheetNumber"`
	SheetPrimarySelector string                `json:"sheetPrimarySelector,omitempty"`
	SheetSelectors       []string              `json:"sheetSelectors,omitempty"`
	PreviousRange        string                `json:"previousRange"`
	Range                string                `json:"range"`
	AppendRange          string                `json:"appendRange"`
	Rows                 int                   `json:"rows"`
	Cols                 int                   `json:"cols"`
	DataRows             int                   `json:"dataRows"`
	Columns              []string              `json:"columns,omitempty"`
	Appended             *XLSXRangeDestination `json:"appended,omitempty"`
}

func collectXLSXTableAppendDestination(pkg opc.PackageSession, workbook *model.Workbook, tableRef model.TableRef, previousRange, appendRange, destinationFile string) (*XLSXTableAppendDestination, error) {
	updated, err := xlsxtable.ReadPart(pkg, tableRef.PartURI)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to read updated table %s: %v", tableRef.PartURI, err)
	}
	updated.Number = tableRef.Number
	updated.Sheet = tableRef.Sheet
	updated.SheetNumber = tableRef.SheetNumber
	updated.SheetPartURI = tableRef.SheetPartURI
	updated.RelationshipID = tableRef.RelationshipID
	updated.PartURI = tableRef.PartURI
	*updated = model.WithTableSelectors(*updated)

	sheetRef := sheetRefForTableAppendDestination(workbook, tableRef)
	appendRangeRef, err := address.ParseRange(appendRange)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to read appended range %q: %v", appendRange, err)
	}
	appended, err := collectXLSXRangeDestination(pkg, workbook, sheetRef, appendRangeRef, destinationFile)
	if err != nil {
		return nil, err
	}

	return &XLSXTableAppendDestination{
		File:                 destinationFile,
		Table:                updated.DisplayName,
		TablePrimarySelector: updated.PrimarySelector,
		TableSelectors:       append([]string{}, updated.Selectors...),
		TablePartURI:         updated.PartURI,
		RelationshipID:       updated.RelationshipID,
		Sheet:                updated.Sheet,
		SheetNumber:          updated.SheetNumber,
		SheetPrimarySelector: sheetRef.PrimarySelector,
		SheetSelectors:       append([]string{}, sheetRef.Selectors...),
		PreviousRange:        previousRange,
		Range:                updated.Range,
		AppendRange:          appendRange,
		Rows:                 updated.Rows,
		Cols:                 updated.Cols,
		DataRows:             updated.DataRowCount,
		Columns:              tableColumnNames(*updated),
		Appended:             appended,
	}, nil
}

func sheetRefForTableAppendDestination(workbook *model.Workbook, tableRef model.TableRef) model.SheetRef {
	if workbook != nil {
		for _, sheet := range workbook.Sheets {
			if sheet.PartURI == tableRef.SheetPartURI {
				return model.WithSheetSelectors(sheet)
			}
		}
	}
	return model.WithSheetSelectors(model.SheetRef{
		Name:    tableRef.Sheet,
		Number:  tableRef.SheetNumber,
		PartURI: tableRef.SheetPartURI,
	})
}
