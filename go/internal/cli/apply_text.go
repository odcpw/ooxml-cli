package cli

import (
	"fmt"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/apply"
)

func renderApplyPlanText(plan apply.Plan) string {
	var b strings.Builder
	fmt.Fprintf(&b, "apply plan for %s (%d op(s), dry-run)\n", plan.File, plan.OpsCount)
	for _, e := range plan.Plan {
		fmt.Fprintf(&b, "  [%d] %s\n", e.Index, e.Command)
		fmt.Fprintf(&b, "      ooxml %s\n", strings.Join(e.Argv, " "))
	}
	return strings.TrimRight(b.String(), "\n")
}

func renderApplyResultText(result apply.Result) string {
	var b strings.Builder
	fmt.Fprintf(&b, "applied %d op(s) to %s\n", len(result.Applied), result.File)
	for _, op := range result.Applied {
		fmt.Fprintf(&b, "  [%d] %s\n", op.Index, op.Command)
	}
	if result.Output != "" {
		fmt.Fprintf(&b, "wrote %s\n", result.Output)
	}
	if result.ValidateCommand != "" {
		fmt.Fprintf(&b, "verify: %s\n", result.ValidateCommand)
	}
	return strings.TrimRight(b.String(), "\n")
}
