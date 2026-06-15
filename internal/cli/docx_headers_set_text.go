package cli

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

type DOCXHeadersSetTextResult struct {
	File                     string   `json:"file"`
	Output                   string   `json:"output,omitempty"`
	DryRun                   bool     `json:"dryRun"`
	Kind                     string   `json:"kind"`
	PartURI                  string   `json:"partUri"`
	ID                       string   `json:"id"`
	Type                     string   `json:"type"`
	Section                  int      `json:"section"`
	PrimarySelector          string   `json:"primarySelector,omitempty"`
	Selectors                []string `json:"selectors,omitempty"`
	ParagraphIndex           int      `json:"paragraphIndex"`
	ParagraphPrimarySelector string   `json:"paragraphPrimarySelector,omitempty"`
	ParagraphSelectors       []string `json:"paragraphSelectors,omitempty"`
	PreviousText             string   `json:"previousText"`
	Text                     string   `json:"text"`
	CreatedPart              bool     `json:"createdPart"`
	CreatedRef               bool     `json:"createdRef"`
	DOCXHeaderFooterReadbackCommands
}

func newDOCXHeadersSetTextCmd(kind string) *cobra.Command {
	var (
		id       string
		refType  string
		section  int
		index    int
		text     string
		textFile string
		selector string
	)
	cmd := &cobra.Command{
		Use:   "set-text <file>",
		Short: fmt.Sprintf("Set %s paragraph text by index", kind),
		Long:  fmt.Sprintf("Replace a %s paragraph's text, creating the %s part and reference if missing.", kind, kind),
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
			if index < 1 {
				return InvalidArgsError("--index must be >= 1")
			}
			if section < 0 {
				return InvalidArgsError("--section must be >= 0 (0 means the last section)")
			}
			resolvedText, err := resolveDOCXHeaderText(cmd, text, textFile)
			if err != nil {
				return err
			}
			mutOpts, err := GetValidatedMutationOptions(cmd)
			if err != nil {
				return err
			}

			result, err := performDOCXHeadersSetText(filePath, kind, id, normType, section, index, selector, selectorGiven, cmd.Flags().Changed("index"), resolvedText, mutOpts)
			if err != nil {
				return err
			}
			if GetGlobalConfig(cmd).Format == "json" {
				return outputDOCXHeadersSetTextJSON(cmd, result)
			}
			return outputDOCXHeadersSetTextText(cmd, result)
		},
	}
	cmd.Flags().StringVar(&id, "id", "", "relationship id to resolve directly (optional)")
	cmd.Flags().StringVar(&refType, "type", "default", "reference type: default, first, or even")
	cmd.Flags().IntVar(&section, "section", 0, "1-based section index (default: last section)")
	cmd.Flags().IntVar(&index, "index", 1, "1-based paragraph index within the part")
	cmd.Flags().StringVar(&selector, "selector", "", "selector from headers/footers list, such as header:1:default or header:1:default/p:1")
	cmd.Flags().StringVar(&text, "text", "", "replacement text")
	cmd.Flags().StringVar(&textFile, "text-file", "", "path to replacement text")
	AddMutationFlags(cmd)
	return cmd
}

func resolveDOCXHeaderText(cmd *cobra.Command, text, textFile string) (string, error) {
	textChanged := cmd.Flags().Lookup("text").Changed
	textFileChanged := cmd.Flags().Lookup("text-file").Changed
	if textChanged == textFileChanged {
		return "", InvalidArgsError("must specify exactly one of --text or --text-file")
	}
	if textChanged {
		return text, nil
	}
	data, err := os.ReadFile(textFile)
	if err != nil {
		return "", FileNotFoundError(textFile)
	}
	return string(data), nil
}

func performDOCXHeadersSetText(filePath, kind, id, refType string, section, index int, selector string, selectorGiven, indexGiven bool, text string, mutOpts *MutationOptions) (*DOCXHeadersSetTextResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	var result *DOCXHeadersSetTextResult
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}

		var (
			partURI      string
			relID        string
			resolvedKind = kind
			createdPart  bool
			createdRef   bool
			ref          *docxinspect.HeaderFooterRef
		)

		if selectorGiven {
			parsed, err := parseDOCXHeaderFooterSelector(kind, selector)
			if err != nil {
				return err
			}
			if parsed.ParagraphIndex > 0 {
				if indexGiven && index != parsed.ParagraphIndex {
					return InvalidArgsError("--index conflicts with the paragraph index embedded in --selector")
				}
				index = parsed.ParagraphIndex
			}
			if parsed.ID != "" || parsed.PartURI != "" {
				ref, err = resolveDOCXHeaderFooterSelector(pkg, documentURI, kind, parsed)
				if err != nil {
					return docxHeaderFooterNotFoundError(pkg, documentURI, kind, parsed.Raw)
				}
				partURI = ref.PartURI
				relID = ref.ID
				refType = ref.Type
				resolvedKind = ref.Kind
			} else {
				section = parsed.Section
				refType = parsed.RefType
			}
		}

		if ref == nil && id != "" {
			ref, err := docxinspect.ResolveHeaderFooter(pkg, documentURI, kind, refType, id, section)
			if err != nil {
				return docxHeaderFooterNotFoundError(pkg, documentURI, kind, "id:"+id)
			}
			partURI = ref.PartURI
			relID = ref.ID
			resolvedKind = ref.Kind
			refType = ref.Type
		}

		if ref == nil {
			ensured, err := docxmutate.EnsureHeaderFooter(&docxmutate.EnsureHeaderFooterRequest{
				Package:      pkg,
				DocumentURI:  documentURI,
				Kind:         kind,
				Type:         refType,
				SectionIndex: section,
			})
			if err != nil {
				return mapDOCXHeaderMutationError(kind, err)
			}
			partURI = ensured.PartURI
			relID = ensured.ID
			resolvedKind = ensured.Kind
			createdPart = ensured.CreatedPart
			createdRef = ensured.CreatedRef
			ref, err = docxinspect.ResolveHeaderFooter(pkg, documentURI, resolvedKind, ensured.Type, ensured.ID, section)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to read back ensured %s: %v", kind, err)
			}
		}

		if partURI == "" {
			return NewCLIErrorf(ExitInvalidArgs, "%s reference %q does not resolve to a part", kind, relID)
		}

		setResult, err := docxmutate.SetHeaderFooterText(&docxmutate.SetHeaderFooterTextRequest{
			Package:        pkg,
			PartURI:        partURI,
			ParagraphIndex: index,
			Text:           text,
		})
		if err != nil {
			if errors.Is(err, docxmutate.ErrHeaderFooterParaOutOfRange) {
				return docxHeaderFooterParagraphNotFoundError(pkg, ref, kind, index)
			}
			return mapDOCXHeaderMutationError(kind, err)
		}

		paragraphPrimary := docxinspect.HeaderFooterParagraphPrimarySelector(ref.PrimarySelector, setResult.ParagraphIndex)
		result = &DOCXHeadersSetTextResult{
			File:                     filePath,
			Output:                   destinationFile,
			DryRun:                   mutOpts.DryRun,
			Kind:                     resolvedKind,
			PartURI:                  setResult.PartURI,
			ID:                       relID,
			Type:                     refType,
			Section:                  ref.Section,
			PrimarySelector:          ref.PrimarySelector,
			Selectors:                ref.Selectors,
			ParagraphIndex:           setResult.ParagraphIndex,
			ParagraphPrimarySelector: paragraphPrimary,
			ParagraphSelectors:       docxinspect.HeaderFooterParagraphSelectors(ref, setResult.ParagraphIndex),
			PreviousText:             setResult.PreviousText,
			Text:                     setResult.Text,
			CreatedPart:              createdPart,
			CreatedRef:               createdRef,
		}
		result.DOCXHeaderFooterReadbackCommands = docxHeaderFooterMutationReadbackCommands(destinationFile, resolvedKind, ref.PrimarySelector)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func mapDOCXHeaderMutationError(kind string, err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	switch {
	case errors.Is(err, docxmutate.ErrHeaderFooterParaOutOfRange):
		return TargetNotFoundError(fmt.Sprintf("%s paragraph", kind))
	case errors.Is(err, docxmutate.ErrHeaderFooterPartNotFound):
		return TargetNotFoundError(fmt.Sprintf("%s part", kind))
	default:
		return NewCLIErrorf(ExitUnexpected, "failed to mutate %s: %v", kind, err)
	}
}

func outputDOCXHeadersSetTextJSON(cmd *cobra.Command, result *DOCXHeadersSetTextResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal %s set-text JSON: %v", result.Kind, err)
	}
	return writeCLIOutput(cmd, data)
}

func outputDOCXHeadersSetTextText(cmd *cobra.Command, result *DOCXHeadersSetTextResult) error {
	text := fmt.Sprintf("set %s paragraph %d = %q", result.Kind, result.ParagraphIndex, result.Text)
	if result.CreatedPart {
		text += " (created part)"
	} else if result.CreatedRef {
		text += " (added reference)"
	}
	return writeCLIOutput(cmd, []byte(text))
}

func init() {
	docxHeadersCmd.AddCommand(newDOCXHeadersSetTextCmd(docxinspect.KindHeader))
	docxFootersCmd.AddCommand(newDOCXHeadersSetTextCmd(docxinspect.KindFooter))
}
