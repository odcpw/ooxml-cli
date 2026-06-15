package cli

import (
	"fmt"
	"os"
	"sort"
	"strings"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

// DOCXStylesListResult is the JSON shape for `docx styles list`.
type DOCXStylesListResult struct {
	File            string            `json:"file"`
	DocumentPartURI string            `json:"documentPartUri"`
	StylesPartURI   *string           `json:"stylesPartUri"`
	Count           int               `json:"count"`
	Styles          []model.StyleInfo `json:"styles"`
}

var docxStylesListType string

var docxStylesListCmd = &cobra.Command{
	Use:   "list <file>",
	Short: "List all paragraph, character, table, and numbering styles",
	Long:  "List style definitions from word/styles.xml, optionally filtered by type.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		styleType, err := normalizeDOCXStyleType(docxStylesListType)
		if err != nil {
			return err
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

		result := &DOCXStylesListResult{
			File:            filePath,
			DocumentPartURI: documentURI,
			Styles:          make([]model.StyleInfo, 0),
		}
		if stylesURI != "" {
			uri := stylesURI
			result.StylesPartURI = &uri
			styles, err := docxinspect.ParseStyles(pkg, stylesURI)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to parse styles: %v", err)
			}
			for _, style := range styles {
				if styleType != "" && style.Type != styleType {
					continue
				}
				result.Styles = append(result.Styles, style)
			}
		}
		result.Count = len(result.Styles)

		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXTablesJSON(cmd, result, "styles list")
		}
		return outputDOCXStylesListText(cmd, result)
	},
}

func normalizeDOCXStyleType(value string) (string, error) {
	if value == "" {
		return "", nil
	}
	normalized := strings.ToLower(strings.TrimSpace(value))
	switch normalized {
	case "paragraph", "character", "table", "numbering":
		return normalized, nil
	default:
		return "", InvalidArgsError("--type must be one of paragraph, character, table, numbering")
	}
}

func outputDOCXStylesListText(cmd *cobra.Command, result *DOCXStylesListResult) error {
	var builder strings.Builder
	if result.StylesPartURI == nil {
		builder.WriteString("No styles part found\n")
		return writeCLIOutput(cmd, []byte(strings.TrimRight(builder.String(), "\n")))
	}

	byType := map[string]int{}
	custom := 0
	for _, style := range result.Styles {
		byType[style.Type]++
		if !style.Builtin {
			custom++
		}
	}
	builtin := result.Count - custom

	builder.WriteString(fmt.Sprintf("%d styles (%d builtin, %d custom)\n", result.Count, builtin, custom))
	types := make([]string, 0, len(byType))
	for t := range byType {
		types = append(types, t)
	}
	sort.Strings(types)
	for _, t := range types {
		label := t
		if label == "" {
			label = "(untyped)"
		}
		builder.WriteString(fmt.Sprintf("  %s: %d\n", label, byType[t]))
	}
	for _, style := range result.Styles {
		flags := make([]string, 0, 2)
		if style.Default {
			flags = append(flags, "default")
		}
		if style.Builtin {
			flags = append(flags, "builtin")
		} else {
			flags = append(flags, "custom")
		}
		builder.WriteString(fmt.Sprintf("  - %s [%s] %q (%s)\n", style.StyleID, style.Type, style.Name, strings.Join(flags, ",")))
	}
	return writeCLIOutput(cmd, []byte(strings.TrimRight(builder.String(), "\n")))
}

func init() {
	docxStylesListCmd.Flags().StringVar(&docxStylesListType, "type", "", "filter by style type: paragraph, character, table, numbering")
	docxStylesCmd.AddCommand(docxStylesListCmd)
}
