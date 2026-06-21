package cli

import (
	"encoding/json"
	"errors"
	"fmt"
	"regexp"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/docx/extract"
	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

type DOCXBlockParagraphResult struct {
	File         string               `json:"file"`
	Index        int                  `json:"index"`
	BlockID      string               `json:"blockId"`
	ContentHash  string               `json:"contentHash"`
	PreviousKind string               `json:"previousKind,omitempty"`
	PreviousHash string               `json:"previousHash,omitempty"`
	PreviousText string               `json:"previousText,omitempty"`
	AnchorHash   string               `json:"anchorHash,omitempty"`
	InsertAfter  int                  `json:"insertAfter,omitempty"`
	Style        string               `json:"style,omitempty"`
	Text         string               `json:"text"`
	Destination  *extract.BlockReport `json:"destination,omitempty"`
}

// collectDOCXBlockDestination re-extracts a single body block by its 1-based
// index in the already-mutated package session so a mutation's readback shares
// the exact same shape (extract.BlockReport) that `ooxml docx blocks --block N`
// emits. Returns nil (without error) when the block can no longer be read, so a
// best-effort readback never fails the mutation itself.
func collectDOCXBlockDestination(pkg opc.PackageSession, index int) *extract.BlockReport {
	if index < 1 {
		return nil
	}
	documentURI, err := docxinspect.FindMainDocumentPart(pkg)
	if err != nil {
		return nil
	}
	extracted, err := extract.ExtractBlocks(&extract.ExtractBlocksRequest{
		Session:     pkg,
		DocumentURI: documentURI,
		Block:       index,
		IncludeRuns: true,
	})
	if err != nil || extracted == nil || len(extracted.Blocks) == 0 {
		return nil
	}
	block := extracted.Blocks[0]
	return &block
}

type DOCXBlockDeleteResult struct {
	File         string `json:"file"`
	Index        int    `json:"index"`
	BlockID      string `json:"blockId"`
	PreviousKind string `json:"previousKind"`
	PreviousHash string `json:"previousHash"`
	PreviousText string `json:"previousText"`
}

var docxBlockHashPattern = regexp.MustCompile(`^sha256:[0-9a-f]{64}$`)

func requireDOCXBlockHash(value string) error {
	if strings.TrimSpace(value) == "" {
		return InvalidArgsError("--expect-hash is required")
	}
	if !docxBlockHashPattern.MatchString(value) {
		return InvalidArgsError("--expect-hash must match sha256:<64 lowercase hex chars> from docx blocks")
	}
	return nil
}

func mapDOCXBlockMutationError(target string, err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	switch {
	case errors.Is(err, docxmutate.ErrBlockIndexOutOfRange):
		return TargetNotFoundError(target)
	case errors.Is(err, docxmutate.ErrBlockHashMismatch):
		return NewCLIErrorf(ExitInvalidArgs, "%v", err)
	case errors.Is(err, docxmutate.ErrDeleteLastBlock):
		return NewCLIErrorf(ExitInvalidArgs, "%v", err)
	case errors.Is(err, docxmutate.ErrBlockHasSectionPr):
		return NewCLIErrorf(ExitInvalidArgs, "%v", err)
	default:
		return NewCLIErrorf(ExitUnexpected, "failed to mutate block: %v", err)
	}
}

func outputDOCXBlockParagraphJSON(cmd *cobra.Command, result *DOCXBlockParagraphResult, label string) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal %s JSON: %v", label, err)
	}
	return writeCLIOutput(cmd, data)
}

func outputDOCXBlockDeleteJSON(cmd *cobra.Command, result *DOCXBlockDeleteResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal blocks delete JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputDOCXBlockParagraphText(cmd *cobra.Command, action string, result *DOCXBlockParagraphResult) error {
	text := fmt.Sprintf("%s block %d", action, result.Index)
	if result.Text != "" {
		text += fmt.Sprintf(" = %q", result.Text)
	}
	return writeCLIOutput(cmd, []byte(text))
}

func outputDOCXBlockDeleteText(cmd *cobra.Command, result *DOCXBlockDeleteResult) error {
	return writeCLIOutput(cmd, []byte(fmt.Sprintf("deleted %s block %d", result.PreviousKind, result.Index)))
}
