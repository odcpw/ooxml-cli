package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strconv"
	"strings"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

var docxHeadersCmd = &cobra.Command{
	Use:     "headers",
	Aliases: []string{"header"},
	Short:   "Inspect and edit DOCX headers",
	Long:    "Commands for listing, showing, and editing document section headers.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

var docxFootersCmd = &cobra.Command{
	Use:     "footers",
	Aliases: []string{"footer"},
	Short:   "Inspect and edit DOCX footers",
	Long:    "Commands for listing, showing, and editing document section footers.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

type DOCXHeadersListResult struct {
	File            string                             `json:"file"`
	DocumentPartURI string                             `json:"documentPartUri"`
	Sections        []docxinspect.SectionHeaderFooters `json:"sections"`
}

func newDOCXHeadersListCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "list <file>",
		Short: "List headers and footers defined per section",
		Long:  "List each section's resolved header and footer references (id, type, part).",
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
			listing, err := docxinspect.ListHeadersFooters(pkg, documentURI)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to list headers/footers: %v", err)
			}
			result := &DOCXHeadersListResult{
				File:            filePath,
				DocumentPartURI: listing.DocumentPartURI,
				Sections:        listing.Sections,
			}
			if GetGlobalConfig(cmd).Format == "json" {
				return outputDOCXHeadersListJSON(cmd, result)
			}
			return outputDOCXHeadersListText(cmd, result)
		},
	}
}

func outputDOCXHeadersListJSON(cmd *cobra.Command, result *DOCXHeadersListResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal headers list JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputDOCXHeadersListText(cmd *cobra.Command, result *DOCXHeadersListResult) error {
	headers, footers := 0, 0
	for _, section := range result.Sections {
		headers += countSet(section.Headers)
		footers += countSet(section.Footers)
	}
	text := fmt.Sprintf("listed %s and %s in %s",
		plural(headers, "header"), plural(footers, "footer"), plural(len(result.Sections), "section"))
	return writeCLIOutput(cmd, []byte(text))
}

func countSet(set *docxinspect.HeaderFooterSet) int {
	if set == nil {
		return 0
	}
	n := 0
	if set.Default != nil {
		n++
	}
	if set.First != nil {
		n++
	}
	if set.Even != nil {
		n++
	}
	return n
}

func plural(n int, noun string) string {
	if n == 1 {
		return fmt.Sprintf("%d %s", n, noun)
	}
	return fmt.Sprintf("%d %ss", n, noun)
}

func normalizeHeaderFooterType(value string) (string, error) {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "", docxinspect.TypeDefault:
		return docxinspect.TypeDefault, nil
	case docxinspect.TypeFirst:
		return docxinspect.TypeFirst, nil
	case docxinspect.TypeEven:
		return docxinspect.TypeEven, nil
	default:
		return "", InvalidArgsError("--type must be one of default, first, even")
	}
}

type docxHeaderFooterSelector struct {
	Raw            string
	Kind           string
	RefType        string
	Section        int
	ID             string
	PartURI        string
	ParagraphIndex int
}

func parseDOCXHeaderFooterSelector(commandKind, raw string) (docxHeaderFooterSelector, error) {
	selector := docxHeaderFooterSelector{
		Raw:     strings.TrimSpace(raw),
		Kind:    commandKind,
		RefType: docxinspect.TypeDefault,
	}
	if selector.Raw == "" {
		return selector, InvalidArgsError("--selector cannot be empty")
	}

	base, paragraphIndex, err := splitDOCXHeaderFooterParagraphSelector(selector.Raw)
	if err != nil {
		return selector, err
	}
	selector.ParagraphIndex = paragraphIndex

	if strings.HasPrefix(base, "id:") {
		selector.ID = strings.TrimPrefix(base, "id:")
		if selector.ID == "" {
			return selector, InvalidArgsError("--selector id:<relId> cannot be empty")
		}
		return selector, nil
	}
	if strings.HasPrefix(base, "part:") {
		selector.PartURI = strings.TrimPrefix(base, "part:")
		if selector.PartURI == "" {
			return selector, InvalidArgsError("--selector part:<partUri> cannot be empty")
		}
		return selector, nil
	}
	if strings.HasPrefix(base, "/") {
		selector.PartURI = base
		return selector, nil
	}
	if strings.HasPrefix(base, "rId") {
		selector.ID = base
		return selector, nil
	}
	if strings.HasPrefix(base, "section:") {
		parts := strings.Split(base, ":")
		if len(parts) != 4 || parts[2] != "type" {
			return selector, InvalidArgsError("--selector section form must be section:<n>:type:<default|first|even>")
		}
		section, err := parseDOCXHeaderFooterPositiveInt(parts[1], "selector section")
		if err != nil {
			return selector, err
		}
		refType, err := normalizeHeaderFooterType(parts[3])
		if err != nil {
			return selector, err
		}
		selector.Section = section
		selector.RefType = refType
		return selector, nil
	}

	parts := strings.Split(base, ":")
	if len(parts) == 3 && (parts[0] == docxinspect.KindHeader || parts[0] == docxinspect.KindFooter) {
		if parts[0] != commandKind {
			return selector, InvalidArgsError(fmt.Sprintf("--selector kind %q does not match %s command", parts[0], commandKind))
		}
		section, err := parseDOCXHeaderFooterPositiveInt(parts[1], "selector section")
		if err != nil {
			return selector, err
		}
		refType, err := normalizeHeaderFooterType(parts[2])
		if err != nil {
			return selector, err
		}
		selector.Kind = parts[0]
		selector.Section = section
		selector.RefType = refType
		return selector, nil
	}

	return selector, InvalidArgsError("--selector must be header:<section>:<type>, footer:<section>:<type>, section:<section>:type:<type>, id:<relId>, or part:<partUri>")
}

func splitDOCXHeaderFooterParagraphSelector(raw string) (string, int, error) {
	for _, marker := range []string{"/paragraph:", "/p:"} {
		if idx := strings.LastIndex(raw, marker); idx >= 0 {
			base := strings.TrimSpace(raw[:idx])
			value := strings.TrimSpace(raw[idx+len(marker):])
			if base == "" {
				return "", 0, InvalidArgsError("--selector paragraph suffix requires a header/footer selector before it")
			}
			paragraphIndex, err := parseDOCXHeaderFooterPositiveInt(value, "selector paragraph")
			if err != nil {
				return "", 0, err
			}
			return base, paragraphIndex, nil
		}
	}
	return raw, 0, nil
}

func parseDOCXHeaderFooterPositiveInt(value, label string) (int, error) {
	value = strings.TrimSpace(value)
	if value == "" {
		return 0, InvalidArgsError(label + " cannot be empty")
	}
	n, err := strconv.Atoi(value)
	if err != nil {
		return 0, InvalidArgsError(fmt.Sprintf("%s must be an integer", label))
	}
	if n < 1 {
		return 0, InvalidArgsError(fmt.Sprintf("%s must be >= 1", label))
	}
	return n, nil
}

func resolveDOCXHeaderFooterByPartSelector(pkg opc.PackageSession, documentURI, kind, partURI string) (*docxinspect.HeaderFooterRef, error) {
	listing, err := docxinspect.ListHeadersFooters(pkg, documentURI)
	if err != nil {
		return nil, err
	}
	for _, ref := range docxinspect.HeaderFooterRefs(listing, kind) {
		if ref.PartURI == partURI {
			return ref, nil
		}
	}
	return nil, fmt.Errorf("%s part %q not found in section references", kind, partURI)
}

func resolveDOCXHeaderFooterSelector(pkg opc.PackageSession, documentURI, kind string, selector docxHeaderFooterSelector) (*docxinspect.HeaderFooterRef, error) {
	switch {
	case selector.ID != "":
		return docxinspect.ResolveHeaderFooter(pkg, documentURI, kind, selector.RefType, selector.ID, selector.Section)
	case selector.PartURI != "":
		return resolveDOCXHeaderFooterByPartSelector(pkg, documentURI, kind, selector.PartURI)
	default:
		return docxinspect.ResolveHeaderFooter(pkg, documentURI, kind, selector.RefType, "", selector.Section)
	}
}

func docxHeaderFooterSelectorCandidates(pkg opc.PackageSession, documentURI, kind string) []SelectorCandidate {
	listing, err := docxinspect.ListHeadersFooters(pkg, documentURI)
	if err != nil {
		return []SelectorCandidate{}
	}
	refs := docxinspect.HeaderFooterRefs(listing, kind)
	out := make([]SelectorCandidate, 0, len(refs))
	for _, ref := range refs {
		out = append(out, SelectorCandidate{Primary: ref.PrimarySelector, Selectors: ref.Selectors})
	}
	return out
}

func docxHeaderFooterNotFoundError(pkg opc.PackageSession, documentURI, kind, selector string) error {
	candidates := docxHeaderFooterSelectorCandidates(pkg, documentURI, kind)
	return SelectorNotFoundError(kind, selector, BuildSelectorCandidates(candidates, selector, maxSelectorCandidates), docxHeaderFooterListDiscoveryCommand(kind))
}

func docxHeaderFooterListDiscoveryCommand(kind string) string {
	if kind == docxinspect.KindFooter {
		return "ooxml --json docx footers list <file>"
	}
	return "ooxml --json docx headers list <file>"
}

func docxHeaderFooterShowDiscoveryCommand(kind, selector string) string {
	command := "ooxml --json docx headers show <file>"
	if kind == docxinspect.KindFooter {
		command = "ooxml --json docx footers show <file>"
	}
	if strings.TrimSpace(selector) != "" {
		command += " --selector " + pptxXLSXCommandArg(selector)
	}
	return command
}

func docxHeaderFooterRequestedSelector(pkg opc.PackageSession, documentURI, kind, refType, id string, section int) string {
	if id != "" {
		return "id:" + id
	}
	if section < 1 {
		if listing, err := docxinspect.ListHeadersFooters(pkg, documentURI); err == nil && len(listing.Sections) > 0 {
			section = listing.Sections[len(listing.Sections)-1].SectionIndex
		}
	}
	if section < 1 {
		section = 1
	}
	return docxinspect.HeaderFooterPrimarySelector(kind, section, refType)
}

func docxHeaderFooterParagraphNotFoundError(pkg opc.PackageSession, ref *docxinspect.HeaderFooterRef, kind string, index int) error {
	selector := ""
	if ref != nil {
		selector = docxinspect.HeaderFooterParagraphPrimarySelector(ref.PrimarySelector, index)
	}
	if selector == "" {
		selector = fmt.Sprintf("%s paragraph %d", kind, index)
	}
	candidates := docxHeaderFooterParagraphSelectorCandidates(pkg, ref)
	discovery := ""
	if ref != nil {
		discovery = docxHeaderFooterShowDiscoveryCommand(kind, ref.PrimarySelector)
	}
	return SelectorNotFoundError(kind+" paragraph", selector, BuildSelectorCandidates(candidates, selector, maxSelectorCandidates), discovery)
}

func docxHeaderFooterParagraphSelectorCandidates(pkg opc.PackageSession, ref *docxinspect.HeaderFooterRef) []SelectorCandidate {
	if ref == nil || ref.PartURI == "" {
		return []SelectorCandidate{}
	}
	paragraphs, err := docxinspect.ReadHeaderFooterParagraphs(pkg, ref.PartURI)
	if err != nil {
		return []SelectorCandidate{}
	}
	paragraphs = docxinspect.AnnotateHeaderFooterParagraphs(ref, paragraphs)
	out := make([]SelectorCandidate, 0, len(paragraphs))
	for _, paragraph := range paragraphs {
		out = append(out, SelectorCandidate{Primary: paragraph.PrimarySelector, Selectors: paragraph.Selectors})
	}
	return out
}

type DOCXHeaderFooterReadbackCommands struct {
	ValidateCommand         string `json:"validateCommand,omitempty"`
	ShowCommand             string `json:"showCommand,omitempty"`
	ListCommand             string `json:"listCommand,omitempty"`
	ValidateCommandTemplate string `json:"validateCommandTemplate,omitempty"`
	ShowCommandTemplate     string `json:"showCommandTemplate,omitempty"`
	ListCommandTemplate     string `json:"listCommandTemplate,omitempty"`
}

func docxHeaderFooterMutationReadbackCommands(destinationFile, kind, selector string) DOCXHeaderFooterReadbackCommands {
	if destinationFile == "" {
		placeholder := "<out.docx>"
		return DOCXHeaderFooterReadbackCommands{
			ValidateCommandTemplate: docxValidateStrictCommand(placeholder),
			ShowCommandTemplate:     docxHeaderFooterConcreteShowCommand(placeholder, kind, selector),
			ListCommandTemplate:     docxHeaderFooterConcreteListCommand(placeholder, kind),
		}
	}
	return DOCXHeaderFooterReadbackCommands{
		ValidateCommand: docxValidateStrictCommand(destinationFile),
		ShowCommand:     docxHeaderFooterConcreteShowCommand(destinationFile, kind, selector),
		ListCommand:     docxHeaderFooterConcreteListCommand(destinationFile, kind),
	}
}

func docxHeaderFooterConcreteShowCommand(filePath, kind, selector string) string {
	command := "ooxml --json docx headers show "
	if kind == docxinspect.KindFooter {
		command = "ooxml --json docx footers show "
	}
	command += pptxXLSXCommandArg(filePath)
	if strings.TrimSpace(selector) != "" {
		command += " --selector " + pptxXLSXCommandArg(selector)
	}
	return command
}

func docxHeaderFooterConcreteListCommand(filePath, kind string) string {
	if kind == docxinspect.KindFooter {
		return "ooxml --json docx footers list " + pptxXLSXCommandArg(filePath)
	}
	return "ooxml --json docx headers list " + pptxXLSXCommandArg(filePath)
}

func init() {
	docxCmd.AddCommand(docxHeadersCmd)
	docxCmd.AddCommand(docxFootersCmd)

	headersList := newDOCXHeadersListCmd()
	footersList := newDOCXHeadersListCmd()
	docxHeadersCmd.AddCommand(headersList)
	docxFootersCmd.AddCommand(footersList)
}
