package cli

import (
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/spf13/cobra"
)

var pptxNotesShowSlide int

var pptxNotesShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Display speaker notes for a slide",
	Long:  "Show the speaker notes (plain text and paragraphs) for one slide.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if pptxNotesShowSlide < 1 {
			return InvalidArgsError("--slide must be >= 1")
		}

		pkg, err := openPackageExpectType(filePath, opc.PackageTypePPTX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", err)
		}
		if pptxNotesShowSlide > len(graph.Slides) {
			return InvalidArgsError(fmt.Sprintf("slide %d not found (presentation has %d slides)", pptxNotesShowSlide, len(graph.Slides)))
		}
		report, err := extract.ExtractNotesForSlide(pkg, graph.Slides[pptxNotesShowSlide-1])
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to extract notes: %v", err)
		}

		if GetGlobalConfig(cmd).Format == "json" {
			return writeGlobalJSON(cmd, report)
		}
		return writeCLIOutput(cmd, []byte(formatPPTXNotesText(report)))
	},
}

func formatPPTXNotesText(report *extract.NotesReport) string {
	if report == nil || report.Notes == nil {
		return ""
	}
	header := fmt.Sprintf("Slide %d notes", report.Slide)
	if report.PartURI != "" {
		header += fmt.Sprintf(" (%s)", report.PartURI)
	} else {
		header += " (none)"
	}
	if report.Notes.PlainText == "" {
		return header + "\n(empty)"
	}
	return header + "\n" + report.Notes.PlainText
}

func init() {
	pptxNotesShowCmd.Flags().IntVar(&pptxNotesShowSlide, "slide", 0, "1-based slide number")
	pptxNotesShowCmd.MarkFlagRequired("slide")
	pptxNotesCmd.AddCommand(pptxNotesShowCmd)
}
