package cli

import (
	"fmt"
	"os"
	"strings"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

type DOCXTablesShowResult struct {
	File   string             `json:"file"`
	Tables []DOCXTableSummary `json:"tables"`
}

var (
	docxTablesShowTable   int
	docxTablesShowDetails bool
)

var docxTablesShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Show DOCX tables",
	Long:  "Show main-document tables by table index, block index, dimensions, and cell text.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if docxTablesShowTable < 0 {
			return InvalidArgsError("--table must be positive")
		}

		pkg, err := openPackageExpectType(filePath, opc.PackageTypeDOCX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}
		tables, err := collectDOCXTables(pkg, documentURI, docxTablesShowTable, docxTablesShowDetails)
		if err != nil {
			return err
		}
		for i := range tables {
			tables[i].File = filePath
		}
		result := &DOCXTablesShowResult{File: filePath, Tables: tables}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXTablesJSON(cmd, result, "tables show")
		}
		return outputDOCXTablesShowText(cmd, result)
	},
}

func outputDOCXTablesShowText(cmd *cobra.Command, result *DOCXTablesShowResult) error {
	var builder strings.Builder
	for tableIndex, table := range result.Tables {
		if tableIndex > 0 {
			builder.WriteByte('\n')
		}
		merged := ""
		if table.Merged {
			merged = " merged"
		}
		builder.WriteString(fmt.Sprintf("Table %d (block %d %s): %dx%d%s\n", table.Table, table.Block, table.ContentHash, table.Rows, table.Cols, merged))
		for rowIndex, row := range table.Cells {
			builder.WriteString(fmt.Sprintf("  R%d: %s\n", rowIndex+1, strings.Join(row, "\t")))
		}
	}
	return writeCLIOutput(cmd, []byte(strings.TrimRight(builder.String(), "\n")))
}

func init() {
	docxTablesShowCmd.Flags().IntVar(&docxTablesShowTable, "table", 0, "1-based table number; omitted shows all tables")
	docxTablesShowCmd.Flags().BoolVar(&docxTablesShowDetails, "details", false, "include detailed table object in JSON output")
	docxTablesCmd.AddCommand(docxTablesShowCmd)
}
