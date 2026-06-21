package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	xlsxmutate "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

type XLSXSheetsAddResult struct {
	File           string                         `json:"file"`
	Number         int                            `json:"number"`
	Name           string                         `json:"name"`
	SheetID        string                         `json:"sheetId"`
	RelationshipID string                         `json:"relationshipId"`
	PartURI        string                         `json:"partUri"`
	Output         string                         `json:"output,omitempty"`
	DryRun         bool                           `json:"dryRun"`
	Destination    *XLSXSheetsMutationDestination `json:"destination,omitempty"`
	XLSXSheetsMutationReadbackCommands
}

var (
	xlsxSheetsAddName  string
	xlsxSheetsAddAfter string
)

var xlsxSheetsAddCmd = &cobra.Command{
	Use:   "add <file>",
	Short: "Add a worksheet to an XLSX workbook",
	Long:  "Add an empty worksheet and wire workbook.xml, workbook relationships, and content types.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if xlsxSheetsAddName == "" {
			return InvalidArgsError("--name is required")
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		wantReadback := GetGlobalConfig(cmd).Format == "json"
		result, err := performXLSXSheetsAdd(filePath, xlsxSheetsAddName, xlsxSheetsAddAfter, mutOpts, wantReadback)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXSheetsAddJSON(cmd, result)
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("added sheet %d %q", result.Number, result.Name)))
	},
}

func performXLSXSheetsAdd(filePath, name, after string, mutOpts *MutationOptions, wantReadback bool) (*XLSXSheetsAddResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}

	var result *XLSXSheetsAddResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		afterPosition := 0
		if after != "" {
			selected, err := selectXLSXSheet(workbook.Sheets, after)
			if err != nil {
				return err
			}
			afterPosition = selected.Position
		}
		addResult, err := xlsxmutate.AddSheet(&xlsxmutate.AddSheetRequest{
			Package:        pkg,
			WorkbookURI:    workbook.PartURI,
			ExistingSheets: workbook.Sheets,
			Name:           name,
			AfterPosition:  afterPosition,
		})
		if err != nil {
			return mapXLSXSheetMutationError(err)
		}
		outputPath := mutationOutputPathForResult(filePath, mutOpts)
		var destination *XLSXSheetsMutationDestination
		if wantReadback {
			destination, err = collectXLSXSheetsMutationDestination(pkg, outputPath, addResult.RelationshipID, addResult.PartURI)
			if err != nil {
				return err
			}
		}
		result = &XLSXSheetsAddResult{
			File:           filePath,
			Number:         addResult.Number,
			Name:           addResult.Name,
			SheetID:        addResult.SheetID,
			RelationshipID: addResult.RelationshipID,
			PartURI:        addResult.PartURI,
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

func mapXLSXSheetMutationError(err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	msg := err.Error()
	switch {
	case strings.Contains(msg, "sheet name"),
		strings.Contains(msg, "already exists"),
		strings.Contains(msg, "out of range"),
		strings.Contains(msg, "last sheet"),
		strings.Contains(msg, "last visible sheet"),
		strings.Contains(msg, "not a worksheet"),
		strings.Contains(msg, "workbook has no sheets"):
		return InvalidArgsError(msg)
	default:
		return NewCLIErrorf(ExitUnexpected, "failed to mutate sheets: %v", err)
	}
}

func outputXLSXSheetsJSON(cmd *cobra.Command, value any, label string) error {
	config := GetGlobalConfig(cmd)
	var (
		data []byte
		err  error
	)
	if config.Pretty {
		data, err = json.MarshalIndent(value, "", "  ")
	} else {
		data, err = json.Marshal(value)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal %s JSON: %v", label, err)
	}
	return writeXLSXOutput(cmd, data)
}

func outputXLSXSheetsAddJSON(cmd *cobra.Command, result *XLSXSheetsAddResult) error {
	return outputXLSXSheetsJSON(cmd, result, "sheets add")
}

func init() {
	xlsxSheetsAddCmd.Flags().StringVar(&xlsxSheetsAddName, "name", "", "new worksheet name")
	xlsxSheetsAddCmd.Flags().StringVar(&xlsxSheetsAddAfter, "after", "", "insert after sheet number or exact sheet name; omitted appends")
	AddMutationFlags(xlsxSheetsAddCmd)
	xlsxSheetsCmd.AddCommand(xlsxSheetsAddCmd)
}
