package cli

import (
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

type PPTXTablesShowResult struct {
	File   string             `json:"file"`
	Slide  int                `json:"slide"`
	Tables []PPTXTableSummary `json:"tables"`
}

var (
	pptxTablesShowSlide   int
	pptxTablesShowTableID int
	pptxTablesShowTarget  string
	pptxTablesShowDetails bool
)

var pptxTablesShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Show PPTX tables on a slide",
	Long:  "Show table graphic frames and cell text for one slide.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if err := parsePositiveIntFlag(pptxTablesShowSlide, "slide"); err != nil {
			return err
		}

		result, err := performPPTXTablesShow(filePath, pptxTablesShowSlide, pptxTablesShowTableID, pptxTablesShowTarget, pptxTablesShowDetails)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputPPTXTablesJSON(cmd, result, "tables show")
		}
		return writePPTXTablesText(cmd, result)
	},
}

func performPPTXTablesShow(filePath string, slideNumber, tableID int, target string, includeDetails bool) (*PPTXTablesShowResult, error) {
	session, err := openPackageExpectType(filePath, opc.PackageTypePPTX)
	if err != nil {
		return nil, err
	}
	defer session.Close()

	slideRef, err := resolvePPTXSlideRef(session, slideNumber)
	if err != nil {
		return nil, err
	}
	resolvedTableID, err := resolvePPTXTableTarget(session, slideNumber, tableID, target)
	if err != nil {
		return nil, err
	}
	tables, err := collectPPTXTables(session, slideRef, resolvedTableID, includeDetails)
	if err != nil {
		return nil, err
	}
	if tables == nil {
		tables = []PPTXTableSummary{}
	}
	return &PPTXTablesShowResult{
		File:   filePath,
		Slide:  slideNumber,
		Tables: tables,
	}, nil
}

func init() {
	pptxTablesShowCmd.Flags().IntVar(&pptxTablesShowSlide, "slide", 0, "1-based slide number")
	pptxTablesShowCmd.Flags().IntVar(&pptxTablesShowTableID, "table-id", 0, "optional table shape ID to show")
	pptxTablesShowCmd.Flags().StringVar(&pptxTablesShowTarget, "target", "", "optional table selector to show (e.g., table:1, shape:2, ~Table 1)")
	pptxTablesShowCmd.Flags().BoolVar(&pptxTablesShowDetails, "details", false, "include enriched row, column, cell, fill, border, and style details")
	tablesCmd.AddCommand(pptxTablesShowCmd)
}
