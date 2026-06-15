package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/spf13/cobra"
)

var (
	pptxFieldsSetFooter          string
	pptxFieldsSetShowFooter      bool
	pptxFieldsSetShowSlideNumber bool
	pptxFieldsSetShowDate        bool
	pptxFieldsSetDateFormat      string
)

// PPTXFieldsSetResult is the JSON readback contract for the fields set command.
type PPTXFieldsSetResult struct {
	File   string `json:"file"`
	Output string `json:"output,omitempty"`
	DryRun bool   `json:"dryRun"`
	PPTXBridgeReadbackCommands
	mutate.SetFieldsResult
}

var pptxFieldsSetCmd = &cobra.Command{
	Use:   "set <file>",
	Short: "Set header/footer visibility, footer text, and date format",
	Long: `Set presentation header/footer/slide-number/date field values.

Visibility toggles (--show-slide-number, --show-footer, --show-date) are written
to each slide master's p:hf element (created when absent); these are the
presentation-wide visibility settings PowerPoint's Header & Footer dialog stores.

--footer sets the literal footer text on every slide that already carries a footer
placeholder. --date-format sets the date field type on every slide that already
carries a date placeholder (auto|datetime|date-only). Slides without the relevant
placeholder are reported but do not cause a failure.

At least one of --footer, --show-slide-number, --show-footer, --show-date, or
--date-format is required.

Examples:
  ooxml pptx fields set deck.pptx --footer "Confidential" --out out.pptx
  ooxml pptx fields set deck.pptx --show-slide-number=true --show-date=false --in-place
  ooxml pptx fields set deck.pptx --date-format date-only --dry-run`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		req := &mutate.SetFieldsRequest{}
		changed := func(name string) bool { return cmd.Flags().Changed(name) }
		any := false
		if changed("footer") {
			v := pptxFieldsSetFooter
			req.FooterText = &v
			any = true
		}
		if changed("show-footer") {
			v := pptxFieldsSetShowFooter
			req.ShowFooter = &v
			any = true
		}
		if changed("show-slide-number") {
			v := pptxFieldsSetShowSlideNumber
			req.ShowSlideNumber = &v
			any = true
		}
		if changed("show-date") {
			v := pptxFieldsSetShowDate
			req.ShowDate = &v
			any = true
		}
		if changed("date-format") {
			req.DateFormat = pptxFieldsSetDateFormat
			any = true
		}
		if !any {
			return InvalidArgsError("no field flags provided; specify at least one of --footer/--show-footer/--show-slide-number/--show-date/--date-format")
		}
		if req.DateFormat != "" && !isValidDateFormat(req.DateFormat) {
			return InvalidArgsError(fmt.Sprintf("invalid --date-format %q (expected one of: %s)", req.DateFormat, strings.Join(mutate.ValidDateFormats(), ", ")))
		}

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performPPTXFieldsSet(filePath, req, mutOpts)
		if err != nil {
			return err
		}

		data, err := marshalWithConfig(GetGlobalConfig(cmd), result)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to marshal fields set JSON: %v", err)
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeCLIOutput(cmd, data)
		}
		return writeCLIOutput(cmd, []byte(formatPPTXFieldsSetText(result)))
	},
}

func isValidDateFormat(value string) bool {
	for _, v := range mutate.ValidDateFormats() {
		if v == value {
			return true
		}
	}
	return false
}

func performPPTXFieldsSet(filePath string, req *mutate.SetFieldsRequest, mutOpts *MutationOptions) (*PPTXFieldsSetResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
	if err != nil {
		return nil, err
	}
	var result *PPTXFieldsSetResult
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		req.Package = pkg
		updated, err := mutate.SetFields(req)
		if err != nil {
			return mapPPTXFieldsMutationError(err)
		}
		result = &PPTXFieldsSetResult{
			File:            filePath,
			Output:          destinationFile,
			DryRun:          mutOpts.DryRun,
			SetFieldsResult: *updated,
		}
		result.PPTXBridgeReadbackCommands = pptxFieldsMutationReadbackCommands(destinationFile)
		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to set fields")
	}
	return result, nil
}

// mapPPTXFieldsMutationError maps mutate-layer field errors to CLI errors,
// treating validation/argument errors as invalid-args.
func mapPPTXFieldsMutationError(err error) error {
	if err == nil {
		return nil
	}
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	msg := err.Error()
	switch {
	case strings.Contains(msg, "invalid date format"),
		strings.Contains(msg, "no field changes requested"):
		return InvalidArgsError(msg)
	default:
		return NewCLIErrorf(ExitUnexpected, "%v", err)
	}
}

// pptxFieldsMutationReadbackCommands builds the inspect/validate/render follow-up
// commands for a fields mutation.
func pptxFieldsMutationReadbackCommands(destinationFile string) PPTXBridgeReadbackCommands {
	if destinationFile == "" {
		placeholder := outputPlaceholder()
		return PPTXBridgeReadbackCommands{
			ReadbackCommandTemplate: pptxFieldsInspectReadbackCommand(placeholder),
			ValidateCommandTemplate: pptxValidateCommand(placeholder),
			RenderCommandTemplate:   pptxRenderCommand(placeholder),
		}
	}
	return PPTXBridgeReadbackCommands{
		ReadbackCommand: pptxFieldsInspectReadbackCommand(destinationFile),
		ValidateCommand: pptxValidateCommand(destinationFile),
		RenderCommand:   pptxRenderCommand(destinationFile),
	}
}

func pptxFieldsInspectReadbackCommand(filePath string) string {
	return fmt.Sprintf("ooxml --json pptx fields inspect %s", pptxXLSXCommandArg(filePath))
}

func formatPPTXFieldsSetText(result *PPTXFieldsSetResult) string {
	var b strings.Builder
	b.WriteString("Updated header/footer fields")
	if result.CreatedHeaderFooter {
		b.WriteString(" (created p:hf on master(s))")
	}
	b.WriteString("\n")
	if len(result.MastersUpdated) > 0 {
		fmt.Fprintf(&b, "Masters updated: %d\n", len(result.MastersUpdated))
	}
	if result.FooterText != nil {
		fmt.Fprintf(&b, "Footer placeholders updated: %d\n", result.FooterPlaceholdersUpdated)
	}
	if result.DateFormat != "" {
		fmt.Fprintf(&b, "Date placeholders updated: %d\n", result.DatePlaceholdersUpdated)
		if len(result.SlidesWithDatePlaceholderButNoField) > 0 {
			fmt.Fprintf(&b, "Slides with date placeholder but no field (--date-format had no effect): %v\n", result.SlidesWithDatePlaceholderButNoField)
		}
	}
	if result.Output != "" {
		fmt.Fprintf(&b, "Output: %s", result.Output)
	} else {
		b.WriteString("Dry run: no output written")
	}
	return strings.TrimRight(b.String(), "\n")
}

func init() {
	pptxFieldsSetCmd.Flags().StringVar(&pptxFieldsSetFooter, "footer", "", "footer text to set on slide footer placeholders")
	pptxFieldsSetCmd.Flags().BoolVar(&pptxFieldsSetShowFooter, "show-footer", true, "show/hide the footer (writes master p:hf @ftr)")
	pptxFieldsSetCmd.Flags().BoolVar(&pptxFieldsSetShowSlideNumber, "show-slide-number", true, "show/hide the slide number (writes master p:hf @sldNum)")
	pptxFieldsSetCmd.Flags().BoolVar(&pptxFieldsSetShowDate, "show-date", true, "show/hide the date (writes master p:hf @dt)")
	pptxFieldsSetCmd.Flags().StringVar(&pptxFieldsSetDateFormat, "date-format", "", "date field type to set on slide date placeholders (auto|datetime|date-only)")

	AddMutationFlags(pptxFieldsSetCmd)
	pptxFieldsCmd.AddCommand(pptxFieldsSetCmd)
}
