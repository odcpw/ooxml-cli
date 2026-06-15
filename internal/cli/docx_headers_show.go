package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

type DOCXHeadersShowResult struct {
	File            string                              `json:"file"`
	Kind            string                              `json:"kind"`
	PartURI         string                              `json:"partUri"`
	ID              string                              `json:"id"`
	Type            string                              `json:"type"`
	Section         int                                 `json:"section"`
	PrimarySelector string                              `json:"primarySelector,omitempty"`
	Selectors       []string                            `json:"selectors,omitempty"`
	Paragraphs      []docxinspect.HeaderFooterParagraph `json:"paragraphs"`
}

func newDOCXHeadersShowCmd(kind string) *cobra.Command {
	var (
		id       string
		refType  string
		section  int
		selector string
	)
	cmd := &cobra.Command{
		Use:   "show <file>",
		Short: fmt.Sprintf("Show %s content by type, section, or relationship id", kind),
		Long:  fmt.Sprintf("Resolve a %s reference and print its paragraph text.", kind),
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			filePath := args[0]
			if _, err := os.Stat(filePath); err != nil {
				return FileNotFoundError(filePath)
			}
			selectorGiven := cmd.Flags().Changed("selector")
			if selectorGiven && (cmd.Flags().Changed("id") || cmd.Flags().Changed("type") || cmd.Flags().Changed("section")) {
				return InvalidArgsError("cannot specify --selector with --id, --type, or --section")
			}
			normType, err := normalizeHeaderFooterType(refType)
			if err != nil {
				return err
			}
			if section < 0 {
				return InvalidArgsError("--section must be >= 0 (0 means the last section)")
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
			var ref *docxinspect.HeaderFooterRef
			if selectorGiven {
				parsed, err := parseDOCXHeaderFooterSelector(kind, selector)
				if err != nil {
					return err
				}
				ref, err = resolveDOCXHeaderFooterSelector(pkg, documentURI, kind, parsed)
				if err != nil {
					return docxHeaderFooterNotFoundError(pkg, documentURI, kind, parsed.Raw)
				}
			} else {
				ref, err = docxinspect.ResolveHeaderFooter(pkg, documentURI, kind, normType, id, section)
				if err != nil {
					return docxHeaderFooterNotFoundError(pkg, documentURI, kind, docxHeaderFooterRequestedSelector(pkg, documentURI, kind, normType, id, section))
				}
			}
			if ref.PartURI == "" {
				return NewCLIErrorf(ExitInvalidArgs, "%s reference %q does not resolve to a part", kind, ref.ID)
			}
			paragraphs, err := docxinspect.ReadHeaderFooterParagraphs(pkg, ref.PartURI)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "%v", err)
			}
			paragraphs = docxinspect.AnnotateHeaderFooterParagraphs(ref, paragraphs)
			result := &DOCXHeadersShowResult{
				File:            filePath,
				Kind:            ref.Kind,
				PartURI:         ref.PartURI,
				ID:              ref.ID,
				Type:            ref.Type,
				Section:         ref.Section,
				PrimarySelector: ref.PrimarySelector,
				Selectors:       ref.Selectors,
				Paragraphs:      paragraphs,
			}
			if GetGlobalConfig(cmd).Format == "json" {
				return outputDOCXHeadersShowJSON(cmd, result)
			}
			return outputDOCXHeadersShowText(cmd, result)
		},
	}
	cmd.Flags().StringVar(&id, "id", "", "relationship id to resolve directly (optional)")
	cmd.Flags().StringVar(&refType, "type", "default", "reference type: default, first, or even")
	cmd.Flags().IntVar(&section, "section", 0, "1-based section index (default: last section)")
	cmd.Flags().StringVar(&selector, "selector", "", "selector from headers/footers list, such as header:1:default, footer:1:default, or id:rId10")
	return cmd
}

func outputDOCXHeadersShowJSON(cmd *cobra.Command, result *DOCXHeadersShowResult) error {
	config := GetGlobalConfig(cmd)
	var (
		data []byte
		err  error
	)
	if config.Pretty {
		data, err = json.MarshalIndent(result, "", "  ")
	} else {
		data, err = json.Marshal(result)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal %s show JSON: %v", result.Kind, err)
	}
	return writeCLIOutput(cmd, data)
}

func outputDOCXHeadersShowText(cmd *cobra.Command, result *DOCXHeadersShowResult) error {
	var builder strings.Builder
	builder.WriteString(fmt.Sprintf("%s %s (%s)", result.Kind, filepath.Base(result.PartURI), result.Type))
	for _, p := range result.Paragraphs {
		builder.WriteString(fmt.Sprintf("\n  %d: %q", p.Index, p.Text))
	}
	return writeCLIOutput(cmd, []byte(builder.String()))
}

func init() {
	docxHeadersCmd.AddCommand(newDOCXHeadersShowCmd(docxinspect.KindHeader))
	docxFootersCmd.AddCommand(newDOCXHeadersShowCmd(docxinspect.KindFooter))
}
