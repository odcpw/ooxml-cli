package cli

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"regexp"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

type DOCXReplaceResult struct {
	File                 string                    `json:"file"`
	TotalReplacements    int                       `json:"totalReplacements"`
	AffectedBlockCount   int                       `json:"affectedBlockCount"`
	AffectedBlockIndices []int                     `json:"affectedBlockIndices"`
	BlockSummaries       []DOCXReplaceBlockSummary `json:"blockSummaries"`
}

type DOCXReplaceBlockSummary struct {
	Index               int    `json:"index"`
	Kind                string `json:"kind"`
	Style               string `json:"style,omitempty"`
	TableIndex          int    `json:"tableIndex,omitempty"`
	RowIndex            int    `json:"rowIndex,omitempty"`
	ColumnIndex         int    `json:"columnIndex,omitempty"`
	ParagraphIndex      int    `json:"paragraphIndex,omitempty"`
	ContentHash         string `json:"contentHash"`
	PreviousHash        string `json:"previousHash"`
	ReplacementsInBlock int    `json:"replacementsInBlock"`
	PreviousText        string `json:"previousText"`
	Text                string `json:"text"`
}

var (
	docxReplaceFind      string
	docxReplaceReplace   string
	docxReplaceRegex     bool
	docxReplaceMatchCase bool
	docxReplaceWholeWord bool
	docxReplaceExpect    int
)

var docxReplaceCmd = &cobra.Command{
	Use:   "replace <file>",
	Short: "Find and replace text across DOCX body text",
	Long:  "Document-wide find/replace across w:t runs in the main body, including top-level paragraphs and table-cell paragraphs. Matching text split across multiple runs within a paragraph is handled by concatenating run text, matching, and writing replacements back.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if !cmd.Flags().Lookup("find").Changed || docxReplaceFind == "" {
			return InvalidArgsError("--find is required and cannot be empty")
		}

		pattern, err := docxmutate.BuildFindReplacePattern(docxReplaceFind, docxReplaceRegex, docxReplaceMatchCase, docxReplaceWholeWord)
		if err != nil {
			return InvalidArgsError(err.Error())
		}

		var expectCount *int
		if cmd.Flags().Lookup("expect-count").Changed {
			if docxReplaceExpect < 0 {
				return InvalidArgsError("--expect-count must be >= 0")
			}
			expectCount = &docxReplaceExpect
		}

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performDOCXReplace(filePath, pattern, docxReplaceReplace, expectCount, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXReplaceJSON(cmd, result)
		}
		return outputDOCXReplaceText(cmd, result)
	},
}

func performDOCXReplace(filePath string, pattern *regexp.Regexp, replace string, expectCount *int, mutOpts *MutationOptions) (*DOCXReplaceResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	var result *DOCXReplaceResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}
		replaceResult, err := docxmutate.FindReplaceInDocument(&docxmutate.FindReplaceRequest{
			Package:     pkg,
			DocumentURI: documentURI,
			Pattern:     pattern,
			Replace:     replace,
			ExpectCount: expectCount,
		})
		if err != nil {
			return mapDOCXReplaceError(err)
		}
		result = buildDOCXReplaceResult(filePath, replaceResult)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func buildDOCXReplaceResult(filePath string, src *docxmutate.FindReplaceResult) *DOCXReplaceResult {
	result := &DOCXReplaceResult{
		File:                 filePath,
		TotalReplacements:    src.TotalReplacements,
		AffectedBlockCount:   src.AffectedBlockCount,
		AffectedBlockIndices: src.AffectedBlockIndices,
		BlockSummaries:       make([]DOCXReplaceBlockSummary, 0, len(src.BlockSummaries)),
	}
	for _, summary := range src.BlockSummaries {
		result.BlockSummaries = append(result.BlockSummaries, DOCXReplaceBlockSummary{
			Index:               summary.Index,
			Kind:                summary.Kind,
			Style:               summary.Style,
			TableIndex:          summary.TableIndex,
			RowIndex:            summary.RowIndex,
			ColumnIndex:         summary.ColumnIndex,
			ParagraphIndex:      summary.ParagraphIndex,
			ContentHash:         summary.ContentHash,
			PreviousHash:        summary.PreviousHash,
			ReplacementsInBlock: summary.ReplacementsInBlock,
			PreviousText:        summary.PreviousText,
			Text:                summary.Text,
		})
	}
	return result
}

func mapDOCXReplaceError(err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	if errors.Is(err, docxmutate.ErrReplacementCountMismatch) {
		return NewCLIErrorf(ExitInvalidArgs, "%v", err)
	}
	return NewCLIErrorf(ExitUnexpected, "failed to replace text: %v", err)
}

func outputDOCXReplaceJSON(cmd *cobra.Command, result *DOCXReplaceResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal replace JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputDOCXReplaceText(cmd *cobra.Command, result *DOCXReplaceResult) error {
	text := fmt.Sprintf("replaced %d occurrences in %d blocks", result.TotalReplacements, result.AffectedBlockCount)
	return writeCLIOutput(cmd, []byte(text))
}

func init() {
	docxReplaceCmd.Flags().StringVar(&docxReplaceFind, "find", "", "text or regex pattern to find (required)")
	docxReplaceCmd.Flags().StringVar(&docxReplaceReplace, "replace", "", "replacement text (inserted literally)")
	docxReplaceCmd.Flags().BoolVar(&docxReplaceRegex, "regex", false, "treat --find as a regular expression")
	docxReplaceCmd.Flags().BoolVar(&docxReplaceMatchCase, "match-case", false, "case-sensitive matching")
	docxReplaceCmd.Flags().BoolVar(&docxReplaceWholeWord, "whole-word", false, "match whole words only")
	docxReplaceCmd.Flags().IntVar(&docxReplaceExpect, "expect-count", 0, "expected number of replacements; when set, errors if the actual count differs")
	AddMutationFlags(docxReplaceCmd)
	docxCmd.AddCommand(docxReplaceCmd)
}
