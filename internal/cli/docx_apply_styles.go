package cli

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"strings"

	docxhandle "github.com/ooxml-cli/ooxml-cli/pkg/docx/handle"
	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

// DOCXStylesApplyResult is the JSON readback for `docx styles apply`.
type DOCXStylesApplyResult struct {
	File          string `json:"file"`
	Index         int    `json:"index"`
	BlockIndex    int    `json:"blockIndex"`
	BlockID       string `json:"blockId"`
	BlockKind     string `json:"blockKind"`
	Target        string `json:"target"`
	PreviousStyle string `json:"previousStyle,omitempty"`
	Style         string `json:"style"`
	ContentHash   string `json:"contentHash"`
	PreviousHash  string `json:"previousHash"`
	Handle        string `json:"handle,omitempty"`
	StyleHandle   string `json:"styleHandle,omitempty"`
}

var (
	docxStylesApplyIndex  int
	docxStylesApplyHandle string
	docxStylesApplyTarget string
	docxStylesApplyStyle  string
	docxStylesApplyHash   string
)

var docxStylesApplyCmd = &cobra.Command{
	Use:   "apply <file>",
	Short: "Apply a paragraph, run, or table style to DOCX content",
	Long: "Set w:pStyle (paragraph), w:rStyle (run), or w:tblStyle (table) on an existing body block selected by its 1-based block index.\n" +
		"The styleId must exist in word/styles.xml with a matching type unless --no-validate is given.",
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		handleSet := cmd.Flags().Lookup("handle").Changed
		if handleSet && cmd.Flags().Lookup("index").Changed {
			return InvalidArgsError("cannot specify both --index and --handle")
		}
		if !handleSet && docxStylesApplyIndex < 1 {
			return InvalidArgsError("--index must be >= 1 (or pass --handle)")
		}
		target, err := normalizeDOCXStyleTarget(docxStylesApplyTarget)
		if err != nil {
			return err
		}
		if strings.TrimSpace(docxStylesApplyStyle) == "" {
			return InvalidArgsError("--style is required")
		}
		if handleSet && target == "table" {
			return InvalidArgsError("--handle is a paragraph handle; use --index with --target table")
		}
		if docxStylesApplyHash != "" {
			if err := requireDOCXBlockHash(docxStylesApplyHash); err != nil {
				return err
			}
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		handleArg := ""
		if handleSet {
			handleArg = docxStylesApplyHandle
		}
		result, err := performDOCXStylesApply(filePath, target, docxStylesApplyIndex, handleArg, docxStylesApplyStyle, docxStylesApplyHash, !mutOpts.NoValidate, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXStylesApplyJSON(cmd, result)
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("applied %s style to %s %d", result.Style, result.Target, result.Index)))
	},
}

func normalizeDOCXStyleTarget(value string) (string, error) {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "paragraph":
		return "paragraph", nil
	case "run":
		return "run", nil
	case "table":
		return "table", nil
	default:
		return "", InvalidArgsError("--target must be one of paragraph, run, table")
	}
}

func performDOCXStylesApply(filePath, target string, index int, handleArg, styleID, expectedHash string, validate bool, mutOpts *MutationOptions) (*DOCXStylesApplyResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	var result *DOCXStylesApplyResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}
		stylesURI, err := docxinspect.FindStylesPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to resolve styles part: %v", err)
		}

		// Handle-first target: a paragraph handle is authoritative for which block
		// is styled (paragraph/run targets only); --index is ignored.
		targetIndex := index
		styleHandleEcho := ""
		if handleArg != "" {
			resolved, herr := resolveDOCXParagraphHandleBlock(pkg, handleArg)
			if herr != nil {
				return herr
			}
			targetIndex = resolved
		}
		// A style handle passed as --style is resolved to its native styleId.
		if docxhandle.IsHandle(styleID) {
			styleHandleEcho = styleID
			resolvedStyle, herr := resolveDOCXStyleHandleID(pkg, styleID)
			if herr != nil {
				return herr
			}
			styleID = resolvedStyle
		}

		var (
			applyResult *docxmutate.ApplyStyleResult
			applyErr    error
		)
		switch target {
		case "paragraph":
			applyResult, applyErr = docxmutate.ApplyParagraphStyle(&docxmutate.ApplyParagraphStyleRequest{
				Package:      pkg,
				DocumentURI:  documentURI,
				StylesURI:    stylesURI,
				Index:        targetIndex,
				StyleID:      styleID,
				ExpectedHash: expectedHash,
				Validate:     validate,
			})
		case "run":
			applyResult, applyErr = docxmutate.ApplyRunStyle(&docxmutate.ApplyRunStyleRequest{
				Package:      pkg,
				DocumentURI:  documentURI,
				StylesURI:    stylesURI,
				Index:        targetIndex,
				StyleID:      styleID,
				ExpectedHash: expectedHash,
				Validate:     validate,
			})
		case "table":
			applyResult, applyErr = docxmutate.ApplyTableStyle(&docxmutate.ApplyTableStyleRequest{
				Package:      pkg,
				DocumentURI:  documentURI,
				StylesURI:    stylesURI,
				Index:        targetIndex,
				StyleID:      styleID,
				ExpectedHash: expectedHash,
				Validate:     validate,
			})
		}
		if applyErr != nil {
			return mapDOCXStyleApplyError(target, targetIndex, applyErr)
		}
		// Surface a paragraph handle for the styled block (paragraph/run); the
		// mutate stamps the marker as part of the lazy-upgrade contract.
		blockHandle := docxParagraphHandleString(applyResult.ParaID)
		result = &DOCXStylesApplyResult{
			File:          filePath,
			Index:         applyResult.Index,
			BlockIndex:    applyResult.BlockIndex,
			BlockID:       fmt.Sprintf("body.b%d", applyResult.BlockIndex),
			BlockKind:     applyResult.BlockKind,
			Target:        applyResult.Target,
			PreviousStyle: applyResult.PreviousStyle,
			Style:         applyResult.Style,
			ContentHash:   applyResult.ContentHash,
			PreviousHash:  applyResult.PreviousHash,
			Handle:        blockHandle,
			StyleHandle:   styleHandleEcho,
		}
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func mapDOCXStyleApplyError(target string, index int, err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	switch {
	case errors.Is(err, docxmutate.ErrStyleNotFound):
		return NewCLIErrorf(ExitTargetNotFound, "%v", err)
	case errors.Is(err, docxmutate.ErrStyleTypeMismatch):
		return NewCLIErrorf(ExitInvalidArgs, "%v", err)
	case errors.Is(err, docxmutate.ErrBlockHashMismatch):
		return NewCLIErrorf(ExitInvalidArgs, "%v", err)
	case errors.Is(err, docxmutate.ErrBlockIndexOutOfRange):
		return TargetNotFoundError(fmt.Sprintf("%s block %d", target, index))
	case errors.Is(err, docxmutate.ErrTableIndexOutOfRange):
		return TargetNotFoundError(fmt.Sprintf("table %d", index))
	case errors.Is(err, docxmutate.ErrBlockNotParagraph):
		return NewCLIErrorf(ExitInvalidArgs, "block %d is a table, not a paragraph", index)
	default:
		return NewCLIErrorf(ExitUnexpected, "failed to apply style: %v", err)
	}
}

func outputDOCXStylesApplyJSON(cmd *cobra.Command, result *DOCXStylesApplyResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal styles apply JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func init() {
	docxStylesApplyCmd.Flags().IntVar(&docxStylesApplyIndex, "index", 0, "1-based block index (paragraph block for paragraph/run; 1-based table number for table)")
	docxStylesApplyCmd.Flags().StringVar(&docxStylesApplyHandle, "handle", "", "stable paragraph handle (H:docx/pt:doc/para:m:<paraId>) for paragraph/run targets; authoritative, ignores --index")
	docxStylesApplyCmd.Flags().StringVar(&docxStylesApplyTarget, "target", "", "style target: paragraph, run, or table")
	docxStylesApplyCmd.Flags().StringVar(&docxStylesApplyStyle, "style", "", "styleId to apply (must exist in word/styles.xml); also accepts a style handle H:docx/pt:styles/style:n:<styleId>")
	docxStylesApplyCmd.Flags().StringVar(&docxStylesApplyHash, "expect-hash", "", "expected sha256: block hash from docx blocks (optional guard)")
	AddMutationFlags(docxStylesApplyCmd)
	docxStylesCmd.AddCommand(docxStylesApplyCmd)
}
