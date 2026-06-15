package cli

import (
	"errors"
	"fmt"
	"os"

	"github.com/spf13/cobra"

	docxdiff "github.com/ooxml-cli/ooxml-cli/pkg/docx/diff"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxdiff "github.com/ooxml-cli/ooxml-cli/pkg/pptx/diff"
	pkgrender "github.com/ooxml-cli/ooxml-cli/pkg/render"
	xlsxdiff "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/diff"
)

// FamilyDiffSchemaVersion pins the family-general diff envelope contract.
const FamilyDiffSchemaVersion = "1.0"

// Injectable for tests; mirror semanticDiffFn in diff.go.
var (
	xlsxSemanticDiffFn = xlsxdiff.SemanticDiff
	docxSemanticDiffFn = docxdiff.SemanticDiff
)

var (
	familyDiffRender    bool
	familyDiffThreshold float64
)

// FamilyDiffResult is the unified family-general diff envelope. Exactly one of
// the family report fields is populated based on the detected package type.
type FamilyDiffResult struct {
	SchemaVersion string        `json:"schemaVersion"`
	Type          string        `json:"type"`
	Semantic      interface{}   `json:"semantic"`
	Visual        *VisualResult `json:"visual,omitempty"`
}

var familyDiffCmd = &cobra.Command{
	Use:   "diff <baseline> <candidate>",
	Short: "Compare two OOXML packages semantically (PPTX, XLSX, DOCX)",
	Long: "Compare two OOXML packages and report a deterministic semantic diff.\n\n" +
		"The package family is detected from the inputs (both must match):\n" +
		"  PPTX: slide/text/image/format/geometry changes (with optional --render visual diff)\n" +
		"  XLSX: sheet/cell/formula/defined-name/table changes\n" +
		"  DOCX: block/paragraph-text/style/table changes\n\n" +
		"Output is a JSON envelope {schemaVersion, type, semantic, visual?}.",
	Args: cobra.ExactArgs(2),
	RunE: func(cmd *cobra.Command, args []string) error {
		baselinePath, candidatePath := args[0], args[1]
		if _, err := os.Stat(baselinePath); err != nil {
			return FileNotFoundError(baselinePath)
		}
		if _, err := os.Stat(candidatePath); err != nil {
			return FileNotFoundError(candidatePath)
		}

		baseline, baseType, err := openDetect(baselinePath)
		if err != nil {
			return err
		}
		defer baseline.Close()
		candidate, candType, err := openDetect(candidatePath)
		if err != nil {
			return err
		}
		defer candidate.Close()

		if baseType != candType {
			return NewCLIErrorf(ExitUnsupportedType,
				"cannot diff different package types (baseline: %s, candidate: %s)",
				baseType, candType)
		}

		result, deferredErr, err := computeFamilyDiff(cmd, baseType, baseline, candidate, baselinePath, candidatePath)
		if err != nil {
			return err
		}

		if GetGlobalConfig(cmd).Format == "json" {
			if err := writeGlobalJSON(cmd, result); err != nil {
				return err
			}
			if cliErr, ok := AsCLIError(deferredErr); ok {
				cliErr.Reported = true
			}
		} else {
			if err := outputFamilyDiffText(cmd, result); err != nil {
				return err
			}
		}
		return deferredErr
	},
}

// openDetect opens a package and returns its detected type.
func openDetect(path string) (*opc.Package, opc.PackageType, error) {
	pkg, err := opc.Open(path)
	if err != nil {
		return nil, "", NewCLIErrorf(ExitUnexpected, "failed to open package: %v", err)
	}
	return pkg, opc.DetectType(pkg), nil
}

// computeFamilyDiff dispatches to the family-specific semantic diff and, for
// PPTX with --render, the visual diff. It returns the result, an optional
// deferred CLIError (e.g. render unavailable / threshold exceeded), and a hard
// error.
func computeFamilyDiff(cmd *cobra.Command, pkgType opc.PackageType, baseline, candidate opc.PackageSession, baselinePath, candidatePath string) (*FamilyDiffResult, error, error) {
	result := &FamilyDiffResult{SchemaVersion: FamilyDiffSchemaVersion, Type: pkgType.String()}

	switch pkgType {
	case opc.PackageTypePPTX:
		semantic, err := semanticDiffFn(baseline, candidate)
		if err != nil {
			return nil, nil, NewCLIErrorf(ExitUnexpected, "failed to compute semantic diff: %v", err)
		}
		result.Semantic = semantic
		deferred := runFamilyVisualDiff(cmd, result, baselinePath, candidatePath)
		return result, deferred, nil
	case opc.PackageTypeXLSX:
		semantic, err := xlsxSemanticDiffFn(baseline, candidate)
		if err != nil {
			return nil, nil, NewCLIErrorf(ExitUnexpected, "failed to compute semantic diff: %v", err)
		}
		result.Semantic = semantic
		return result, nil, nil
	case opc.PackageTypeDOCX:
		semantic, err := docxSemanticDiffFn(baseline, candidate)
		if err != nil {
			return nil, nil, NewCLIErrorf(ExitUnexpected, "failed to compute semantic diff: %v", err)
		}
		result.Semantic = semantic
		return result, nil, nil
	default:
		return nil, nil, NewCLIErrorf(ExitUnsupportedType, "unsupported package type for diff: %s", pkgType)
	}
}

// runFamilyVisualDiff runs a PPTX visual diff when --render is set. It reuses
// the same shared diffRender/diffThreshold state and runVisualDiff helper as the
// existing `pptx diff` command by populating those vars before delegating.
func runFamilyVisualDiff(cmd *cobra.Command, result *FamilyDiffResult, baselinePath, candidatePath string) error {
	if !familyDiffRender {
		result.Visual = &VisualResult{Enabled: false, Status: "disabled"}
		return nil
	}

	// runVisualDiff reads the package-level diffThreshold var.
	diffThreshold = familyDiffThreshold
	visual, err := runVisualDiff(baselinePath, candidatePath, cmd)
	if err != nil {
		var missing *pkgrender.MissingDependencyError
		var toolFailure *pkgrender.ToolFailureError
		if errors.As(err, &missing) || errors.As(err, &toolFailure) {
			result.Visual = &VisualResult{Enabled: true, Status: "unavailable", Threshold: familyDiffThreshold}
			if GetGlobalConfig(cmd).Strict {
				return mapRenderError(err)
			}
			return NewCLIError(ExitPartialSuccess, err.Error())
		}
		// Hard failure surfaces as a deferred unexpected error so the JSON still emits.
		result.Visual = &VisualResult{Enabled: true, Status: "error", Threshold: familyDiffThreshold}
		return NewCLIErrorf(ExitUnexpected, "failed to compute visual diff: %v", err)
	}
	result.Visual = visual
	if !visual.Pass {
		return DiffThresholdError(fmt.Sprintf("visual difference exceeded threshold %.4f", familyDiffThreshold))
	}
	return nil
}

// familyDiffChangeCount totals the semantic changes across all diff categories
// for the detected family.
func familyDiffChangeCount(semantic interface{}) int {
	switch sem := semantic.(type) {
	case *pptxdiff.Report:
		return len(sem.LayoutDiffs) + len(sem.TextDiffs) + len(sem.ImageDiffs) + len(sem.FormatDiffs) + len(sem.GeometryDiffs)
	case *xlsxdiff.Report:
		return len(sem.Sheets) + len(sem.CellDiffs) + len(sem.DefinedNameDiffs) + len(sem.TableDiffs)
	case *docxdiff.Report:
		return len(sem.Blocks)
	default:
		return 0
	}
}

func outputFamilyDiffText(cmd *cobra.Command, result *FamilyDiffResult) error {
	var text string
	switch sem := result.Semantic.(type) {
	case *pptxdiff.Report:
		text = fmt.Sprintf("Type: pptx\nChanged slides: %v\nText diffs: %d\nImage diffs: %d\n", sem.ChangedSlides, len(sem.TextDiffs), len(sem.ImageDiffs))
	case *xlsxdiff.Report:
		text = fmt.Sprintf("Type: xlsx\nChanged sheets: %v\nCell diffs: %d\nDefined-name diffs: %d\nTable diffs: %d\n", sem.ChangedSheets, len(sem.CellDiffs), len(sem.DefinedNameDiffs), len(sem.TableDiffs))
	case *docxdiff.Report:
		text = fmt.Sprintf("Type: docx\nChanged blocks: %v\nBlock diffs: %d\n", sem.ChangedBlocks, len(sem.Blocks))
	default:
		text = fmt.Sprintf("Type: %s\n", result.Type)
	}
	if result.Visual != nil && result.Visual.Enabled {
		text += fmt.Sprintf("Visual status: %s\n", result.Visual.Status)
	}
	return writeGlobalOutput(cmd, []byte(text))
}

func init() {
	familyDiffCmd.Flags().BoolVar(&familyDiffRender, "render", false, "enable visual diff via rendered slide images (PPTX only)")
	familyDiffCmd.Flags().Float64Var(&familyDiffThreshold, "threshold", 0.01, "visual diff threshold (PPTX --render)")
	familyDiffCmd.Flags().String("out", "", "output directory for visual diff artifacts (PPTX --render)")
	GetRootCmd().AddCommand(familyDiffCmd)
}
