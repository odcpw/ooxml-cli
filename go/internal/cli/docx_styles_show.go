package cli

import (
	"fmt"
	"os"
	"strings"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

// DOCXStylesShowResult is the JSON shape for `docx styles show`.
type DOCXStylesShowResult struct {
	File            string           `json:"file"`
	DocumentPartURI string           `json:"documentPartUri"`
	StylesPartURI   *string          `json:"stylesPartUri"`
	StyleID         string           `json:"styleId"`
	Found           bool             `json:"found"`
	Style           *model.StyleInfo `json:"style"`
}

var docxStylesShowStyle string

var docxStylesShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Show detailed info for a single style by styleId",
	Long:  "Show the definition of a single style identified by its styleId.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if strings.TrimSpace(docxStylesShowStyle) == "" {
			return InvalidArgsError("--style is required")
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
		stylesURI, err := docxinspect.FindStylesPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to resolve styles part: %v", err)
		}

		result := &DOCXStylesShowResult{
			File:            filePath,
			DocumentPartURI: documentURI,
			StyleID:         docxStylesShowStyle,
		}
		if stylesURI != "" {
			uri := stylesURI
			result.StylesPartURI = &uri
			styles, err := docxinspect.ParseStyles(pkg, stylesURI)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to parse styles: %v", err)
			}
			if style, ok := docxinspect.FindStyle(styles, docxStylesShowStyle); ok {
				found := style
				result.Found = true
				result.Style = &found
			}
		}

		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXTablesJSON(cmd, result, "styles show")
		}
		return outputDOCXStylesShowText(cmd, result)
	},
}

func outputDOCXStylesShowText(cmd *cobra.Command, result *DOCXStylesShowResult) error {
	var builder strings.Builder
	if !result.Found || result.Style == nil {
		builder.WriteString(fmt.Sprintf("Style %q not found\n", result.StyleID))
		return writeCLIOutput(cmd, []byte(strings.TrimRight(builder.String(), "\n")))
	}
	style := result.Style
	builder.WriteString(fmt.Sprintf("Style: %s\n", style.StyleID))
	builder.WriteString(fmt.Sprintf("  Name: %s\n", style.Name))
	builder.WriteString(fmt.Sprintf("  Type: %s\n", style.Type))
	origin := "builtin"
	if !style.Builtin {
		origin = "custom"
	}
	builder.WriteString(fmt.Sprintf("  Origin: %s\n", origin))
	builder.WriteString(fmt.Sprintf("  Default: %t\n", style.Default))
	if style.BasedOn != "" {
		builder.WriteString(fmt.Sprintf("  BasedOn: %s\n", style.BasedOn))
	}
	if style.Next != "" {
		builder.WriteString(fmt.Sprintf("  Next: %s\n", style.Next))
	}
	return writeCLIOutput(cmd, []byte(strings.TrimRight(builder.String(), "\n")))
}

func init() {
	docxStylesShowCmd.Flags().StringVar(&docxStylesShowStyle, "style", "", "styleId to show (required)")
	docxStylesCmd.AddCommand(docxStylesShowCmd)
}
