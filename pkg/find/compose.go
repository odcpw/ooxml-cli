package find

import (
	"fmt"
	"sort"
	"strings"

	xlsxhandle "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/handle"
)

// This file closes the find->apply composition loop. Each Hit that can be
// mutated carries a STRUCTURED operation spec (an ordered command + named
// arguments plus the single argument that receives the replacement value). The
// same spec is the single source of truth for two derived artifacts:
//
//   - the human-readable, paste-runnable Hit.MutationCommand shell string, and
//   - an apply-compatible operation ({command, args}) suitable for ooxml apply.
//
// Building the structured spec once and deriving both from it guarantees the
// argument keys match the real subcommand flag names (apply re-dispatches
// "--key value"), and that the replacement target stays correct per hit kind
// (e.g. xlsx value vs. formula, defined-name ref, pptx new-text, docx replace).

// opArg is one ordered argument of a structured operation. Order is preserved so
// the derived human command renders deterministically and matches the historical
// flag ordering for each command.
type opArg struct {
	Key   string
	Value string
}

// opSpec is the structured, apply-derivable mutation for a single hit. It is the
// in-memory source of truth attached to a Hit; it is never serialized (find's
// stable JSON contract is unchanged) and is read only in-package by HitsToOps.
//
// A zero opSpec (empty Command) marks a hit that has no semantic mutation
// command (e.g. PPTX speaker notes); such hits are skipped by HitsToOps.
type opSpec struct {
	// Command is the subcommand path, e.g. "xlsx cells set".
	Command string
	// Args are the named arguments in authoring (human) order.
	Args []opArg
	// ReplaceKey is the Args key whose value carries the replacement. In the
	// human command its value is the "<NEW>" placeholder; HitsToOps substitutes
	// the caller's --replace value here when building the apply op.
	ReplaceKey string
	// ReplaceToken is the literal marker to substitute inside the ReplaceKey arg.
	// It defaults to newOpPlaceholder. Paragraph-template ops may set a private,
	// collision-free token so literal "<NEW>" text in the document is preserved.
	ReplaceToken string

	// HandleKey, when non-empty AND Handle is non-empty, names the Args key that
	// is the op's TARGET selector (e.g. "cell", "name", "for-slides"). HitsToOps
	// overwrites that arg's value with Handle so the emitted apply op carries a
	// STABLE handle instead of a positional selector. This is what lets a
	// find->apply batch survive structural shifts caused by earlier ops: the
	// later op re-resolves its target by durable id, not by a position that an
	// earlier op may have moved. When Handle is empty (no stable handle exists
	// for this hit) the positional selector is kept and the op is
	// position-dependent.
	HandleKey string
	// Handle is the stable handle value for this op's target, or "" when none
	// exists. Set from the owning Hit.Handle so the op stays consistent with the
	// surfaced find handle.
	Handle string

	// PositionIndependent marks an op that targets its object WITHOUT a positional
	// selector and so is immune to structural shifts even with no handle (e.g.
	// `docx replace` is a global find/replace that re-matches the literal against
	// the evolving file each run). Such ops are never reported as
	// position-dependent. Ops with neither a handle nor this flag ARE
	// position-dependent.
	PositionIndependent bool
}

// newOpPlaceholder is the placeholder used for the replacement value in the
// human-readable mutation command and in --to-ops output when no --replace was
// supplied.
const newOpPlaceholder = "<NEW>"

// humanCommand renders the paste-runnable shell string for this op:
// "ooxml --json <command> <file> <--flag value...> --out <OUT>".
// Argument values are shell-quoted via shellArg. The ReplaceKey's value is the
// "<NEW>" placeholder so an agent fills in the real value before running.
func (s opSpec) humanCommand() string {
	if s.Command == "" {
		return ""
	}
	var b strings.Builder
	fmt.Fprintf(&b, "ooxml --json %s <file>", s.Command)
	for _, a := range s.Args {
		b.WriteString(" --")
		b.WriteString(a.Key)
		b.WriteString(" ")
		// The replacement placeholder is a literal token an agent fills in, not
		// data to quote; render it verbatim (matching find's historical output).
		// All other values are real package data and are shell-quoted.
		if a.Key == s.ReplaceKey && a.Value == newOpPlaceholder {
			b.WriteString(newOpPlaceholder)
		} else {
			b.WriteString(shellArg(a.Value))
		}
	}
	b.WriteString(" --out <OUT>")
	return b.String()
}

// Operation is an apply-compatible mutation: a command path plus its named
// arguments. It mirrors the shape ooxml apply consumes (a JSON array of
// {command, args}), so a slice of Operation marshals directly into a valid
// ops.json that the apply engine accepts.
type Operation struct {
	Command string            `json:"command"`
	Args    map[string]string `json:"args"`
}

// OpsResult is the structured output of converting hits into apply operations.
// Ops are emitted in hit (file) order, DE-DUPLICATED by full Operation identity
// (see HitsToOps). SkippedHitIndices lists the indices of hits that have no
// semantic mutation command (reported, not applied).
//
// PositionDependentHitIndices lists the indices of hits whose emitted op targets
// a POSITIONAL selector because no stable handle exists for them. Such ops can
// land on the wrong object if an EARLIER op in the same batch structurally shifts
// the target's position; callers may surface this as a diagnostic. Ops that carry
// a stable handle are absent from this list and survive structural shifts.
//
// DuplicateHitIndices lists the indices of hits whose emitted op was identical to
// an earlier hit's op and so was collapsed (not emitted again). It is a
// diagnostic only; the deduped Ops are the source of truth.
type OpsResult struct {
	Ops                         []Operation
	SkippedHitIndices           []int
	PositionDependentHitIndices []int
	DuplicateHitIndices         []int
}

// HitsToOps converts find hits into apply-compatible operations using each hit's
// STRUCTURED op spec (never by re-parsing the printed mutationCommand string).
//
// For each hit:
//   - hits with no mutation command (empty op spec) are recorded in
//     SkippedHitIndices and produce no operation;
//   - otherwise the op's ReplaceKey argument is set to newValue (or the "<NEW>"
//     placeholder when newValue is empty) and an Operation is appended UNLESS an
//     identical Operation (same command and every arg key/value) was already
//     emitted, in which case the hit is recorded in DuplicateHitIndices and no
//     duplicate op is appended.
//
// De-duplication is essential under --apply: when a substring recurs, distinct
// hits would otherwise emit IDENTICAL ops; the first op replaces every occurrence
// and a second identical op then matches zero, failing FailOnZero and aborting
// the whole batch. After shape-scoping, two hits in DIFFERENT shapes carry
// DIFFERENT (shape-scoped) ops and are NOT collapsed; only two hits in the SAME
// shape collapse, which loses nothing because text-occurrences replaces all
// matches in that shape in one pass.
//
// Emitted ops keep first-seen (file) order; because dedup may drop later hits,
// callers must correlate readback against the deduped Ops, not by raw hit index.
// The returned OpsResult is never nil.
func HitsToOps(hits []Hit, newValue string) (*OpsResult, error) {
	res := &OpsResult{Ops: []Operation{}, SkippedHitIndices: []int{}, PositionDependentHitIndices: []int{}, DuplicateHitIndices: []int{}}
	repl := newValue
	if repl == "" {
		repl = newOpPlaceholder
	}
	seen := map[string]struct{}{}
	for _, hit := range hits {
		spec := hit.op
		if spec.Command == "" {
			res.SkippedHitIndices = append(res.SkippedHitIndices, hit.Index)
			continue
		}
		replaceToken := spec.ReplaceToken
		if replaceToken == "" {
			replaceToken = newOpPlaceholder
		}
		// Prefer a STABLE handle for this op's target arg so the op survives
		// structural shifts caused by earlier ops in an apply batch. The handle is
		// the same one surfaced on the Hit; it overrides the positional selector in
		// the named target arg the mutate command already accepts. When no handle
		// exists the positional selector is kept and the op is position-dependent.
		useHandle := spec.HandleKey != "" && spec.Handle != ""
		args := make(map[string]string, len(spec.Args))
		for _, a := range spec.Args {
			v := a.Value
			if a.Key == spec.ReplaceKey {
				if strings.Contains(v, replaceToken) {
					v = strings.ReplaceAll(v, replaceToken, repl)
				} else {
					v = repl
				}
			}
			if useHandle && a.Key == spec.HandleKey {
				v = spec.Handle
			}
			args[a.Key] = v
		}
		op := Operation{Command: spec.Command, Args: args}

		// Collapse a hit whose op is byte-for-byte identical to one already emitted.
		// Identity = command + every arg key/value (after replacement + handle
		// substitution above), independent of Finding 1 as defense-in-depth.
		key := operationIdentity(op)
		if _, dup := seen[key]; dup {
			res.DuplicateHitIndices = append(res.DuplicateHitIndices, hit.Index)
			continue
		}
		seen[key] = struct{}{}

		// Position-dependence is decided by the TARGET's stability class, not by
		// mere handle presence. An op with no handle (and not globally
		// position-independent) is position-dependent. Critically, an op that DOES
		// carry a handle is STILL position-dependent when that handle is
		// address-positional (an A1-tagged cell/comment handle): such a handle
		// survives sheet reorder/rename but its A1 address shifts under a row/column
		// insert/delete, so an earlier batch op that shifts rows can move a populated
		// cell onto the address (the silent-wrong-target case). Native-id handles
		// (sheet/shape/paraId/defined-name) are genuinely immune and stay excluded.
		addressPositional := useHandle && xlsxhandle.IsAddressPositional(spec.Handle)
		if (!useHandle && !spec.PositionIndependent) || addressPositional {
			res.PositionDependentHitIndices = append(res.PositionDependentHitIndices, hit.Index)
		}
		res.Ops = append(res.Ops, op)
	}
	return res, nil
}

// operationIdentity returns a canonical, order-independent string identity for an
// Operation: its command plus every arg key/value. Arg keys are sorted so two ops
// with the same content but different map iteration order compare equal. Lengths
// are encoded to keep the key injective (so "a"+"bc" and "ab"+"c" differ).
func operationIdentity(op Operation) string {
	keys := make([]string, 0, len(op.Args))
	for k := range op.Args {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	var b strings.Builder
	fmt.Fprintf(&b, "%d:%s", len(op.Command), op.Command)
	for _, k := range keys {
		v := op.Args[k]
		fmt.Fprintf(&b, "|%d:%s=%d:%s", len(k), k, len(v), v)
	}
	return b.String()
}
