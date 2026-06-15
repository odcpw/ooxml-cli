package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/spf13/cobra"
)

var pptxFieldsInspectCmd = &cobra.Command{
	Use:   "inspect <file>",
	Short: "Inspect header/footer/slide-number/date field settings",
	Long: `Report the presentation-wide header/footer field configuration.

For each slide master it reports the p:hf visibility defaults (slide number,
footer, date; a missing attribute defaults to visible). For each slide it reports
which footer/date/slide-number placeholders exist and their practical values
(footer text and the date field type).

Examples:
  ooxml pptx fields inspect deck.pptx
  ooxml --json pptx fields inspect deck.pptx`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		pkg, err := openPackageExpectType(filePath, opc.PackageTypePPTX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		report, err := inspect.ReadFields(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to inspect fields: %v", err)
		}

		if GetGlobalConfig(cmd).Format == "json" {
			return writeGlobalJSON(cmd, report)
		}
		return writeCLIOutput(cmd, []byte(formatPPTXFieldsText(report)))
	},
}

func formatPPTXFieldsText(report *inspect.FieldsReport) string {
	var b strings.Builder
	b.WriteString("Masters:\n")
	if len(report.Masters) == 0 {
		b.WriteString("  (none)\n")
	}
	for _, m := range report.Masters {
		source := "default"
		if m.HasHeaderFooter {
			source = "p:hf"
		}
		fmt.Fprintf(&b, "  %s (%s): slideNumber=%s footer=%s date=%s\n",
			m.PartURI, source,
			onOff(m.ShowSlideNumber), onOff(m.ShowFooter), onOff(m.ShowDate))
	}
	b.WriteString("Slides:\n")
	if len(report.Slides) == 0 {
		b.WriteString("  (none)\n")
	}
	for _, s := range report.Slides {
		fmt.Fprintf(&b, "  Slide %d:\n", s.Slide)
		fmt.Fprintf(&b, "    footer: %s\n", describeFooter(s.FooterPlaceholder))
		fmt.Fprintf(&b, "    date: %s\n", describeField(s.DatePlaceholder))
		fmt.Fprintf(&b, "    slideNumber: %s\n", describeField(s.SlideNumberPlaceholder))
	}
	return strings.TrimRight(b.String(), "\n")
}

func describeFooter(info *inspect.FieldPlaceholderInfo) string {
	if info == nil {
		return "no placeholder"
	}
	if info.Text == "" {
		return "placeholder (empty)"
	}
	return fmt.Sprintf("%q", info.Text)
}

func describeField(info *inspect.FieldPlaceholderInfo) string {
	if info == nil {
		return "no placeholder"
	}
	if info.FieldType != "" {
		return fmt.Sprintf("placeholder (fld type=%s)", info.FieldType)
	}
	return "placeholder"
}

func onOff(v bool) string {
	if v {
		return "shown"
	}
	return "hidden"
}

func init() {
	pptxFieldsCmd.AddCommand(pptxFieldsInspectCmd)
}
