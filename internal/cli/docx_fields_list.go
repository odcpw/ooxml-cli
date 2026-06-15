package cli

import (
	"fmt"
	"os"
	"strings"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

// DOCXFieldsListResult is the JSON shape of docx fields list.
type DOCXFieldsListResult struct {
	File            string              `json:"file"`
	DocumentPartURI string              `json:"documentPartUri"`
	Fields          []docxinspect.Field `json:"fields"`
}

func newDOCXFieldsListCmd() *cobra.Command {
	var typeFilter string
	cmd := &cobra.Command{
		Use:   "list <file>",
		Short: "List all simple/complex fields in document body + headers/footers",
		Long:  "List each field (instruction, cached result, type, and location). Cached results are a stale cache until Word recalculates fields.",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			filePath := args[0]
			if _, err := os.Stat(filePath); err != nil {
				return FileNotFoundError(filePath)
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
			listing, err := docxinspect.ListFields(pkg, documentURI)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to list fields: %v", err)
			}

			fields := listing.Fields
			if cmd.Flags().Lookup("type").Changed && typeFilter != "" {
				// Match on the leading field keyword so a filter of PAGE matches fields
				// carrying switches like "PAGE \* MERGEFORMAT".
				want := strings.ToUpper(typeFilter)
				filtered := make([]docxinspect.Field, 0, len(fields))
				for _, f := range fields {
					if docxmutate.FieldCodeBase(f.Instruction) == want {
						filtered = append(filtered, f)
					}
				}
				fields = filtered
			}

			result := &DOCXFieldsListResult{
				File:            filePath,
				DocumentPartURI: listing.DocumentPartURI,
				Fields:          fields,
			}
			if GetGlobalConfig(cmd).Format == "json" {
				return outputDOCXFieldJSON(cmd, result, "fields list")
			}
			return outputDOCXFieldsListText(cmd, result)
		},
	}
	cmd.Flags().StringVar(&typeFilter, "type", "", "show only fields whose leading instruction keyword matches (e.g. PAGE matches \"PAGE \\* MERGEFORMAT\")")
	return cmd
}

func outputDOCXFieldsListText(cmd *cobra.Command, result *DOCXFieldsListResult) error {
	if len(result.Fields) == 0 {
		return writeCLIOutput(cmd, []byte("no fields"))
	}
	var b strings.Builder
	for i, f := range result.Fields {
		if i > 0 {
			b.WriteString("\n")
		}
		b.WriteString(fmt.Sprintf("field %d [%s] %s = %q at %s", f.Index, f.FieldType, f.Instruction, f.CachedResult, f.Location))
	}
	return writeCLIOutput(cmd, []byte(b.String()))
}

func init() {
	docxFieldsCmd.AddCommand(newDOCXFieldsListCmd())
}
