package cli

import (
	"encoding/json"
	"path/filepath"
	"sort"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/apply"
	findpkg "github.com/ooxml-cli/ooxml-cli/pkg/find"
	pkgrender "github.com/ooxml-cli/ooxml-cli/pkg/render"
)

type pptxAgentWorkflowGolden struct {
	Workflow    string                        `json:"workflow"`
	InitialFind pptxAgentWorkflowFindGolden   `json:"initialFind"`
	Ops         []pptxAgentWorkflowOpGolden   `json:"ops"`
	Apply       pptxAgentWorkflowApplyGolden  `json:"apply"`
	PostFind    pptxAgentWorkflowPostGolden   `json:"postFind"`
	Verify      pptxAgentWorkflowVerifyGolden `json:"verify"`
}

type pptxAgentWorkflowFindGolden struct {
	PackageType     string `json:"packageType"`
	Query           string `json:"query"`
	TotalHits       int    `json:"totalHits"`
	FirstKind       string `json:"firstKind"`
	FirstHandle     string `json:"firstHandle"`
	PrimarySelector string `json:"primarySelector"`
	MatchedValue    string `json:"matchedValue"`
}

type pptxAgentWorkflowOpGolden struct {
	Command string                         `json:"command"`
	Args    []pptxAgentWorkflowOpArgGolden `json:"args"`
}

type pptxAgentWorkflowOpArgGolden struct {
	Name  string `json:"name"`
	Value string `json:"value"`
}

type pptxAgentWorkflowApplyGolden struct {
	SchemaVersion          int                             `json:"schemaVersion"`
	OpsCount               int                             `json:"opsCount"`
	DryRun                 bool                            `json:"dryRun"`
	OutputPublished        bool                            `json:"outputPublished"`
	ValidateCommandPresent bool                            `json:"validateCommandPresent"`
	AppliedCommands        []string                        `json:"appliedCommands"`
	Readback               pptxAgentWorkflowReadbackGolden `json:"readback"`
}

type pptxAgentWorkflowReadbackGolden struct {
	Operation          string                           `json:"operation"`
	MatchText          string                           `json:"matchText"`
	NewText            string                           `json:"newText"`
	DryRun             bool                             `json:"dryRun"`
	ChangedTargetCount int                              `json:"changedTargetCount"`
	ReplacementCount   int                              `json:"replacementCount"`
	ScopeSlides        []int                            `json:"scopeSlides"`
	Match              pptxAgentWorkflowMatchGolden     `json:"match"`
	ReadbackCommands   pptxAgentWorkflowCommandBooleans `json:"readbackCommands"`
}

type pptxAgentWorkflowMatchGolden struct {
	SlideNumber     int    `json:"slideNumber"`
	ShapeID         int    `json:"shapeId"`
	TargetKind      string `json:"targetKind"`
	PrimarySelector string `json:"primarySelector"`
	BeforeText      string `json:"beforeText"`
	AfterText       string `json:"afterText"`
	MatchCount      int    `json:"matchCount"`
}

type pptxAgentWorkflowCommandBooleans struct {
	Readback bool `json:"readback"`
	Slide    bool `json:"slide"`
	Validate bool `json:"validate"`
	Render   bool `json:"render"`
}

type pptxAgentWorkflowPostGolden struct {
	NewQueryHits    int    `json:"newQueryHits"`
	NewQueryHandle  string `json:"newQueryHandle"`
	OldQueryHits    int    `json:"oldQueryHits"`
	MatchedNewValue string `json:"matchedNewValue"`
}

type pptxAgentWorkflowVerifyGolden struct {
	SchemaVersion    string                          `json:"schemaVersion"`
	Type             string                          `json:"type"`
	Valid            bool                            `json:"valid"`
	ValidationStatus string                          `json:"validationStatus"`
	RenderStatus     string                          `json:"renderStatus"`
	SummaryChanges   int                             `json:"summaryChanges"`
	DiffType         string                          `json:"diffType"`
	ChangedSlides    []int                           `json:"changedSlides"`
	TextDiffCount    int                             `json:"textDiffCount"`
	FirstTextDiff    pptxAgentWorkflowTextDiffGolden `json:"firstTextDiff"`
}

type pptxAgentWorkflowTextDiffGolden struct {
	Slide     int    `json:"slide"`
	ShapeKey  string `json:"shapeKey"`
	ShapeName string `json:"shapeName"`
	Before    string `json:"before"`
	After     string `json:"after"`
}

// TestPPTXAgentWorkflowGolden freezes the practical PPTX agent loop:
// semantic find, apply-compatible op emission, atomic find->apply, changed-object
// readback, validation, and semantic diff against the baseline. This catches
// regressions in the workflow shape without snapshotting temp paths or rendered
// pixels.
func TestPPTXAgentWorkflowGolden(t *testing.T) {
	base := stagePPTXFixture(t, "title-content")
	outPath := filepath.Join(t.TempDir(), "edited.pptx")

	initial := decodeFindResult(t, mustRunFind(t, "--json", "find", "Content Slide", base))
	if initial.TotalHits != 1 {
		t.Fatalf("expected exactly one initial PPTX hit, got %d", initial.TotalHits)
	}

	opsJSON := mustRunFind(t, "--json", "find", "Content Slide", base, "--replace", "Renamed Content", "--to-ops")
	ops, err := apply.ParseOps([]byte(opsJSON))
	if err != nil {
		t.Fatalf("ParseOps: %v (%s)", err, opsJSON)
	}
	if len(ops) != 1 {
		t.Fatalf("expected one emitted op, got %d (%s)", len(ops), opsJSON)
	}

	origSelf := findSelfExecutable
	findSelfExecutable = func() (string, error) { return serveBinary, nil }
	t.Cleanup(func() { findSelfExecutable = origSelf })

	applyJSON, err := runFind(t, "--json", "find", "Content Slide", base, "--replace", "Renamed Content", "--apply", "--out", outPath)
	if err != nil {
		t.Fatalf("find --apply: %v\n%s", err, applyJSON)
	}
	var applied apply.Result
	if err := json.Unmarshal([]byte(applyJSON), &applied); err != nil {
		t.Fatalf("unmarshal apply result: %v (%s)", err, applyJSON)
	}
	if applied.Output != outPath {
		t.Fatalf("apply output = %q, want %q", applied.Output, outPath)
	}
	if len(applied.Applied) != 1 || applied.Applied[0].Readback == nil {
		t.Fatalf("unexpected applied ops: %+v", applied.Applied)
	}
	if strings.Contains(string(applied.Applied[0].Readback), ".ooxml-") {
		t.Fatalf("readback leaked scratch path: %s", string(applied.Applied[0].Readback))
	}

	var readback replaceTextOccurrencesResult
	if err := json.Unmarshal(applied.Applied[0].Readback, &readback); err != nil {
		t.Fatalf("unmarshal replace readback: %v (%s)", err, string(applied.Applied[0].Readback))
	}
	if readback.Output != outPath {
		t.Fatalf("readback output = %q, want %q", readback.Output, outPath)
	}
	if len(readback.Matches) != 1 {
		t.Fatalf("expected one replacement match, got %d", len(readback.Matches))
	}

	newFind := decodeFindResult(t, mustRunFind(t, "--json", "find", "Renamed Content", outPath))
	oldFind := decodeFindResult(t, mustRunFind(t, "--json", "find", "Content Slide", outPath))
	if newFind.TotalHits != 1 || oldFind.TotalHits != 0 {
		t.Fatalf("unexpected post-apply find counts: new=%d old=%d", newFind.TotalHits, oldFind.TotalHits)
	}

	verify := runPPTXAgentWorkflowVerify(t, outPath, base)
	if verify.SummaryChanges != 1 || verify.TextDiffCount != 1 {
		t.Fatalf("expected one semantic text change, got changes=%d textDiffs=%d", verify.SummaryChanges, verify.TextDiffCount)
	}

	actual := pptxAgentWorkflowGolden{
		Workflow:    "pptx-find-to-apply-verify",
		InitialFind: summarizePPTXAgentWorkflowFind(initial),
		Ops:         summarizePPTXAgentWorkflowOps(ops),
		Apply:       summarizePPTXAgentWorkflowApply(applied, readback, outPath),
		PostFind: pptxAgentWorkflowPostGolden{
			NewQueryHits:    newFind.TotalHits,
			NewQueryHandle:  newFind.Hits[0].Handle,
			OldQueryHits:    oldFind.TotalHits,
			MatchedNewValue: newFind.Hits[0].MatchedValue,
		},
		Verify: verify,
	}
	assertGoldenJSONValue(t, "pptx_agent_workflow_summary.json", actual)
}

func summarizePPTXAgentWorkflowFind(result findpkg.Result) pptxAgentWorkflowFindGolden {
	hit := result.Hits[0]
	return pptxAgentWorkflowFindGolden{
		PackageType:     result.PackageType,
		Query:           result.Query,
		TotalHits:       result.TotalHits,
		FirstKind:       string(hit.Kind),
		FirstHandle:     hit.Handle,
		PrimarySelector: hit.PrimarySelector,
		MatchedValue:    hit.MatchedValue,
	}
}

func summarizePPTXAgentWorkflowOps(ops []apply.Operation) []pptxAgentWorkflowOpGolden {
	out := make([]pptxAgentWorkflowOpGolden, 0, len(ops))
	for _, op := range ops {
		keys := make([]string, 0, len(op.Args))
		for key := range op.Args {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		args := make([]pptxAgentWorkflowOpArgGolden, 0, len(keys))
		for _, key := range keys {
			args = append(args, pptxAgentWorkflowOpArgGolden{Name: key, Value: op.Args[key].String()})
		}
		out = append(out, pptxAgentWorkflowOpGolden{Command: op.Command, Args: args})
	}
	return out
}

func summarizePPTXAgentWorkflowApply(applied apply.Result, readback replaceTextOccurrencesResult, outPath string) pptxAgentWorkflowApplyGolden {
	commands := make([]string, 0, len(applied.Applied))
	for _, op := range applied.Applied {
		commands = append(commands, op.Command)
	}
	match := readback.Matches[0]
	return pptxAgentWorkflowApplyGolden{
		SchemaVersion:          applied.SchemaVersion,
		OpsCount:               applied.OpsCount,
		DryRun:                 applied.DryRun,
		OutputPublished:        applied.Output == outPath && fileExists(outPath),
		ValidateCommandPresent: applied.ValidateCommand != "",
		AppliedCommands:        commands,
		Readback: pptxAgentWorkflowReadbackGolden{
			Operation:          readback.Operation,
			MatchText:          readback.MatchText,
			NewText:            readback.NewText,
			DryRun:             readback.DryRun,
			ChangedTargetCount: readback.Summary.ChangedTargetCount,
			ReplacementCount:   readback.Summary.ReplacementCount,
			ScopeSlides:        readback.Scope.Slides,
			Match: pptxAgentWorkflowMatchGolden{
				SlideNumber:     match.SlideNumber,
				ShapeID:         match.ShapeID,
				TargetKind:      match.TargetKind,
				PrimarySelector: match.PrimarySelector,
				BeforeText:      match.BeforeText,
				AfterText:       match.AfterText,
				MatchCount:      match.MatchCount,
			},
			ReadbackCommands: pptxAgentWorkflowCommandBooleans{
				Readback: match.ReadbackCommand != "",
				Slide:    match.SlideReadbackCommand != "",
				Validate: match.ValidateCommand != "",
				Render:   match.RenderCommand != "",
			},
		},
	}
}

func runPPTXAgentWorkflowVerify(t *testing.T, outPath, baseline string) pptxAgentWorkflowVerifyGolden {
	t.Helper()
	resetVerifyFlags()
	t.Cleanup(resetVerifyFlags)
	resetFamilyDiffFlags()

	origRender := renderToPDFFn
	renderToPDFFn = func(string, string) (string, error) {
		return "", &pkgrender.MissingDependencyError{Tool: "soffice"}
	}
	t.Cleanup(func() { renderToPDFFn = origRender })

	output, err := executeRootForXLSXTest(t, "--format", "json", "verify", outPath, "--baseline", baseline)
	if err != nil {
		t.Fatalf("verify: %v\n%s", err, output)
	}

	var parsed struct {
		SchemaVersion string `json:"schemaVersion"`
		Type          string `json:"type"`
		Valid         bool   `json:"valid"`
		Validation    struct {
			Status string `json:"status"`
		} `json:"validation"`
		Rendered struct {
			Status string `json:"status"`
		} `json:"rendered"`
		Summary struct {
			Changes int `json:"changes"`
		} `json:"summary"`
		Diff struct {
			Type     string `json:"type"`
			Semantic struct {
				ChangedSlides []int `json:"changedSlides"`
				TextDiffs     []struct {
					Slide     int    `json:"slide"`
					ShapeKey  string `json:"shapeKey"`
					ShapeName string `json:"shapeName"`
					Before    string `json:"before"`
					After     string `json:"after"`
				} `json:"textDiffs"`
			} `json:"semantic"`
		} `json:"diff"`
	}
	if err := json.Unmarshal([]byte(output), &parsed); err != nil {
		t.Fatalf("unmarshal verify result: %v (%s)", err, output)
	}
	if len(parsed.Diff.Semantic.TextDiffs) == 0 {
		t.Fatalf("verify diff had no text diffs: %s", output)
	}
	first := parsed.Diff.Semantic.TextDiffs[0]
	return pptxAgentWorkflowVerifyGolden{
		SchemaVersion:    parsed.SchemaVersion,
		Type:             parsed.Type,
		Valid:            parsed.Valid,
		ValidationStatus: parsed.Validation.Status,
		RenderStatus:     parsed.Rendered.Status,
		SummaryChanges:   parsed.Summary.Changes,
		DiffType:         parsed.Diff.Type,
		ChangedSlides:    parsed.Diff.Semantic.ChangedSlides,
		TextDiffCount:    len(parsed.Diff.Semantic.TextDiffs),
		FirstTextDiff: pptxAgentWorkflowTextDiffGolden{
			Slide:     first.Slide,
			ShapeKey:  first.ShapeKey,
			ShapeName: first.ShapeName,
			Before:    first.Before,
			After:     first.After,
		},
	}
}
