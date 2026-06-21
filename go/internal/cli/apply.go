package cli

import (
	"os"
	"path/filepath"

	"github.com/ooxml-cli/ooxml-cli/pkg/apply"
	"github.com/spf13/cobra"
)

var applyOpsPath string

// applySelfExecutable resolves the ooxml binary used for per-op subprocess
// dispatch. Tests inject a freshly built ./cmd/ooxml because os.Executable()
// points at the package test binary under go test.
var applySelfExecutable = os.Executable

var applyCmd = &cobra.Command{
	Use:   "apply <file>",
	Short: "Apply an ordered array of mutation operations all-or-nothing",
	Long: `Apply an ordered array of mutation operations to a single OOXML file
all-or-nothing, with a single final validation and per-op readback.

Operations come from a JSON file (--ops) shaped as an array of
{command, args} entries, for example:

  [
    {"command": "xlsx cells set", "args": {"sheet": "1", "cell": "A1", "value": "x"}},
    {"command": "xlsx cells set", "args": {"sheet": "1", "cell": "A2", "value": "y"}}
  ]

Each op is re-dispatched as a subprocess of this binary (clean isolation, no
in-process global-flag leakage) writing to a rolling temp file. The original
input is never touched until every op succeeds and the final package validates;
only then is the result written atomically to --out or --in-place. If any op
fails, the chain stops and nothing is written.

Use --dry-run to print the resolved plan (the argv that would run for each op)
without executing anything.

Exit codes: 0 success; 2 invalid ops.json or an op that failed argument
validation; 4/6 an op whose target type/object was not found; 5 op-level or final
whole-package validation failure; 1 an unexpected op error. An op failure
propagates the failing command's OWN exit code (so branch on the structured
--json error.exitCode, not on "op failure == 2").`,
	Args: cobra.ExactArgs(1),
	// Op/validation failures return a CLIError carrying the exit code; keep usage
	// off that path so stdout stays pure result data.
	SilenceUsage:  true,
	SilenceErrors: true,
	RunE:          runApply,
}

func runApply(cmd *cobra.Command, args []string) error {
	filePath := args[0]
	if _, err := os.Stat(filePath); err != nil {
		return FileNotFoundError(filePath)
	}
	if applyOpsPath == "" {
		return InvalidArgsError("--ops is required")
	}

	opsData, err := os.ReadFile(applyOpsPath)
	if err != nil {
		if os.IsNotExist(err) {
			return FileNotFoundError(applyOpsPath)
		}
		return NewCLIErrorf(ExitInvalidArgs, "failed to read ops file: %v", err)
	}
	ops, err := apply.ParseOps(opsData)
	if err != nil {
		return NewCLIErrorf(ExitInvalidArgs, "%v", err)
	}
	if err := validateKnownOperationCommands(ops); err != nil {
		return err
	}
	if err := validateOpBatchHandleSafety(ops); err != nil {
		return err
	}

	mutOpts, err := GetValidatedMutationOptions(cmd)
	if err != nil {
		return err
	}

	wantJSON := GetGlobalConfig(cmd).Format == "json"

	// --dry-run: print the resolved plan without executing anything.
	if mutOpts.DryRun {
		plan := apply.Plan{
			SchemaVersion: apply.SchemaVersion,
			File:          filePath,
			OpsCount:      len(ops),
			DryRun:        true,
			Plan:          apply.BuildPlan(ops, filePath),
		}
		if wantJSON {
			return writeGlobalJSON(cmd, plan)
		}
		return writeGlobalOutput(cmd, []byte(renderApplyPlanText(plan)))
	}

	self, err := applySelfExecutable()
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to resolve own executable path: %v", err)
	}

	outputPath := mutOpts.OutPath
	if mutOpts.InPlace {
		outputPath = filePath
	}

	tempDir, err := os.MkdirTemp(applyTempBaseFor(cmd, outputPath), "ooxml-apply-*")
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tempDir)

	// --backup is a literal backup path, matching MutationWriter's convention.
	backupPath := ""
	if mutOpts.InPlace && mutOpts.Backup != "" {
		backupPath = mutOpts.Backup
	}

	exec := &apply.Executor{Self: self, TempDir: tempDir}
	applied, err := exec.Execute(filePath, ops, outputPath, backupPath, mutOpts.NoValidate)
	if err != nil {
		return mapApplyError(err)
	}

	result := apply.Result{
		SchemaVersion:   apply.SchemaVersion,
		File:            filePath,
		OpsCount:        len(ops),
		Applied:         applied,
		Output:          outputPath,
		DryRun:          false,
		ValidateCommand: apply.ShellCommand("ooxml", "validate", "--strict", outputPath),
	}
	if wantJSON {
		return writeGlobalJSON(cmd, result)
	}
	return writeGlobalOutput(cmd, []byte(renderApplyResultText(result)))
}

// applyTempBase returns the directory to create the scratch temp dir under,
// honoring the global --temp-dir flag when set. With no override it returns ""
// (the OS default temp dir). Prefer applyTempBaseFor where an output path is
// known, so the final atomic move stays on the target filesystem.
func applyTempBase(cmd *cobra.Command) string {
	if cfg := GetGlobalConfig(cmd); cfg != nil && cfg.TempDir != "" {
		return cfg.TempDir
	}
	return ""
}

// applyTempBaseFor returns the directory to create the scratch temp dir under
// when the final output path is known. The --temp-dir override always wins; with
// no override it bases the scratch dir on the OUTPUT file's directory so the
// final MoveFile (rename) is intra-filesystem and therefore atomic, instead of
// landing in /tmp (often a different filesystem) and degrading to copy+remove.
// It falls back to "" (OS default temp) when the output dir is unknown or not
// usable for a scratch dir.
func applyTempBaseFor(cmd *cobra.Command, outputPath string) string {
	if cfg := GetGlobalConfig(cmd); cfg != nil && cfg.TempDir != "" {
		return cfg.TempDir
	}
	if outputPath == "" {
		return ""
	}
	dir := filepath.Dir(outputPath)
	if info, err := os.Stat(dir); err != nil || !info.IsDir() {
		return ""
	}
	return dir
}

// mapApplyError converts pkg/apply typed errors into CLIErrors with exit codes.
func mapApplyError(err error) error {
	switch e := err.(type) {
	case *apply.OpError:
		return cliErrorFromOpError(e)
	case *apply.ValidationError:
		return ValidationFailedErrorWithDiagnostics(e.Error(), e.Diagnostics)
	default:
		return NewCLIErrorf(ExitUnexpected, "%v", err)
	}
}

func init() {
	applyCmd.Flags().StringVar(&applyOpsPath, "ops", "", "path to ops.json (required)")
	AddMutationFlags(applyCmd)
	rootCmd.AddCommand(applyCmd)
}
