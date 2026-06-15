package serve

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os/exec"
	"sort"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/apply"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/validate"
)

// ReadCommandDeniedError reports an inspect command that would not be read-only
// or artifact-free inside a held session.
type ReadCommandDeniedError struct {
	Command string
	Reason  string
}

func (e *ReadCommandDeniedError) Error() string {
	if e.Reason == "" {
		return fmt.Sprintf("inspect command is not allowed: %q", e.Command)
	}
	return fmt.Sprintf("inspect command is not allowed: %q (%s)", e.Command, e.Reason)
}

// multiSourcePrefixes are the command-word prefixes of ops that consume a second
// source package (clone/import/merge). The working-copy MVP engine cannot run
// these because they need a second open package, so Op rejects them with a clear
// MultiSourceError rather than silently mis-applying against the wrong package.
var multiSourcePrefixes = []string{
	"pptx slides merge",
	"pptx slides import-slide",
	"pptx layouts import",
	"pptx masters import",
}

// IsMultiSource reports whether command is a multi-source op unsupported by the
// working-copy engine.
func IsMultiSource(command string) bool {
	c := strings.Join(strings.Fields(command), " ")
	for _, p := range multiSourcePrefixes {
		if c == p || strings.HasPrefix(c, p+" ") {
			return true
		}
	}
	return false
}

// runReadCommand dispatches a read-only command as a subprocess against the
// working file and returns its JSON stdout. It mirrors apply.RunOp's subprocess
// discipline but uses a read argv (no --out / --no-validate, just --json).
func runReadCommand(self, working, command string, args map[string]apply.Arg) (json.RawMessage, error) {
	if err := validateReadCommand(command, args); err != nil {
		return nil, err
	}
	argv := buildReadArgv(command, working, args)
	cmd := exec.Command(self, argv...)
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		return nil, &apply.OpError{
			Command: command,
			Stderr:  stderr.String(),
			Err:     err,
		}
	}
	out := bytes.TrimSpace(stdout.Bytes())
	if len(out) == 0 || !json.Valid(out) {
		return nil, fmt.Errorf("inspect command %q did not return JSON", command)
	}
	return json.RawMessage(out), nil
}

var readCommandArtifactFlags = map[string]string{
	"apply":       "find --apply mutates through the apply engine",
	"backup":      "backup writes a file outside inspect",
	"data-out":    "data-out writes an exported artifact",
	"dry-run":     "dry-run belongs to mutation commands",
	"in-place":    "in-place would write the original file",
	"keep-temp":   "keep-temp can leave artifacts outside the session",
	"no-validate": "no-validate belongs to mutation commands",
	"ops":         "ops drives the apply mutation engine",
	"out":         "out writes an output package",
	"out-dir":     "out-dir writes extracted artifacts",
	"output":      "output writes a file outside inspect",
}

var readCommandArtifactPrefixes = map[string]string{
	"pptx extract images": "extract images writes image files and a manifest even when --out is omitted",
	"pptx extract xml":    "extract xml writes raw XML files to --out and is not a session inspect read",
	"pptx render":         "render writes PDF/image artifacts to --out and should be run after commit on a real output path",
	"vba extract":         "vba extract writes .bas/.cls source files to --out-dir and is not a session inspect read",
	"vba extract-bin":     "vba extract-bin writes vbaProject.bin to --out and is not a session inspect read",
	"vba inspect-bin":     "inspect-bin reads a standalone vbaProject.bin, not the session working package",
}

var readCommandSessionIncompatiblePrefixes = map[string]string{
	"capabilities": "capabilities is a session-independent discovery command; use the serve capabilities method or MCP resource://capabilities",
	"diff":         "diff needs both baseline and candidate packages; use ooxml diff after committing or use verify with a baseline",
	"pptx diff":    "diff needs both baseline and candidate packages; use ooxml diff after committing or use verify with a baseline",
	"render":       "render writes visual artifacts and should be run after commit on a real output path",
	"verify":       "verify may diff/render against an external baseline; run it after committing on real output paths",
}

var readCommandMutationWords = map[string]bool{
	"add":                   true,
	"add-module":            true,
	"add-placeholder":       true,
	"add-textbox":           true,
	"add-column-filter":     true,
	"append":                true,
	"append-records":        true,
	"append-rows":           true,
	"apply":                 true,
	"attach":                true,
	"clear":                 true,
	"clear-autofilter":      true,
	"clear-cell":            true,
	"clear-column-filter":   true,
	"clear-sort":            true,
	"clone":                 true,
	"clone-slide":           true,
	"compile":               true,
	"convert-type":          true,
	"copy-style":            true,
	"create":                true,
	"delete":                true,
	"delete-col":            true,
	"delete-row":            true,
	"delete-shape":          true,
	"edit":                  true,
	"insert":                true,
	"insert-after":          true,
	"insert-col":            true,
	"insert-row":            true,
	"import":                true,
	"import-slide":          true,
	"merge":                 true,
	"move":                  true,
	"new-slide-from-layout": true,
	"place":                 true,
	"prune":                 true,
	"prune-stale":           true,
	"remove":                true,
	"remove-module":         true,
	"rename":                true,
	"reorder":               true,
	"replace":               true,
	"replace-module":        true,
	"save":                  true,
	"set":                   true,
	"set-autofilter":        true,
	"set-axis":              true,
	"set-batch":             true,
	"set-bounds":            true,
	"set-cell":              true,
	"set-chart-area-fill":   true,
	"set-column-format":     true,
	"set-format":            true,
	"set-legend":            true,
	"set-plot-area-fill":    true,
	"set-result":            true,
	"set-series-style":      true,
	"set-sort":              true,
	"set-style":             true,
	"set-text":              true,
	"set-title":             true,
	"sync":                  true,
	"update":                true,
	"update-data":           true,
	"update-from-xlsx":      true,
	"update-source":         true,
}

// validateReadCommand keeps inspect strictly read-only and artifact-free. It
// preserves ordinary list/show/export reads, but blocks known mutators and any
// flag that can publish output outside the held session.
func validateReadCommand(command string, args map[string]apply.Arg) error {
	normalized := strings.Join(strings.Fields(command), " ")
	if normalized == "" {
		return &ReadCommandDeniedError{Command: command, Reason: "missing command"}
	}
	for _, word := range strings.Fields(normalized) {
		if strings.HasPrefix(word, "-") {
			return &ReadCommandDeniedError{Command: normalized, Reason: fmt.Sprintf("command must contain only command words; put flag %q in args instead", word)}
		}
	}
	for prefix, reason := range readCommandArtifactPrefixes {
		if normalized == prefix || strings.HasPrefix(normalized, prefix+" ") {
			return &ReadCommandDeniedError{Command: normalized, Reason: reason}
		}
	}
	for prefix, reason := range readCommandSessionIncompatiblePrefixes {
		if normalized == prefix || strings.HasPrefix(normalized, prefix+" ") {
			return &ReadCommandDeniedError{Command: normalized, Reason: reason}
		}
	}
	for k := range args {
		flag := apply.NormalizeArgKeyName(k)
		if flag == "" {
			return &ReadCommandDeniedError{Command: normalized, Reason: fmt.Sprintf("arg key %q must name a flag", k)}
		}
		if strings.Contains(flag, "=") {
			return &ReadCommandDeniedError{Command: normalized, Reason: fmt.Sprintf("arg key %q must be a flag name without '='; put the flag value in the JSON value instead", k)}
		}
		if reason, ok := readCommandArtifactFlags[flag]; ok {
			return &ReadCommandDeniedError{Command: normalized, Reason: fmt.Sprintf("--%s is not allowed in inspect: %s", flag, reason)}
		}
	}
	for _, word := range strings.Fields(normalized) {
		if readCommandMutationWords[word] {
			return &ReadCommandDeniedError{Command: normalized, Reason: fmt.Sprintf("%q is a mutation command word; use op/apply instead", word)}
		}
	}
	return nil
}

// buildReadArgv builds the subprocess argv for a read-only command:
//
//	--json <command words> <file> --<k> <v> ...
//
// Arg keys are sorted for determinism. --json is a global flag so it precedes the
// command words.
func buildReadArgv(command, file string, args map[string]apply.Arg) []string {
	argv := []string{"--json"}
	argv = append(argv, strings.Fields(command)...)
	argv = append(argv, file)

	keys := make([]string, 0, len(args))
	for k := range args {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	for _, k := range keys {
		argv = apply.AppendFlagArg(argv, k, args[k])
	}
	return argv
}

// validatePackage runs the standard package validation against an open session.
func validatePackage(pkg opc.PackageSession) ([]result.Diagnostic, error) {
	return validate.ValidatePackage(pkg)
}
