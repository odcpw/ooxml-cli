// Package diff computes a deterministic semantic diff between two DOCX
// documents: block membership, paragraph text/style changes, and table cell
// changes. It reuses the existing extract reader so the diff reflects the same
// block model the rest of the CLI exposes.
package diff

import (
	"fmt"
	"sort"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/docx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// SchemaVersion pins the DOCX semantic diff contract.
const SchemaVersion = "1.0"

// Report is the structured DOCX semantic diff result. All slices are sorted for
// deterministic output.
type Report struct {
	SchemaVersion   string      `json:"schemaVersion"`
	BlockCountA     int         `json:"blockCountA"`
	BlockCountB     int         `json:"blockCountB"`
	BlockCountEqual bool        `json:"blockCountEqual"`
	ChangedBlocks   []int       `json:"changedBlocks"`
	Blocks          []BlockDiff `json:"blocks"`
}

// BlockDiff records a per-block change. Index matches the block index reported
// by `docx text` / addressable via `docx blocks --block` (1-based), so callers
// can act on the reported block directly.
type BlockDiff struct {
	Index    int    `json:"index"`
	Kind     string `json:"kind"`
	Property string `json:"property"` // "presence", "text", "style", "table"
	Change   string `json:"change"`   // "added", "removed", or "modified"
	Before   string `json:"before,omitempty"`
	After    string `json:"after,omitempty"`
}

// SemanticDiff compares two DOCX packages without rendering.
func SemanticDiff(a, b opc.PackageSession) (*Report, error) {
	if a == nil || b == nil {
		return nil, fmt.Errorf("semantic diff requires two package sessions")
	}

	blocksA, err := readBlocks(a)
	if err != nil {
		return nil, fmt.Errorf("failed to read baseline document: %w", err)
	}
	blocksB, err := readBlocks(b)
	if err != nil {
		return nil, fmt.Errorf("failed to read candidate document: %w", err)
	}

	report := &Report{
		SchemaVersion:   SchemaVersion,
		BlockCountA:     len(blocksA),
		BlockCountB:     len(blocksB),
		BlockCountEqual: len(blocksA) == len(blocksB),
		ChangedBlocks:   []int{},
		Blocks:          []BlockDiff{},
	}

	changed := map[int]struct{}{}
	// Align blocks by content signature (LCS) rather than position, so a block
	// inserted or removed near the top does not misreport every later block as
	// changed. Signature-identical blocks are treated as unchanged; the gaps
	// between them are reconciled into modified/added/removed.
	for _, diff := range alignBlocks(blocksA, blocksB) {
		report.Blocks = append(report.Blocks, diff)
		changed[diff.Index] = struct{}{}
	}
	// Deterministic ordering: by block index, then property.
	sort.SliceStable(report.Blocks, func(i, j int) bool {
		if report.Blocks[i].Index != report.Blocks[j].Index {
			return report.Blocks[i].Index < report.Blocks[j].Index
		}
		return report.Blocks[i].Property < report.Blocks[j].Property
	})

	for idx := range changed {
		report.ChangedBlocks = append(report.ChangedBlocks, idx)
	}
	sort.Ints(report.ChangedBlocks)
	return report, nil
}

// blockSignature is a content fingerprint used to align blocks across the two
// documents (identity, not position).
func blockSignature(b model.Block) string {
	return string(b.Kind) + "\x00" + b.Style + "\x00" + b.Text + "\x00" + tableShape(b.Table)
}

// alignBlocks diffs two block sequences using an LCS over signatures, then
// reconciles the non-matching gaps into modified/added/removed diffs.
func alignBlocks(a, b []model.Block) []BlockDiff {
	sigA := make([]string, len(a))
	for i := range a {
		sigA[i] = blockSignature(a[i])
	}
	sigB := make([]string, len(b))
	for j := range b {
		sigB[j] = blockSignature(b[j])
	}

	var diffs []BlockDiff
	ia, ib := 0, 0
	emitGap := func(ga, gb []model.Block) {
		n := min(len(ga), len(gb))
		for k := 0; k < n; k++ {
			if ga[k].Kind == gb[k].Kind {
				diffs = append(diffs, compareBlock(ga[k], gb[k])...)
			} else {
				diffs = append(diffs, removedBlock(ga[k]), addedBlock(gb[k]))
			}
		}
		for k := n; k < len(ga); k++ {
			diffs = append(diffs, removedBlock(ga[k]))
		}
		for k := n; k < len(gb); k++ {
			diffs = append(diffs, addedBlock(gb[k]))
		}
	}
	for _, p := range lcsPairs(sigA, sigB) {
		emitGap(a[ia:p.i], b[ib:p.j])
		ia, ib = p.i+1, p.j+1 // the matched pair is signature-identical: unchanged
	}
	emitGap(a[ia:], b[ib:])
	return diffs
}

type lcsPair struct{ i, j int }

// lcsPairs returns the matched index pairs of a longest common subsequence of
// the two signature slices, in increasing order.
func lcsPairs(a, b []string) []lcsPair {
	n, m := len(a), len(b)
	table := make([][]int, n+1)
	for i := range table {
		table[i] = make([]int, m+1)
	}
	for i := n - 1; i >= 0; i-- {
		for j := m - 1; j >= 0; j-- {
			if a[i] == b[j] {
				table[i][j] = table[i+1][j+1] + 1
			} else if table[i+1][j] >= table[i][j+1] {
				table[i][j] = table[i+1][j]
			} else {
				table[i][j] = table[i][j+1]
			}
		}
	}
	var pairs []lcsPair
	for i, j := 0, 0; i < n && j < m; {
		switch {
		case a[i] == b[j]:
			pairs = append(pairs, lcsPair{i, j})
			i++
			j++
		case table[i+1][j] >= table[i][j+1]:
			i++
		default:
			j++
		}
	}
	return pairs
}

func removedBlock(b model.Block) BlockDiff {
	return BlockDiff{Index: b.Index, Kind: string(b.Kind), Property: "presence", Change: "removed", Before: b.Text}
}

func addedBlock(b model.Block) BlockDiff {
	return BlockDiff{Index: b.Index, Kind: string(b.Kind), Property: "presence", Change: "added", After: b.Text}
}

func readBlocks(session opc.PackageSession) ([]model.Block, error) {
	documentURI, err := inspect.FindMainDocumentPart(session)
	if err != nil {
		return nil, fmt.Errorf("failed to find main document: %w", err)
	}
	extracted, err := extract.ExtractText(&extract.ExtractTextRequest{Session: session, DocumentURI: documentURI})
	if err != nil {
		return nil, err
	}
	return extracted.Blocks, nil
}

func compareBlock(before, after model.Block) []BlockDiff {
	// Report the candidate (after) index so a caller can act on the block in the
	// file they now hold; for equal-length docs this equals before.Index.
	idx := after.Index
	diffs := make([]BlockDiff, 0)
	if before.Kind != after.Kind {
		diffs = append(diffs, BlockDiff{Index: idx, Kind: string(after.Kind), Property: "kind", Change: "modified", Before: string(before.Kind), After: string(after.Kind)})
		return diffs
	}
	if before.Text != after.Text {
		diffs = append(diffs, BlockDiff{Index: idx, Kind: string(before.Kind), Property: "text", Change: "modified", Before: before.Text, After: after.Text})
	}
	if before.Kind == model.BlockKindParagraph && before.Style != after.Style {
		diffs = append(diffs, BlockDiff{Index: idx, Kind: string(before.Kind), Property: "style", Change: "modified", Before: before.Style, After: after.Style})
	}
	if before.Kind == model.BlockKindTable {
		beforeShape := tableShape(before.Table)
		afterShape := tableShape(after.Table)
		if beforeShape != afterShape {
			diffs = append(diffs, BlockDiff{Index: idx, Kind: string(before.Kind), Property: "table", Change: "modified", Before: beforeShape, After: afterShape})
		}
	}
	return diffs
}

func tableShape(table *model.Table) string {
	if table == nil {
		return ""
	}
	parts := make([]string, 0, len(table.Rows))
	for _, row := range table.Rows {
		parts = append(parts, fmt.Sprintf("%d", len(row.Cells)))
	}
	return fmt.Sprintf("rows=%d cols=[%s]", len(table.Rows), strings.Join(parts, ","))
}
