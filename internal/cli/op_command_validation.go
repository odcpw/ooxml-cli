package cli

import (
	"fmt"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/apply"
	"github.com/spf13/cobra"
)

// validateOpBatchHandleSafety rejects a batch in which a structural row/column
// shift op precedes an op targeting an address-positional XLSX handle (cell or
// comment). apply sees the whole ops array up front, so it can prevent the
// silent-wrong-target write statically; serve/MCP apply ops incrementally (one
// op per call) and rely on the runtime HANDLE_STALE guard (which catches an
// insert that empties the address) plus this documented in-session limitation.
//
// The check is intentionally conservative: it does not resolve whether the shift
// and the handle target the SAME sheet (the shift op names its sheet by
// number/name while the handle carries a sheetId, and matching them needs the
// open workbook). A cross-sheet false positive is a clean pre-flight error with
// an actionable workaround, never a silent write.
func validateOpBatchHandleSafety(ops []apply.Operation) *CLIError {
	if violation, ok := apply.FirstHandleSafetyViolation(ops); ok {
		return NewCLIErrorf(ExitInvalidArgs, "%s", violation.Error())
	}
	return nil
}

func validateKnownOperationCommand(command string) *CLIError {
	normalized := apply.NormalizeCommand(command)
	if normalized == "" {
		return InvalidArgsError("missing command")
	}
	wantPath := "ooxml " + normalized
	cmd := commandForPath(rootCmd, wantPath)
	if cmd == nil {
		return InvalidArgsError(fmt.Sprintf("unknown command %q; command must be one command path from `ooxml capabilities --json`, with flags and positional values supplied through args", command))
	}
	if reason := operationCommandIncompatibility(cmd); reason != "" {
		return InvalidArgsError(fmt.Sprintf("command %q cannot be used as an apply/serve/MCP op: %s; use a mutation command whose only positional argument is the package file, and supply every other value through args", command, reason))
	}
	return nil
}

func validateKnownOperationCommands(ops []apply.Operation) *CLIError {
	for i, op := range ops {
		if err := validateKnownOperationCommand(op.Command); err != nil {
			return InvalidArgsError(fmt.Sprintf("op %d: %s", i, err.Message))
		}
	}
	return nil
}

func commandForPath(root *cobra.Command, path string) *cobra.Command {
	var found *cobra.Command
	var walk func(*cobra.Command)
	walk = func(parent *cobra.Command) {
		if found != nil {
			return
		}
		if parent.CommandPath() == path {
			found = parent
			return
		}
		for _, child := range parent.Commands() {
			if child.Hidden {
				continue
			}
			walk(child)
		}
	}
	walk(root)
	return found
}

func operationCommandIncompatibility(cmd *cobra.Command) string {
	if cmd.HasSubCommands() {
		return "it is a command group, not a leaf mutation command"
	}
	if cmd.Flags().Lookup("out") == nil || cmd.Flags().Lookup("no-validate") == nil {
		return "it does not accept the mutation output flags injected by the op engine"
	}
	if count := requiredUsePositionals(cmd.Use); count != 1 {
		return fmt.Sprintf("its use string %q has %d required positional arguments; op can supply only the package file positionally", cmd.Use, count)
	}
	return ""
}

func requiredUsePositionals(use string) int {
	count := 0
	for _, word := range strings.Fields(use) {
		if strings.HasPrefix(word, "<") && strings.HasSuffix(word, ">") {
			count++
		}
	}
	return count
}
