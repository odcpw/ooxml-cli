package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/spf13/cobra"
)

type XLSXSheetsListResult struct {
	File            string              `json:"file"`
	ValidateCommand string              `json:"validateCommand,omitempty"`
	Sheets          []XLSXSheetListItem `json:"sheets"`
}

var xlsxSheetsListCmd = &cobra.Command{
	Use:   "list <file>",
	Short: "List sheets in a workbook",
	Long:  "List all sheets in an XLSX workbook with their workbook order and worksheet part URI.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		pkg, err := openPackageExpectType(filePath, opc.PackageTypeXLSX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		sheets, err := xlsxinspect.ListSheets(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to list sheets: %v", err)
		}

		config := GetGlobalConfig(cmd)
		if config.Format == "json" {
			return outputXLSXSheetsListJSON(cmd, filePath, sheets)
		}
		return outputXLSXSheetsListText(cmd, sheets)
	},
}

func outputXLSXSheetsListJSON(cmd *cobra.Command, filePath string, sheets []model.SheetRef) error {
	config := GetGlobalConfig(cmd)
	counts := xlsxSheetIDCounts(sheets)
	items := make([]XLSXSheetListItem, 0, len(sheets))
	for _, sheet := range sheets {
		items = append(items, xlsxSheetListItemWithCounts(filePath, sheet, counts))
	}
	result := XLSXSheetsListResult{
		File:            filePath,
		ValidateCommand: xlsxValidateCommand(filePath),
		Sheets:          items,
	}

	var jsonData []byte
	var err error
	if config.Pretty {
		jsonData, err = json.MarshalIndent(result, "", "  ")
	} else {
		jsonData, err = json.Marshal(result)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal JSON: %v", err)
	}

	var outFile io.Writer
	if config.Output != "" {
		file, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer file.Close()
		outFile = file
	} else {
		outFile = cmd.OutOrStdout()
	}

	fmt.Fprintf(outFile, "%s\n", string(jsonData))
	return nil
}

func outputXLSXSheetsListText(cmd *cobra.Command, sheets []model.SheetRef) error {
	config := GetGlobalConfig(cmd)

	var outFile io.Writer
	if config.Output != "" {
		file, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer file.Close()
		outFile = file
	} else {
		outFile = cmd.OutOrStdout()
	}

	fmt.Fprintf(outFile, "%-4s %-24s %-10s %-10s %s\n", "[N]", "Name", "sheetId", "state", "PartURI")
	fmt.Fprintf(outFile, "%s\n", strings.Repeat("-", 90))
	for _, sheet := range sheets {
		state := sheet.State
		if state == "" {
			state = "visible"
		}
		fmt.Fprintf(outFile, "[%-2d] %-24s %-10s %-10s %s\n",
			sheet.Number,
			truncateStr(sheet.Name, 24),
			sheet.SheetID,
			state,
			sheet.PartURI,
		)
	}
	return nil
}

func init() {
	xlsxSheetsCmd.AddCommand(xlsxSheetsListCmd)
}
