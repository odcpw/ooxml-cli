package cli

import (
	"fmt"

	"github.com/ooxml-cli/ooxml-cli/pkg/apply"
)

func cliErrorFromOpError(e *apply.OpError) *CLIError {
	body, _ := opErrorBody(e)
	exitCode := body.ExitCode
	if exitCode == 0 {
		exitCode = ExitInvalidArgs
	}
	code := body.Code
	if code == "" {
		code = codeForExit(exitCode)
	}
	childMessage := body.Message
	if childMessage == "" {
		childMessage = e.Error()
	}
	diagnostics := make([]DiagnosticJSON, 0, len(body.Diagnostics)+1)
	diagnostics = append(diagnostics, DiagnosticJSON{
		Code:     "op_failed",
		Severity: "error",
		Message:  fmt.Sprintf("op %d (%s) failed", e.FailedOpIndex, e.Command),
	})
	diagnostics = append(diagnostics, body.Diagnostics...)
	return &CLIError{
		ExitCode:    exitCode,
		Code:        code,
		Message:     fmt.Sprintf("op %d (%s) failed: %s", e.FailedOpIndex, e.Command, childMessage),
		Diagnostics: diagnostics,
	}
}
