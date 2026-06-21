// Package apply implements the all-or-nothing `ooxml apply` pipeline.
//
// It applies an ordered array of {command, args} mutation operations to a
// single OOXML file using a rolling-temp-file pipeline. Each operation is
// re-dispatched as a SUBPROCESS of the ooxml binary itself (Path B), which
// gives clean isolation with no in-process global-flag leakage between the
// many existing mutation commands. The user's output target is only written
// at the very end, after a single final validation pass.
package apply

import "encoding/json"

// SchemaVersion identifies the apply result contract. Bump on breaking changes.
const SchemaVersion = 1

// Operation is a single entry in ops.json: a command path plus its named
// arguments. For example:
//
//	{"command": "xlsx cells set", "args": {"sheet": "1", "cell": "A1", "value": "x"}}
type Operation struct {
	Command string         `json:"command"`
	Args    map[string]Arg `json:"args"`
}

// Arg is a single argument value decoded from ops.json. ops.json values may be
// strings, numbers, or booleans; Arg stringifies them deterministically for the
// subprocess argv.
type Arg struct {
	raw json.RawMessage
}

// AppliedOp is the readback record for one successfully applied operation.
type AppliedOp struct {
	Index    int             `json:"index"`
	Command  string          `json:"command"`
	Readback json.RawMessage `json:"readback,omitempty"`
}

// Result is the JSON contract returned by `ooxml apply`.
type Result struct {
	SchemaVersion   int         `json:"schemaVersion"`
	File            string      `json:"file"`
	OpsCount        int         `json:"opsCount"`
	Applied         []AppliedOp `json:"applied"`
	Output          string      `json:"output,omitempty"`
	DryRun          bool        `json:"dryRun"`
	ValidateCommand string      `json:"validateCommand,omitempty"`
}

// PlanEntry is one resolved op in a --dry-run plan: the exact argv that would
// be executed (with the rolling temp paths shown as placeholders).
type PlanEntry struct {
	Index   int      `json:"index"`
	Command string   `json:"command"`
	Argv    []string `json:"argv"`
}

// Plan is the JSON contract returned by `ooxml apply --dry-run`.
type Plan struct {
	SchemaVersion int         `json:"schemaVersion"`
	File          string      `json:"file"`
	OpsCount      int         `json:"opsCount"`
	DryRun        bool        `json:"dryRun"`
	Plan          []PlanEntry `json:"plan"`
}
