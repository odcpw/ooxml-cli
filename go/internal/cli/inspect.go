package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/spf13/cobra"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmodel "github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxinspect "github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	xlsxmodel "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
)

// InspectResult is the JSON output structure for the inspect command
type InspectResult struct {
	Type    string          `json:"type"`
	File    string          `json:"file"`
	Summary *InspectSummary `json:"summary"`
	Error   string          `json:"error,omitempty"`
}

// XLSXInspectResult is the JSON output structure for XLSX inspect
type XLSXInspectResult struct {
	Type    string              `json:"type"`
	File    string              `json:"file"`
	Summary *XLSXInspectSummary `json:"summary"`
	Error   string              `json:"error,omitempty"`
}

// DOCXInspectResult is the JSON output structure for DOCX inspect
type DOCXInspectResult struct {
	Type    string              `json:"type"`
	File    string              `json:"file"`
	Summary *DOCXInspectSummary `json:"summary"`
	Error   string              `json:"error,omitempty"`
}

// XLSXInspectSummary contains workbook summary statistics
type XLSXInspectSummary struct {
	Sheets            int  `json:"sheets"`
	Worksheets        int  `json:"worksheets"`
	SharedStrings     bool `json:"sharedStrings"`
	SharedStringCount int  `json:"sharedStringCount,omitempty"`
	Styles            bool `json:"styles"`
	Themes            int  `json:"themes"`
	Tables            int  `json:"tables"`
	Pivots            int  `json:"pivots"`
	PivotCaches       int  `json:"pivotCaches"`
	Charts            int  `json:"charts"`
	MediaAssets       int  `json:"mediaAssets"`
	CustomXmlParts    int  `json:"customXmlParts"`
}

// DOCXInspectSummary contains document summary statistics
type DOCXInspectSummary struct {
	Paragraphs     int  `json:"paragraphs"`
	Tables         int  `json:"tables"`
	Hyperlinks     int  `json:"hyperlinks"`
	Headers        int  `json:"headers"`
	Footers        int  `json:"footers"`
	Footnotes      bool `json:"footnotes"`
	Endnotes       bool `json:"endnotes"`
	Comments       bool `json:"comments"`
	Sections       int  `json:"sections"`
	Styles         bool `json:"styles"`
	Numbering      bool `json:"numbering"`
	MediaAssets    int  `json:"mediaAssets"`
	CustomXmlParts int  `json:"customXmlParts"`
}

// InspectSummary contains the summary statistics
type InspectSummary struct {
	Slides         int            `json:"slides"`
	Masters        int            `json:"masters"`
	Layouts        int            `json:"layouts"`
	Themes         int            `json:"themes"`
	NotesMasters   int            `json:"notesMasters"`
	HandoutMasters int            `json:"handoutMasters"`
	MediaAssets    int            `json:"mediaAssets"`
	CustomXmlParts int            `json:"customXmlParts"`
	SlideSize      *SlideSizeJSON `json:"slideSize,omitempty"`
}

// SlideSizeJSON represents slide dimensions in JSON format
type SlideSizeJSON struct {
	CX   int64  `json:"cx"`
	CY   int64  `json:"cy"`
	Unit string `json:"unit"`
}

var inspectCmd = &cobra.Command{
	Use:   "inspect <file>",
	Short: "Inspect OOXML package structure",
	Long: `Inspect an OOXML package (PPTX, DOCX, or XLSX) and report its structure.

Outputs a type-specific summary, such as slide counts for PPTX files or sheet
counts for XLSX files.`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]

		// Check if file exists
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		// Open the package
		pkg, err := opc.Open(filePath)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to open package: %v", err)
		}
		defer pkg.Close()

		// Get global config
		config := GetGlobalConfig(cmd)

		pkgType := opc.DetectType(pkg)
		switch pkgType {
		case opc.PackageTypePPTX:
			summary, err := pptxinspect.SummarizeDeck(pkg)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to inspect deck: %v", err)
			}
			if config.Format == "json" {
				return outputInspectJSON(cmd, filePath, summary)
			}
			return outputInspectText(cmd, summary)
		case opc.PackageTypeXLSX:
			summary, err := xlsxinspect.SummarizeWorkbook(pkg)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to inspect workbook: %v", err)
			}
			if config.Format == "json" {
				return outputXLSXInspectJSON(cmd, filePath, summary)
			}
			return outputXLSXInspectText(cmd, summary)
		case opc.PackageTypeDOCX:
			summary, err := docxinspect.SummarizeDocument(pkg)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to inspect document: %v", err)
			}
			if config.Format == "json" {
				return outputDOCXInspectJSON(cmd, filePath, summary)
			}
			return outputDOCXInspectText(cmd, summary)
		default:
			return UnsupportedTypeError(pkgType.String())
		}
	},
}

// outputInspectJSON outputs the inspect result in JSON format
func outputInspectJSON(cmd *cobra.Command, filePath string, summary *pptxinspect.DeckSummary) error {
	config := GetGlobalConfig(cmd)

	// Build the result
	result := InspectResult{
		Type: summary.Type,
		File: filePath,
		Summary: &InspectSummary{
			Slides:         summary.SlideCount,
			Masters:        summary.MasterCount,
			Layouts:        summary.LayoutCount,
			Themes:         summary.ThemeCount,
			NotesMasters:   summary.NotesMasterCount,
			HandoutMasters: summary.HandoutMasterCount,
			MediaAssets:    summary.MediaCount,
			CustomXmlParts: summary.CustomXMLCount,
		},
	}

	// Add slide size if available
	if summary.SlideSizeEMU != nil {
		result.Summary.SlideSize = &SlideSizeJSON{
			CX:   summary.SlideSizeEMU.CX,
			CY:   summary.SlideSizeEMU.CY,
			Unit: "emu",
		}
	}

	// Marshal to JSON
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

	// Write to output
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

// outputInspectText outputs the inspect result in human-readable text format
func outputInspectText(cmd *cobra.Command, summary *pptxinspect.DeckSummary) error {
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

	// Format text output
	lines := []string{}
	lines = append(lines, fmt.Sprintf("Type: %s", summary.Type))
	lines = append(lines, fmt.Sprintf("Slides: %d", summary.SlideCount))
	lines = append(lines, fmt.Sprintf("Masters: %d", summary.MasterCount))
	lines = append(lines, fmt.Sprintf("Layouts: %d", summary.LayoutCount))
	lines = append(lines, fmt.Sprintf("Themes: %d", summary.ThemeCount))
	if summary.NotesMasterCount > 0 {
		lines = append(lines, fmt.Sprintf("Notes masters: %d", summary.NotesMasterCount))
	}
	if summary.HandoutMasterCount > 0 {
		lines = append(lines, fmt.Sprintf("Handout masters: %d", summary.HandoutMasterCount))
	}
	lines = append(lines, fmt.Sprintf("Media assets: %d", summary.MediaCount))
	lines = append(lines, fmt.Sprintf("Custom XML parts: %d", summary.CustomXMLCount))

	if summary.SlideSizeEMU != nil {
		lines = append(lines, fmt.Sprintf("Slide size: %d x %d EMU", summary.SlideSizeEMU.CX, summary.SlideSizeEMU.CY))
	}

	// Write output
	for _, line := range lines {
		fmt.Fprintf(outFile, "%s\n", line)
	}

	return nil
}

func outputXLSXInspectJSON(cmd *cobra.Command, filePath string, summary *xlsxmodel.WorkbookSummary) error {
	config := GetGlobalConfig(cmd)

	result := XLSXInspectResult{
		Type: summary.Type,
		File: filePath,
		Summary: &XLSXInspectSummary{
			Sheets:            summary.SheetCount,
			Worksheets:        summary.WorksheetCount,
			SharedStrings:     summary.SharedStrings,
			SharedStringCount: summary.SharedStringCount,
			Styles:            summary.Styles,
			Themes:            summary.Themes,
			Tables:            summary.Tables,
			Pivots:            summary.Pivots,
			PivotCaches:       summary.PivotCaches,
			Charts:            summary.Charts,
			MediaAssets:       summary.MediaAssets,
			CustomXmlParts:    summary.CustomXMLParts,
		},
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

func outputXLSXInspectText(cmd *cobra.Command, summary *xlsxmodel.WorkbookSummary) error {
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

	fmt.Fprintf(outFile, "Type: %s\n", summary.Type)
	fmt.Fprintf(outFile, "Sheets: %d\n", summary.SheetCount)
	fmt.Fprintf(outFile, "Worksheets: %d\n", summary.WorksheetCount)
	fmt.Fprintf(outFile, "Shared strings: %t\n", summary.SharedStrings)
	if summary.SharedStringCount > 0 {
		fmt.Fprintf(outFile, "Shared string count: %d\n", summary.SharedStringCount)
	}
	fmt.Fprintf(outFile, "Styles: %t\n", summary.Styles)
	fmt.Fprintf(outFile, "Themes: %d\n", summary.Themes)
	fmt.Fprintf(outFile, "Tables: %d\n", summary.Tables)
	fmt.Fprintf(outFile, "Pivots: %d\n", summary.Pivots)
	fmt.Fprintf(outFile, "Pivot caches: %d\n", summary.PivotCaches)
	fmt.Fprintf(outFile, "Charts: %d\n", summary.Charts)
	fmt.Fprintf(outFile, "Media assets: %d\n", summary.MediaAssets)
	fmt.Fprintf(outFile, "Custom XML parts: %d\n", summary.CustomXMLParts)
	return nil
}

func outputDOCXInspectJSON(cmd *cobra.Command, filePath string, summary *docxmodel.DocumentSummary) error {
	result := DOCXInspectResult{
		Type: summary.Type,
		File: filePath,
		Summary: &DOCXInspectSummary{
			Paragraphs:     summary.Paragraphs,
			Tables:         summary.Tables,
			Hyperlinks:     summary.Hyperlinks,
			Headers:        summary.Headers,
			Footers:        summary.Footers,
			Footnotes:      summary.Footnotes,
			Endnotes:       summary.Endnotes,
			Comments:       summary.Comments,
			Sections:       summary.Sections,
			Styles:         summary.Styles,
			Numbering:      summary.Numbering,
			MediaAssets:    summary.MediaAssets,
			CustomXmlParts: summary.CustomXMLParts,
		},
	}

	var data []byte
	var err error
	if GetGlobalConfig(cmd).Pretty {
		data, err = json.MarshalIndent(result, "", "  ")
	} else {
		data, err = json.Marshal(result)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal DOCX inspect JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputDOCXInspectText(cmd *cobra.Command, summary *docxmodel.DocumentSummary) error {
	var builder strings.Builder
	builder.WriteString(fmt.Sprintf("Type: %s\n", summary.Type))
	builder.WriteString(fmt.Sprintf("Paragraphs: %d\n", summary.Paragraphs))
	builder.WriteString(fmt.Sprintf("Tables: %d\n", summary.Tables))
	builder.WriteString(fmt.Sprintf("Hyperlinks: %d\n", summary.Hyperlinks))
	builder.WriteString(fmt.Sprintf("Headers: %d\n", summary.Headers))
	builder.WriteString(fmt.Sprintf("Footers: %d\n", summary.Footers))
	builder.WriteString(fmt.Sprintf("Footnotes: %t\n", summary.Footnotes))
	builder.WriteString(fmt.Sprintf("Endnotes: %t\n", summary.Endnotes))
	builder.WriteString(fmt.Sprintf("Comments: %t\n", summary.Comments))
	builder.WriteString(fmt.Sprintf("Sections: %d\n", summary.Sections))
	builder.WriteString(fmt.Sprintf("Styles: %t\n", summary.Styles))
	builder.WriteString(fmt.Sprintf("Numbering: %t\n", summary.Numbering))
	builder.WriteString(fmt.Sprintf("Media assets: %d\n", summary.MediaAssets))
	builder.WriteString(fmt.Sprintf("Custom XML parts: %d\n", summary.CustomXMLParts))
	return writeCLIOutput(cmd, []byte(builder.String()))
}

// init registers the inspect command with the root command
func init() {
	GetRootCmd().AddCommand(inspectCmd)
}
