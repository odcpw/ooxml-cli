package cli

import (
	"errors"
	"fmt"
	"strconv"
	"strings"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/spf13/cobra"
)

var docxFieldsCmd = &cobra.Command{
	Use:     "fields",
	Aliases: []string{"field"},
	Short:   "Inspect and edit DOCX fields (PAGE, NUMPAGES, DATE, etc.)",
	Long: "Commands for listing fields in the body and headers/footers, inserting field codes, " +
		"and setting a field's cached result text. Cached results only refresh when Word " +
		"recalculates fields, so reported results are a cache, not a live value.",
	Args: cobra.NoArgs,
	RunE: showHelp,
}

// fieldLocation is a parsed --location / --selector value.
type fieldLocation struct {
	part       string // "body" or a header/footer label like "header1"
	blockIndex int
	fieldIndex int
	hasField   bool
}

// parseFieldLocation parses "body:1", "body:1:0", "header1:1", or "header1:1:0".
// The leading segment selects the part, the second is a 1-based block/paragraph
// index, and the optional third is a 0-based field index within that block.
func parseFieldLocation(value string) (*fieldLocation, error) {
	parts := strings.Split(strings.TrimSpace(value), ":")
	if len(parts) < 2 || len(parts) > 3 {
		return nil, InvalidArgsError(fmt.Sprintf("invalid location %q: expected part:block[:field] (e.g. body:1 or header1:1:0)", value))
	}
	loc := &fieldLocation{part: strings.TrimSpace(parts[0])}
	if loc.part == "" {
		return nil, InvalidArgsError(fmt.Sprintf("invalid location %q: part segment is empty", value))
	}
	block, err := strconv.Atoi(strings.TrimSpace(parts[1]))
	if err != nil || block < 1 {
		return nil, InvalidArgsError(fmt.Sprintf("invalid location %q: block index must be a positive integer", value))
	}
	loc.blockIndex = block
	if len(parts) == 3 {
		field, err := strconv.Atoi(strings.TrimSpace(parts[2]))
		if err != nil || field < 0 {
			return nil, InvalidArgsError(fmt.Sprintf("invalid location %q: field index must be a non-negative integer", value))
		}
		loc.fieldIndex = field
		loc.hasField = true
	}
	return loc, nil
}

// resolvePartURI maps a location's part label to a concrete part URI in the package.
func resolvePartURIForLocation(loc *fieldLocation, documentURI string, headerFooterURIs map[string]string) (string, error) {
	if loc.part == "body" {
		return documentURI, nil
	}
	if uri, ok := headerFooterURIs[loc.part]; ok {
		return uri, nil
	}
	return "", TargetNotFoundError(fmt.Sprintf("part %q (use 'docx fields list' to discover locations)", loc.part))
}

// headerFooterLabelMap builds a map from part labels (e.g. "header1") to part URIs.
func headerFooterLabelMap(listing *docxinspect.DocumentHeaderFooters) map[string]string {
	out := make(map[string]string)
	if listing == nil {
		return out
	}
	add := func(ref *docxinspect.HeaderFooterRef) {
		if ref == nil || ref.PartURI == "" {
			return
		}
		label := ref.PartURI
		if idx := strings.LastIndex(label, "/"); idx >= 0 {
			label = label[idx+1:]
		}
		label = strings.TrimSuffix(label, ".xml")
		out[label] = ref.PartURI
	}
	for _, section := range listing.Sections {
		for _, set := range []*docxinspect.HeaderFooterSet{section.Headers, section.Footers} {
			if set == nil {
				continue
			}
			add(set.Default)
			add(set.First)
			add(set.Even)
		}
	}
	return out
}

// mapDOCXFieldMutationError translates mutate-layer field errors to CLI errors.
func mapDOCXFieldMutationError(err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	switch {
	case errors.Is(err, docxmutate.ErrFieldNotFound):
		return TargetNotFoundError("field")
	case errors.Is(err, docxmutate.ErrFieldInTable):
		return InvalidArgsError(err.Error() + " (it is listed with editable=false by 'docx fields list'; editing table-nested fields is not yet supported)")
	case errors.Is(err, docxmutate.ErrFieldParaOutOfRange):
		return TargetNotFoundError("field target paragraph")
	case errors.Is(err, docxmutate.ErrInvalidFieldCode):
		return InvalidArgsError("--field-code must be a non-empty instruction (e.g. PAGE)")
	case errors.Is(err, docxmutate.ErrFieldHashMismatch):
		return InvalidArgsError(err.Error())
	default:
		return NewCLIErrorf(ExitUnexpected, "failed to mutate fields: %v", err)
	}
}

// outputDOCXFieldJSON marshals a field result honoring --pretty.
func outputDOCXFieldJSON(cmd *cobra.Command, result interface{}, label string) error {
	return writeLabeledJSON(cmd, result, label)
}

// docxFieldsListCommand renders a follow-up `docx fields list` command for readback.
func docxFieldsListCommand(filePath string) string {
	return fmt.Sprintf("ooxml --json docx fields list %s", filePath)
}

// docxValidateStrictCommand renders a follow-up `validate --strict` command.
func docxValidateStrictCommand(filePath string) string {
	return fmt.Sprintf("ooxml validate --strict %s", filePath)
}

func init() {
	docxCmd.AddCommand(docxFieldsCmd)
}
