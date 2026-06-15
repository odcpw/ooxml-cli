package apply

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"strconv"
	"strings"
	"unicode"
)

// String renders an arg value as the string that will be passed on the
// subprocess command line. Numbers use json.Number formatting (so an integer
// 1 becomes "1", not "1.000000"); booleans become "true"/"false"; strings are
// used verbatim. Objects/arrays are rendered as their compact JSON.
func (a Arg) String() string {
	if len(a.raw) == 0 {
		return ""
	}
	dec := json.NewDecoder(bytes.NewReader(a.raw))
	dec.UseNumber()
	var v any
	if err := dec.Decode(&v); err != nil {
		return string(a.raw)
	}
	switch t := v.(type) {
	case string:
		return t
	case json.Number:
		return t.String()
	case bool:
		return strconv.FormatBool(t)
	case nil:
		return ""
	default:
		// Objects/arrays: pass through compact JSON.
		return string(bytes.TrimSpace(a.raw))
	}
}

// Bool returns the decoded boolean value when the raw arg is a JSON bool.
func (a Arg) Bool() (bool, bool) {
	if len(a.raw) == 0 {
		return false, false
	}
	dec := json.NewDecoder(bytes.NewReader(a.raw))
	var v bool
	if err := dec.Decode(&v); err != nil {
		return false, false
	}
	return v, true
}

// AppendFlagArg appends a deterministic CLI flag representation for one JSON
// arg. Bool flags must use --flag=true/false; pflag treats "--flag true" as a
// standalone bool plus an extra positional arg.
func AppendFlagArg(argv []string, key string, arg Arg) []string {
	name := "--" + NormalizeArgKeyName(key)
	if v, ok := arg.Bool(); ok {
		return append(argv, name+"="+strconv.FormatBool(v))
	}
	return append(argv, name, arg.String())
}

// NormalizeArgKeyName converts the JSON arg vocabulary used by apply/serve/MCP
// into the real pflag name. It keeps legacy kebab-case and --flag keys working
// while letting capabilities advertise JSON-friendly camelCase keys such as
// expectHash, outDir, and noValidate.
func NormalizeArgKeyName(key string) string {
	name := strings.TrimLeft(strings.TrimSpace(key), "-")
	name = strings.ReplaceAll(name, "_", "-")
	name = strings.ReplaceAll(name, " ", "-")
	if name == "" {
		return ""
	}
	var b strings.Builder
	var prev rune
	for i, r := range name {
		if r == '-' {
			if b.Len() > 0 && prev != '-' {
				b.WriteRune('-')
				prev = '-'
			}
			continue
		}
		if unicode.IsUpper(r) {
			if i > 0 && b.Len() > 0 && prev != '-' {
				b.WriteRune('-')
			}
			r = unicode.ToLower(r)
		}
		b.WriteRune(r)
		prev = r
	}
	out := b.String()
	out = strings.Trim(out, "-")
	return strings.ToLower(out)
}

// MarshalJSON re-emits the original raw value (so Operation round-trips cleanly).
func (a Arg) MarshalJSON() ([]byte, error) {
	if len(a.raw) == 0 {
		return []byte("null"), nil
	}
	return a.raw, nil
}

// UnmarshalJSON captures the raw bytes for deterministic stringification.
func (a *Arg) UnmarshalJSON(data []byte) error {
	a.raw = append([]byte(nil), data...)
	return nil
}

// ParseOps decodes an ops.json document into a slice of operations. It returns
// a descriptive error suitable for ExitInvalidArgs mapping by the caller.
func ParseOps(data []byte) ([]Operation, error) {
	trimmed := bytes.TrimSpace(data)
	if len(trimmed) == 0 {
		return nil, fmt.Errorf("ops file is empty")
	}
	dec := json.NewDecoder(bytes.NewReader(trimmed))
	dec.DisallowUnknownFields()
	var ops []Operation
	if err := dec.Decode(&ops); err != nil {
		return nil, fmt.Errorf("invalid ops JSON: %w", err)
	}
	var extra any
	if err := dec.Decode(&extra); err != io.EOF {
		if err == nil {
			return nil, fmt.Errorf("invalid ops JSON: trailing JSON value after operations array")
		}
		return nil, fmt.Errorf("invalid ops JSON: trailing data after operations array: %w", err)
	}
	for i := range ops {
		ops[i].Command = NormalizeCommand(ops[i].Command)
		if ops[i].Command == "" {
			return nil, fmt.Errorf("op %d: missing \"command\"", i)
		}
		if err := validateCommandWords(i, ops[i].Command); err != nil {
			return nil, err
		}
		if err := validateOpArgs(i, ops[i].Args); err != nil {
			return nil, err
		}
	}
	return ops, nil
}

func validateCommandWords(index int, command string) error {
	for _, word := range strings.Fields(command) {
		if strings.HasPrefix(word, "-") {
			return fmt.Errorf("op %d: command must contain only command words; put flag %q in args instead", index, word)
		}
	}
	return nil
}

func validateOpArgs(index int, args map[string]Arg) error {
	seen := make(map[string]string, len(args))
	for key := range args {
		name, err := validateArgKeyName(key)
		if err != nil {
			return fmt.Errorf("op %d: %w", index, err)
		}
		if isSessionOwnedMutationArg(key) {
			return fmt.Errorf("op %d: arg %q is owned by the apply/serve/MCP session; omit it from op args and set it on the outer command or session", index, name)
		}
		// Two case/separator-variant JSON keys (e.g. "Sheet" and "sheet") normalize
		// to the same flag and would both be appended to argv, where pflag silently
		// keeps the last in sorted order and drops the other. Reject the ambiguity at
		// parse time rather than silently mis-bind. (Map iteration is unordered, so
		// report both keys without implying which "wins".)
		if prev, dup := seen[name]; dup {
			return fmt.Errorf("op %d: arg keys %q and %q both map to flag %q; pass each flag at most once", index, prev, key, name)
		}
		seen[name] = key
	}
	return nil
}

func validateArgKeyName(key string) (string, error) {
	name := NormalizeArgKeyName(key)
	if name == "" {
		return "", fmt.Errorf("arg key %q must name a flag", key)
	}
	if strings.Contains(name, "=") {
		return "", fmt.Errorf("arg key %q must be a flag name without '='; put the flag value in the JSON value instead", key)
	}
	return name, nil
}

func isSessionOwnedMutationArg(key string) bool {
	normalized := NormalizeArgKeyName(key)
	switch normalized {
	case "out", "in-place", "inplace", "dry-run", "dryrun", "backup", "no-validate", "novalidate",
		"output", "json", "pretty", "no-color", "nocolor", "keep-temp", "keeptemp",
		"temp-dir", "tempdir", "verbosity", "strict", "help", "h", "o", "v":
		return true
	default:
		return false
	}
}

// NormalizeCommand converts full CLI command paths from capabilities
// ("ooxml xlsx cells set") into the op vocabulary consumed by apply/serve
// ("xlsx cells set"). It is deliberately forgiving because capabilities is the
// advertised command inventory agents read first.
func NormalizeCommand(command string) string {
	parts := strings.Fields(command)
	if len(parts) > 0 && strings.EqualFold(parts[0], "ooxml") {
		parts = parts[1:]
	}
	return strings.Join(parts, " ")
}
