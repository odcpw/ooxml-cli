package render

import (
	"bytes"
	"context"
	"os/exec"
)

// Runner abstracts command discovery and execution for testability.
type Runner interface {
	LookPath(name string) (string, error)
	Run(ctx context.Context, name string, args []string) (*RunResult, error)
}

// RunResult captures subprocess output.
type RunResult struct {
	Stdout string
	Stderr string
}

// ExecRunner is the real subprocess runner used in production.
type ExecRunner struct{}

func (ExecRunner) LookPath(name string) (string, error) {
	return exec.LookPath(name)
}

func (ExecRunner) Run(ctx context.Context, name string, args []string) (*RunResult, error) {
	cmd := exec.CommandContext(ctx, name, args...)
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	err := cmd.Run()
	return &RunResult{Stdout: stdout.String(), Stderr: stderr.String()}, err
}
