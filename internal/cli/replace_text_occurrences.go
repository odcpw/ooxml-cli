package cli

import (
	"errors"
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxhandle "github.com/ooxml-cli/ooxml-cli/pkg/pptx/handle"
	pptxinspect "github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	pptxselectors "github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
	"github.com/spf13/cobra"
)

var (
	replaceTextOccurrencesMatchText      string
	replaceTextOccurrencesNewText        string
	replaceTextOccurrencesNewTextFile    string
	replaceTextOccurrencesForSlides      string
	replaceTextOccurrencesForShape       string
	replaceTextOccurrencesIgnoreCase     bool
	replaceTextOccurrencesExpectCount    int
	replaceTextOccurrencesExpectPlanHash string
	replaceTextOccurrencesAllowZero      bool
)

var replaceTextOccurrencesCmd = &cobra.Command{
	Use:   "text-occurrences <file>",
	Short: "Replace matching slide text occurrences across a deck",
	Long: `Replace matching text occurrences across slide-visible PPTX text nodes.

This command is for practical deck rebranding and repeated-label updates. It
preserves surrounding runs and table-cell formatting when each match is fully
contained inside one XML text node. It scans normal slide shapes and table
cells. Notes, layouts, masters, comments, charts, and matches split across
multiple runs are not changed by this command.

Examples:
  ooxml --json pptx replace text-occurrences deck.pptx --match-text "Old Client" --new-text "New Client" --dry-run
  ooxml --json pptx replace text-occurrences deck.pptx --match-text "Old Client" --new-text "New Client" --expect-count 12 --expect-plan-hash sha256:... --out edited.pptx
  ooxml --json pptx replace text-occurrences deck.pptx --for-slides "1-3,5" --match-text "FY25" --new-text "FY26" --out edited.pptx`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		result, err := performReplaceTextOccurrences(filePath, cmd)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputReplaceTextOccurrencesJSON(cmd, result)
		}
		return outputReplaceTextOccurrencesText(cmd, result)
	},
}

type replaceTextOccurrencesResult struct {
	File       string                              `json:"file"`
	Output     string                              `json:"output,omitempty"`
	DryRun     bool                                `json:"dryRun"`
	Operation  string                              `json:"operation"`
	MatchText  string                              `json:"matchText"`
	NewText    string                              `json:"newText"`
	IgnoreCase bool                                `json:"ignoreCase"`
	ForSlides  string                              `json:"forSlides,omitempty"`
	StaleGuard replaceTextOccurrencesStaleGuard    `json:"staleGuard"`
	Summary    replaceTextOccurrencesSummary       `json:"summary"`
	Scope      mutate.TextOccurrencesReplaceScope  `json:"scope"`
	Matches    []replaceTextOccurrencesMatchResult `json:"matches"`
	PPTXBridgeReadbackCommands
}

type replaceTextOccurrencesStaleGuard struct {
	ExpectedCount    *int   `json:"expectedCount,omitempty"`
	ActualCount      int    `json:"actualCount"`
	ExpectedPlanHash string `json:"expectedPlanHash,omitempty"`
	ActualPlanHash   string `json:"actualPlanHash"`
	AllowZero        bool   `json:"allowZero"`
}

type replaceTextOccurrencesSummary struct {
	SlidesScanned      int `json:"slidesScanned"`
	TargetsScanned     int `json:"targetsScanned"`
	TextNodesScanned   int `json:"textNodesScanned"`
	ChangedTargetCount int `json:"changedTargetCount"`
	ReplacementCount   int `json:"replacementCount"`
}

type replaceTextOccurrencesMatchResult struct {
	mutate.TextOccurrenceMatch
	PPTXBridgeReadbackCommands
}

func performReplaceTextOccurrences(filePath string, cmd *cobra.Command) (*replaceTextOccurrencesResult, error) {
	if _, err := os.Stat(filePath); err != nil {
		return nil, FileNotFoundError(filePath)
	}
	mutOpts, err := GetValidatedMutationOptions(cmd)
	if err != nil {
		return nil, err
	}

	matchText, err := cmd.Flags().GetString("match-text")
	if err != nil {
		return nil, err
	}
	if !cmd.Flags().Lookup("match-text").Changed || matchText == "" {
		return nil, InvalidArgsError("--match-text must be specified and cannot be empty")
	}

	newText, err := resolveTextOccurrencesNewText(cmd)
	if err != nil {
		return nil, err
	}

	slideNums, forSlidesValue, slideHandle, err := resolveTextOccurrencesSlides(cmd)
	if err != nil {
		return nil, err
	}

	shapeHandle, err := resolveTextOccurrencesShape(cmd)
	if err != nil {
		return nil, err
	}
	if shapeHandle != nil && cmd.Flags().Lookup("for-slides").Changed {
		return nil, InvalidArgsError("--for-shape and --for-slides are mutually exclusive; --for-shape already encodes its slide scope")
	}

	var expectCount *int
	if cmd.Flags().Lookup("expect-count").Changed {
		count, err := cmd.Flags().GetInt("expect-count")
		if err != nil {
			return nil, err
		}
		if count < 0 {
			return nil, InvalidArgsError("--expect-count must be >= 0")
		}
		expectCount = &count
	}

	expectPlanHash, err := cmd.Flags().GetString("expect-plan-hash")
	if err != nil {
		return nil, err
	}
	expectPlanHash = strings.TrimSpace(expectPlanHash)

	ignoreCase, err := cmd.Flags().GetBool("ignore-case")
	if err != nil {
		return nil, err
	}
	allowZero, err := cmd.Flags().GetBool("allow-zero")
	if err != nil {
		return nil, err
	}

	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
	if err != nil {
		return nil, err
	}

	destinationFile := mutationOutputPathForResult(filePath, mutOpts)
	var replaceResult *mutate.TextOccurrencesReplaceResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		// A slide handle in --for-slides re-resolves to a slide number against the
		// OPEN package by its durable sldId, so the scope survives slide shifts.
		effectiveSlides := slideNums
		if slideHandle != nil {
			graph, gerr := pptxinspect.ParsePresentation(pkg)
			if gerr != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", gerr)
			}
			num, rerr := pptxselectors.ResolveSlideNumberForHandle(graph, *slideHandle)
			if rerr != nil {
				return mapPPTXHandleError(rerr)
			}
			effectiveSlides = []int{num}
		}
		replaceResult, err = mutate.ReplaceTextOccurrences(&mutate.TextOccurrencesReplaceRequest{
			Package:        pkg,
			SlideNumbers:   effectiveSlides,
			ShapeHandle:    shapeHandle,
			MatchText:      matchText,
			NewText:        newText,
			IgnoreCase:     ignoreCase,
			ExpectCount:    expectCount,
			ExpectPlanHash: expectPlanHash,
			AllowZero:      allowZero,
			FailOnZero:     !mutOpts.DryRun,
		})
		return err
	}); err != nil {
		return nil, mapReplaceTextOccurrencesMutationError(err)
	}

	result := buildReplaceTextOccurrencesCLIResult(filePath, destinationFile, mutOpts, replaceResult, expectCount, expectPlanHash, allowZero, forSlidesValue)
	return result, nil
}

func resolveTextOccurrencesNewText(cmd *cobra.Command) (string, error) {
	newTextFlag := cmd.Flags().Lookup("new-text")
	newTextFileFlag := cmd.Flags().Lookup("new-text-file")
	hasInline := newTextFlag != nil && newTextFlag.Changed
	hasFile := newTextFileFlag != nil && newTextFileFlag.Changed
	if hasInline == hasFile {
		return "", InvalidArgsError("must specify exactly one of --new-text or --new-text-file")
	}
	if hasInline {
		return cmd.Flags().GetString("new-text")
	}
	textFilePath, err := cmd.Flags().GetString("new-text-file")
	if err != nil {
		return "", err
	}
	data, err := os.ReadFile(textFilePath)
	if err != nil {
		return "", FileNotFoundError(textFilePath)
	}
	return string(data), nil
}

// resolveTextOccurrencesSlides parses --for-slides. It returns the resolved
// 1-based slide numbers, the original flag text (for readback), and, when
// --for-slides carries a slide HANDLE, the parsed handle so the slide number can
// be re-resolved against the OPEN package by its durable sldId. A handle in
// --for-slides survives slide insert/delete/reorder (the durable sldId wins over
// any positional number), which is what lets a find->apply batch restrict a
// later op to the right slide after an earlier op shifts slide numbers.
func resolveTextOccurrencesSlides(cmd *cobra.Command) (slideNums []int, forSlidesValue string, slideHandle *pptxhandle.Handle, err error) {
	if !cmd.Flags().Lookup("for-slides").Changed {
		return nil, "", nil, nil
	}
	forSlides, err := cmd.Flags().GetString("for-slides")
	if err != nil {
		return nil, "", nil, err
	}
	if pptxhandle.IsHandle(forSlides) {
		h, perr := pptxhandle.Parse(forSlides)
		if perr != nil {
			return nil, "", nil, mapPPTXHandleError(perr)
		}
		if h.Kind != pptxhandle.KindSlide {
			return nil, "", nil, InvalidArgsError("--for-slides handle must be a slide handle (H:pptx/s:<sldId>)")
		}
		// Slide number is resolved later against the open package; the durable
		// sldId in the handle is authoritative.
		return nil, forSlides, &h, nil
	}
	nums, err := parseSlideSpec(forSlides)
	if err != nil {
		return nil, "", nil, InvalidArgsError(fmt.Sprintf("invalid slide specification: %v", err))
	}
	if len(nums) == 0 {
		return nil, "", nil, InvalidArgsError("no valid slides specified in --for-slides")
	}
	return nums, forSlides, nil, nil
}

// resolveTextOccurrencesShape parses --for-shape. When set it MUST be a shape
// handle (H:pptx/s:<sldId>/shape:n:<id>); the replacement is then confined to
// that ONE shape. The handle encodes its own slide scope and is resolved against
// the OPEN package, so it survives slide+shape reorder/insert/delete. This is the
// scope a find->ops shape hit emits, eliminating cross-shape leakage.
func resolveTextOccurrencesShape(cmd *cobra.Command) (*pptxhandle.Handle, error) {
	if !cmd.Flags().Lookup("for-shape").Changed {
		return nil, nil
	}
	forShape, err := cmd.Flags().GetString("for-shape")
	if err != nil {
		return nil, err
	}
	if strings.TrimSpace(forShape) == "" {
		return nil, InvalidArgsError("--for-shape cannot be empty")
	}
	if !pptxhandle.IsHandle(forShape) {
		return nil, InvalidArgsError("--for-shape must be a shape handle (H:pptx/s:<sldId>/shape:n:<id>)")
	}
	h, perr := pptxhandle.Parse(forShape)
	if perr != nil {
		return nil, mapPPTXHandleError(perr)
	}
	if h.Kind != pptxhandle.KindShape {
		return nil, InvalidArgsError("--for-shape must be a shape handle (H:pptx/s:<sldId>/shape:n:<id>), not a slide handle")
	}
	return &h, nil
}

func buildReplaceTextOccurrencesCLIResult(filePath, destinationFile string, mutOpts *MutationOptions, replaceResult *mutate.TextOccurrencesReplaceResult, expectCount *int, expectPlanHash string, allowZero bool, forSlides string) *replaceTextOccurrencesResult {
	if replaceResult == nil {
		return nil
	}
	result := &replaceTextOccurrencesResult{
		File:       filePath,
		Output:     destinationFile,
		DryRun:     mutOpts != nil && mutOpts.DryRun,
		Operation:  replaceResult.Operation,
		MatchText:  replaceResult.MatchText,
		NewText:    replaceResult.NewText,
		IgnoreCase: replaceResult.IgnoreCase,
		ForSlides:  forSlides,
		StaleGuard: replaceTextOccurrencesStaleGuard{
			ExpectedCount:    expectCount,
			ActualCount:      replaceResult.ReplacementCount,
			ExpectedPlanHash: expectPlanHash,
			ActualPlanHash:   replaceResult.PlanHash,
			AllowZero:        allowZero,
		},
		Summary: replaceTextOccurrencesSummary{
			SlidesScanned:      replaceResult.SlidesScanned,
			TargetsScanned:     replaceResult.TargetsScanned,
			TextNodesScanned:   replaceResult.TextNodesScanned,
			ChangedTargetCount: replaceResult.ChangedTargetCount,
			ReplacementCount:   replaceResult.ReplacementCount,
		},
		Scope:   replaceResult.Scope,
		Matches: make([]replaceTextOccurrencesMatchResult, 0, len(replaceResult.Matches)),
	}
	for _, match := range replaceResult.Matches {
		result.Matches = append(result.Matches, replaceTextOccurrencesMatchResult{
			TextOccurrenceMatch:        match,
			PPTXBridgeReadbackCommands: pptxTextOccurrenceReadbackCommands(destinationFile, match),
		})
	}
	result.PPTXBridgeReadbackCommands = pptxBridgeOutputVerificationCommands(destinationFile)
	return result
}

func pptxTextOccurrenceReadbackCommands(destinationFile string, match mutate.TextOccurrenceMatch) PPTXBridgeReadbackCommands {
	return pptxBridgeReadbackCommands(destinationFile, match.SlideNumber, func(path string) string {
		if match.TargetKind == "table" {
			return pptxTableReadbackCommand(path, match.SlideNumber, match.PrimarySelector)
		}
		return pptxShapeReadbackCommand(path, match.SlideNumber, match.PrimarySelector, true, true)
	})
}

func mapReplaceTextOccurrencesMutationError(err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	// A --for-shape handle that no longer resolves (slide/shape gone, or an
	// ambiguous id) surfaces as a typed handle error from the scan planner; map it
	// to the standard handle-error contract rather than ExitUnexpected.
	if _, ok := err.(*pptxhandle.Error); ok {
		return mapPPTXHandleError(err)
	}
	if errors.Is(err, mutate.ErrTextOccurrencesGuardMismatch) || errors.Is(err, mutate.ErrTextOccurrencesNoMatches) {
		return InvalidArgsError(err.Error())
	}
	message := err.Error()
	if strings.Contains(message, "slide ") && strings.Contains(message, "not found (presentation has") {
		return InvalidArgsError(message)
	}
	if strings.Contains(message, "match text cannot be empty") || strings.Contains(message, "no valid slides") {
		return InvalidArgsError(message)
	}
	return NewCLIErrorf(ExitUnexpected, "failed to replace text occurrences: %v", err)
}

func outputReplaceTextOccurrencesJSON(cmd *cobra.Command, result *replaceTextOccurrencesResult) error {
	data, err := marshalWithConfig(GetGlobalConfig(cmd), result)
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal text occurrences JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputReplaceTextOccurrencesText(cmd *cobra.Command, result *replaceTextOccurrencesResult) error {
	var builder strings.Builder
	builder.WriteString(fmt.Sprintf("Replaced %d occurrence(s) in %d target(s) across %d slide(s)\n", result.Summary.ReplacementCount, result.Summary.ChangedTargetCount, result.Summary.SlidesScanned))
	builder.WriteString(fmt.Sprintf("Plan hash: %s\n", result.StaleGuard.ActualPlanHash))
	if result.Output != "" {
		builder.WriteString(fmt.Sprintf("Output: %s\n", result.Output))
	}
	return writeCLIOutput(cmd, []byte(builder.String()))
}

func init() {
	replaceTextOccurrencesCmd.Flags().StringVar(&replaceTextOccurrencesMatchText, "match-text", "", "literal text to match in slide-visible text nodes")
	replaceTextOccurrencesCmd.Flags().StringVar(&replaceTextOccurrencesNewText, "new-text", "", "replacement text; may be empty")
	replaceTextOccurrencesCmd.Flags().StringVar(&replaceTextOccurrencesNewTextFile, "new-text-file", "", "path to replacement text")
	replaceTextOccurrencesCmd.Flags().StringVar(&replaceTextOccurrencesForSlides, "for-slides", "", "optional slide specification to restrict replacement (e.g., '1-3,5'), or a slide handle (H:pptx/s:<sldId>) that survives slide reorder/insert/delete")
	replaceTextOccurrencesCmd.Flags().StringVar(&replaceTextOccurrencesForShape, "for-shape", "", "confine the replacement to ONE shape via a shape handle (H:pptx/s:<sldId>/shape:n:<id>); mutually exclusive with --for-slides; survives slide+shape reorder/insert/delete")
	replaceTextOccurrencesCmd.Flags().BoolVar(&replaceTextOccurrencesIgnoreCase, "ignore-case", false, "match text case-insensitively")
	replaceTextOccurrencesCmd.Flags().IntVar(&replaceTextOccurrencesExpectCount, "expect-count", 0, "fail unless the planned replacement count matches this value")
	replaceTextOccurrencesCmd.Flags().StringVar(&replaceTextOccurrencesExpectPlanHash, "expect-plan-hash", "", "fail unless the current dry-run plan hash matches this value")
	replaceTextOccurrencesCmd.Flags().BoolVar(&replaceTextOccurrencesAllowZero, "allow-zero", false, "allow saved output when no occurrences match")
	AddMutationFlags(replaceTextOccurrencesCmd)
	replaceCmd.AddCommand(replaceTextOccurrencesCmd)
}
