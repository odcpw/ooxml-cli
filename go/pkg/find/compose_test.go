package find

import (
	"encoding/json"
	"testing"
)

// hitWithOp builds a hit carrying a structured op, mirroring what the searchers
// produce, so HitsToOps can be exercised in isolation.
func hitWithOp(index int, command, replaceKey string, args []opArg) Hit {
	op := opSpec{Command: command, Args: args, ReplaceKey: replaceKey}
	return Hit{Index: index, MutationCommand: op.humanCommand(), op: op}
}

func TestHitsToOpsBasic(t *testing.T) {
	hits := []Hit{
		hitWithOp(0, "xlsx cells set", "value", []opArg{
			{"sheet", "Types"}, {"cell", "A1"}, {"value", newOpPlaceholder},
		}),
	}
	res, err := HitsToOps(hits, "NEWVAL")
	if err != nil {
		t.Fatalf("HitsToOps: %v", err)
	}
	if len(res.Ops) != 1 || len(res.SkippedHitIndices) != 0 {
		t.Fatalf("unexpected result: %+v", res)
	}
	op := res.Ops[0]
	if op.Command != "xlsx cells set" {
		t.Errorf("command = %q", op.Command)
	}
	if op.Args["value"] != "NEWVAL" {
		t.Errorf("value = %q, want NEWVAL", op.Args["value"])
	}
	if op.Args["sheet"] != "Types" || op.Args["cell"] != "A1" {
		t.Errorf("structured args lost: %+v", op.Args)
	}
}

// TestHitsToOpsDedupsIdenticalOps pins Finding 2: two hits that yield the SAME
// op (same command + every arg key/value, e.g. the same substring twice in one
// shape) collapse to a single emitted op, with the later hit recorded as a
// duplicate. This is what stops a recurring substring from emitting a second,
// identical op that would match zero under --apply and abort the batch.
func TestHitsToOpsDedupsIdenticalOps(t *testing.T) {
	mk := func(idx int) Hit {
		op := opSpec{
			Command:    "pptx replace text-occurrences",
			Args:       []opArg{{"match-text", "r"}, {"new-text", newOpPlaceholder}, {"for-shape", "H:pptx/s:257/shape:n:2"}},
			ReplaceKey: "new-text",
			HandleKey:  "for-shape",
			Handle:     "H:pptx/s:257/shape:n:2",
		}
		return Hit{Index: idx, op: op}
	}
	res, err := HitsToOps([]Hit{mk(0), mk(1)}, "Q")
	if err != nil {
		t.Fatalf("HitsToOps: %v", err)
	}
	if len(res.Ops) != 1 {
		t.Fatalf("identical ops must collapse to 1, got %d", len(res.Ops))
	}
	if len(res.DuplicateHitIndices) != 1 || res.DuplicateHitIndices[0] != 1 {
		t.Fatalf("want duplicate hit index [1], got %v", res.DuplicateHitIndices)
	}
}

// TestHitsToOpsDistinctShapeOpsNotDeduped pins that Finding 1's shape-scoping
// keeps two hits in DIFFERENT shapes as DIFFERENT ops (distinct handles), so they
// are not collapsed.
func TestHitsToOpsDistinctShapeOpsNotDeduped(t *testing.T) {
	mk := func(idx int, handle string) Hit {
		op := opSpec{
			Command:    "pptx replace text-occurrences",
			Args:       []opArg{{"match-text", "x"}, {"new-text", newOpPlaceholder}, {"for-shape", handle}},
			ReplaceKey: "new-text",
			HandleKey:  "for-shape",
			Handle:     handle,
		}
		return Hit{Index: idx, op: op}
	}
	res, err := HitsToOps([]Hit{mk(0, "H:pptx/s:257/shape:n:2"), mk(1, "H:pptx/s:257/shape:n:3")}, "Q")
	if err != nil {
		t.Fatalf("HitsToOps: %v", err)
	}
	if len(res.Ops) != 2 {
		t.Fatalf("distinct shape ops must NOT collapse, got %d", len(res.Ops))
	}
	if len(res.DuplicateHitIndices) != 0 {
		t.Fatalf("no duplicates expected, got %v", res.DuplicateHitIndices)
	}
}

func TestHitsToOpsPlaceholderWhenNoReplace(t *testing.T) {
	hits := []Hit{
		hitWithOp(0, "docx replace", "replace", []opArg{
			{"find", "Old"}, {"replace", newOpPlaceholder},
		}),
	}
	res, err := HitsToOps(hits, "")
	if err != nil {
		t.Fatalf("HitsToOps: %v", err)
	}
	if res.Ops[0].Args["replace"] != newOpPlaceholder {
		t.Errorf("want placeholder, got %q", res.Ops[0].Args["replace"])
	}
}

func TestHitsToOpsSubstitutesReplacementInsideTemplate(t *testing.T) {
	hit := hitWithOp(0, "docx paragraphs set", "text", []opArg{
		{"handle", "H:docx/pt:doc/para:m:1A2B3C4D"},
		{"text", "First " + newOpPlaceholder + " paragraph"},
	})
	hit.op.HandleKey = "handle"
	hit.op.Handle = "H:docx/pt:doc/para:m:1A2B3C4D"
	res, err := HitsToOps([]Hit{hit}, "updated")
	if err != nil {
		t.Fatalf("HitsToOps: %v", err)
	}
	if got := res.Ops[0].Args["text"]; got != "First updated paragraph" {
		t.Fatalf("templated text = %q, want full paragraph with replacement", got)
	}
	if got := res.Ops[0].Args["handle"]; got != "H:docx/pt:doc/para:m:1A2B3C4D" {
		t.Fatalf("handle = %q", got)
	}
}

func TestHitsToOpsUsesCollisionFreeReplaceToken(t *testing.T) {
	hit := hitWithOp(0, "docx paragraphs set", "text", []opArg{
		{"handle", "H:docx/pt:doc/para:m:1A2B3C4D"},
		{"text", "Keep <NEW>; change <OOXML_NEW_1>"},
	})
	hit.op.ReplaceToken = "<OOXML_NEW_1>"
	res, err := HitsToOps([]Hit{hit}, "updated")
	if err != nil {
		t.Fatalf("HitsToOps: %v", err)
	}
	if got := res.Ops[0].Args["text"]; got != "Keep <NEW>; change updated" {
		t.Fatalf("templated text = %q, want literal <NEW> preserved", got)
	}
}

func TestHitsToOpsSkipsHitsWithoutOp(t *testing.T) {
	hits := []Hit{
		hitWithOp(0, "xlsx cells set", "value", []opArg{{"sheet", "S"}, {"cell", "A1"}, {"value", newOpPlaceholder}}),
		{Index: 1}, // no op: simulates a speaker-notes hit
		hitWithOp(2, "docx replace", "replace", []opArg{{"find", "X"}, {"replace", newOpPlaceholder}}),
	}
	res, err := HitsToOps(hits, "Y")
	if err != nil {
		t.Fatalf("HitsToOps: %v", err)
	}
	if len(res.Ops) != 2 {
		t.Fatalf("want 2 ops, got %d", len(res.Ops))
	}
	if len(res.SkippedHitIndices) != 1 || res.SkippedHitIndices[0] != 1 {
		t.Fatalf("want skipped [1], got %v", res.SkippedHitIndices)
	}
}

func TestHitsToOpsEmpty(t *testing.T) {
	res, err := HitsToOps(nil, "x")
	if err != nil {
		t.Fatalf("HitsToOps: %v", err)
	}
	if len(res.Ops) != 0 || len(res.SkippedHitIndices) != 0 {
		t.Fatalf("want empty result, got %+v", res)
	}
}

// TestHitsToOpsEmitsHandleInTargetArg confirms that when a hit carries a stable
// NATIVE-ID handle (here a pptx shape), HitsToOps writes the handle into the
// named target arg (overriding the positional selector) and does NOT flag the op
// as position-dependent — native-id handles are immune to structural shifts.
func TestHitsToOpsEmitsHandleInTargetArg(t *testing.T) {
	hit := Hit{
		Index:           0,
		MutationCommand: "x",
		op: opSpec{
			Command:    "pptx shapes set-text",
			Args:       []opArg{{"for-shape", "Title 1"}, {"new-text", newOpPlaceholder}},
			ReplaceKey: "new-text",
			HandleKey:  "for-shape",
			Handle:     "H:pptx/s:257/shape:n:2",
		},
	}
	res, err := HitsToOps([]Hit{hit}, "NEW")
	if err != nil {
		t.Fatalf("HitsToOps: %v", err)
	}
	if got := res.Ops[0].Args["for-shape"]; got != "H:pptx/s:257/shape:n:2" {
		t.Errorf("for-shape arg = %q, want the handle", got)
	}
	if got := res.Ops[0].Args["new-text"]; got != "NEW" {
		t.Errorf("new-text arg = %q, want NEW", got)
	}
	if len(res.PositionDependentHitIndices) != 0 {
		t.Errorf("native-id handle op should not be position-dependent, got %v", res.PositionDependentHitIndices)
	}
}

// TestHitsToOpsAddressPositionalCellHandleIsPositionDependent is the other half
// of the discriminator: an ADDRESS-POSITIONAL handle (an A1-tagged XLSX cell
// handle) is STILL substituted into the target arg (so it survives sheet
// reorder/rename), but it IS flagged position-dependent because its A1 address
// shifts under a row/column insert/delete. Mere handle presence must not be read
// as position-immunity.
func TestHitsToOpsAddressPositionalCellHandleIsPositionDependent(t *testing.T) {
	hit := Hit{
		Index:           0,
		MutationCommand: "x",
		op: opSpec{
			Command:    "xlsx cells set",
			Args:       []opArg{{"sheet", "Types"}, {"cell", "A1"}, {"value", newOpPlaceholder}},
			ReplaceKey: "value",
			HandleKey:  "cell",
			Handle:     "H:xlsx/ws:1/cell:a:A1",
		},
	}
	res, err := HitsToOps([]Hit{hit}, "NEW")
	if err != nil {
		t.Fatalf("HitsToOps: %v", err)
	}
	if got := res.Ops[0].Args["cell"]; got != "H:xlsx/ws:1/cell:a:A1" {
		t.Errorf("cell arg = %q, want the handle still substituted", got)
	}
	if len(res.PositionDependentHitIndices) != 1 || res.PositionDependentHitIndices[0] != 0 {
		t.Fatalf("address-positional cell handle must be position-dependent [0], got %v", res.PositionDependentHitIndices)
	}
}

// TestHitsToOpsPositionDependentWhenNoHandle confirms an op with a positional
// target and no handle is reported as position-dependent, while a
// PositionIndependent op (docx replace) is not.
func TestHitsToOpsPositionDependentWhenNoHandle(t *testing.T) {
	positional := Hit{Index: 0, MutationCommand: "x", op: opSpec{
		Command:    "xlsx cells set",
		Args:       []opArg{{"sheet", "S"}, {"cell", "A1"}, {"value", newOpPlaceholder}},
		ReplaceKey: "value",
		HandleKey:  "cell", // no Handle value -> falls back to positional
	}}
	global := Hit{Index: 1, MutationCommand: "y", op: opSpec{
		Command:             "docx replace",
		Args:                []opArg{{"find", "X"}, {"replace", newOpPlaceholder}},
		ReplaceKey:          "replace",
		PositionIndependent: true,
	}}
	res, err := HitsToOps([]Hit{positional, global}, "v")
	if err != nil {
		t.Fatalf("HitsToOps: %v", err)
	}
	if res.Ops[0].Args["cell"] != "A1" {
		t.Errorf("positional cell arg should stay A1, got %q", res.Ops[0].Args["cell"])
	}
	if len(res.PositionDependentHitIndices) != 1 || res.PositionDependentHitIndices[0] != 0 {
		t.Fatalf("want position-dependent [0] (docx replace excluded), got %v", res.PositionDependentHitIndices)
	}
}

// TestHitsToOpsMarshalsDeterministically confirms the emitted ops marshal to a
// stable bare JSON array (Go sorts map keys), which is the apply-compatible form.
func TestHitsToOpsMarshalsDeterministically(t *testing.T) {
	hits := []Hit{
		hitWithOp(0, "xlsx cells set", "value", []opArg{{"sheet", "S"}, {"cell", "A1"}, {"value", newOpPlaceholder}}),
	}
	res, _ := HitsToOps(hits, "v")
	a, _ := json.Marshal(res.Ops)
	b, _ := json.Marshal(res.Ops)
	if string(a) != string(b) {
		t.Fatalf("non-deterministic marshal: %s vs %s", a, b)
	}
	want := `[{"command":"xlsx cells set","args":{"cell":"A1","sheet":"S","value":"v"}}]`
	if string(a) != want {
		t.Fatalf("marshal = %s, want %s", a, want)
	}
}

// TestHumanCommandDerivesFromOp confirms the human command is rendered from the
// structured op in authoring order with the replacement placeholder unquoted.
func TestHumanCommandDerivesFromOp(t *testing.T) {
	op := opSpec{
		Command: "pptx replace text-occurrences",
		Args: []opArg{
			{"match-text", "Old Corp"},
			{"new-text", newOpPlaceholder},
			{"for-slides", "2"},
		},
		ReplaceKey: "new-text",
	}
	got := op.humanCommand()
	want := "ooxml --json pptx replace text-occurrences <file> --match-text 'Old Corp' --new-text <NEW> --for-slides 2 --out <OUT>"
	if got != want {
		t.Fatalf("humanCommand =\n  %q\nwant\n  %q", got, want)
	}
}
