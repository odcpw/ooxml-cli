package cli

import (
	"github.com/ooxml-cli/ooxml-cli/pkg/capabilities"
	"github.com/spf13/cobra"
)

// capabilityExample mirrors capabilities.Example for the JSON contract. It is a
// thin CLI-layer DTO so the contract shape stays owned here, not in the pkg.
type capabilityExample struct {
	Command        string `json:"command"`
	Description    string `json:"description"`
	ExpectedOutput string `json:"expectedOutput,omitempty"`
}

// capabilityCommonError mirrors capabilities.CommonError for the JSON contract.
type capabilityCommonError struct {
	Pattern  string `json:"pattern"`
	Solution string `json:"solution"`
}

// examplesForPath returns the structured examples for a command path, or nil.
func examplesForPath(path string) []capabilityExample {
	meta, ok := capabilities.MetadataFor(path)
	if !ok {
		return nil
	}
	out := make([]capabilityExample, 0, len(meta.Examples))
	for _, ex := range meta.Examples {
		out = append(out, capabilityExample{
			Command:        ex.Command,
			Description:    ex.Description,
			ExpectedOutput: ex.ExpectedOutput,
		})
	}
	return out
}

// commonErrorsForPath returns the hand-authored common errors for a path, or nil.
func commonErrorsForPath(path string) []capabilityCommonError {
	meta, ok := capabilities.MetadataFor(path)
	if !ok {
		return nil
	}
	out := make([]capabilityCommonError, 0, len(meta.CommonErrors))
	for _, ce := range meta.CommonErrors {
		out = append(out, capabilityCommonError{Pattern: ce.Pattern, Solution: ce.Solution})
	}
	return out
}

// targetObjectKindsForPath returns the object kinds a command targets, or nil.
func targetObjectKindsForPath(path string) []string {
	meta, ok := capabilities.MetadataFor(path)
	if !ok {
		return nil
	}
	if len(meta.TargetObjectKinds) == 0 {
		return nil
	}
	kinds := make([]string, len(meta.TargetObjectKinds))
	copy(kinds, meta.TargetObjectKinds)
	return kinds
}

// applyExampleMetadata walks the command tree and, for every command that has
// authored example metadata but no hand-authored cobra Example, derives the
// Command.Example help text from the metadata. Commands that already set Example
// (e.g. find, clone-slide) are left untouched so their richer snippets survive.
// This keeps a single source of truth for examples shared by help and the JSON
// contract. It is idempotent and safe to call at init time.
func applyExampleMetadata(root *cobra.Command) {
	var walk func(*cobra.Command)
	walk = func(parent *cobra.Command) {
		for _, child := range parent.Commands() {
			if child.Example == "" {
				if examples := examplesForPath(child.CommandPath()); len(examples) > 0 {
					child.Example = renderExampleHelp(examples)
				}
			}
			walk(child)
		}
	}
	walk(root)
}

// renderExampleHelp formats structured examples as cobra Example help text:
// each example as a "  <command>\n  # <description>" pair.
func renderExampleHelp(examples []capabilityExample) string {
	var b []byte
	for i, ex := range examples {
		if i > 0 {
			b = append(b, '\n')
		}
		b = append(b, "  "...)
		b = append(b, ex.Command...)
		b = append(b, '\n')
		b = append(b, "  # "...)
		b = append(b, ex.Description...)
	}
	return string(b)
}
