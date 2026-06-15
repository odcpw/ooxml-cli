package cli

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/apply"
	findpkg "github.com/ooxml-cli/ooxml-cli/pkg/find"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// findSelfExecutable resolves the path to the ooxml binary used by the apply
// engine to re-dispatch each op as a subprocess. It is a package var so tests
// can inject a freshly built binary (in-process os.Executable() points at the
// test binary, which cannot dispatch ooxml subcommands).
var findSelfExecutable = os.Executable

// gatherFindOps opens the package, runs the read-only search, and converts the
// in-memory hits into apply-compatible operations using each hit's STRUCTURED op
// (never by re-parsing the printed mutationCommand string). newValue is
// substituted into each op's replacement argument; when empty the "<NEW>"
// placeholder is left in place (used by --to-ops without --replace).
//
// It returns the apply operations, the count of hits found, the indices of
// skipped hits (those with no semantic mutation command), and any error.
func gatherFindOps(filePath, query string, opts findpkg.Options, newValue string) (ops []apply.Operation, hitsFound int, skipped []int, positionDependent []int, duplicates []int, err error) {
	pkg, oerr := opc.Open(filePath)
	if oerr != nil {
		return nil, 0, nil, nil, nil, NewCLIErrorf(ExitUnexpected, "failed to open package: %v", oerr)
	}
	defer pkg.Close()

	typeKey, terr := findPackageTypeKey(pkg)
	if terr != nil {
		return nil, 0, nil, nil, nil, terr
	}

	result, serr := findpkg.Search(pkg, typeKey, opts)
	if serr != nil {
		return nil, 0, nil, nil, nil, mapFindSearchError(serr, opts.Regex)
	}

	opsResult, herr := findpkg.HitsToOps(result.Hits, newValue)
	if herr != nil {
		return nil, 0, nil, nil, nil, NewCLIErrorf(ExitUnexpected, "failed to build operations from hits: %v", herr)
	}

	// Bridge find's neutral Operation into apply.Operation via the canonical
	// ops.json encoding. This guarantees the emitted/applied ops are exactly what
	// `ooxml apply` accepts (apply.ParseOps is the single validator), without
	// importing apply's unexported Arg internals into pkg/find.
	encoded, merr := json.Marshal(opsResult.Ops)
	if merr != nil {
		return nil, 0, nil, nil, nil, NewCLIErrorf(ExitUnexpected, "failed to encode operations: %v", merr)
	}
	parsed, perr := apply.ParseOps(encoded)
	if perr != nil {
		return nil, 0, nil, nil, nil, NewCLIErrorf(ExitUnexpected, "generated operations are not apply-compatible: %v", perr)
	}
	if parsed == nil {
		parsed = []apply.Operation{}
	}
	return parsed, len(result.Hits), opsResult.SkippedHitIndices, opsResult.PositionDependentHitIndices, opsResult.DuplicateHitIndices, nil
}

// runFindToOps emits an apply-compatible ops.json (a bare JSON array of
// {command, args}) to stdout. It is read-only. Skipped hits (no mutation
// command) are reported as a diagnostic on stderr so stdout stays pure data and
// remains directly consumable by `ooxml apply --ops`.
func runFindToOps(cmd *cobra.Command, filePath, query string, opts findpkg.Options, newValue string) error {
	ops, hitsFound, skipped, posDep, duplicates, err := gatherFindOps(filePath, query, opts, newValue)
	if err != nil {
		return err
	}
	reportSkipped(cmd, hitsFound, len(ops), skipped)
	reportPositionDependent(cmd, posDep)
	reportDuplicates(cmd, duplicates)
	return writeGlobalJSON(cmd, ops)
}

// runFindApply composes find+apply: it builds ops from the hits and runs them
// through the apply engine (atomic, single final validation, per-op readback).
// With --dry-run it prints the resolved plan and executes nothing. The output
// contracts are reused from pkg/apply (apply.Plan / apply.Result).
func runFindApply(cmd *cobra.Command, filePath, query string, opts findpkg.Options, newValue string, mutOpts *MutationOptions) error {
	ops, hitsFound, skipped, posDep, duplicates, err := gatherFindOps(filePath, query, opts, newValue)
	if err != nil {
		return err
	}
	reportSkipped(cmd, hitsFound, len(ops), skipped)
	reportPositionDependent(cmd, posDep)
	reportDuplicates(cmd, duplicates)

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

	self, err := findSelfExecutable()
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to resolve own executable path: %v", err)
	}

	outputPath := mutOpts.OutPath
	if mutOpts.InPlace {
		outputPath = filePath
	}

	tempDir, err := os.MkdirTemp(applyTempBaseFor(cmd, outputPath), "ooxml-find-apply-*")
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tempDir)

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
		ValidateCommand: "ooxml validate --strict " + pptxXLSXCommandArg(outputPath),
	}
	if wantJSON {
		return writeGlobalJSON(cmd, result)
	}
	return writeGlobalOutput(cmd, []byte(renderApplyResultText(result)))
}

// reportSkipped writes a human diagnostic to stderr when some hits had no
// semantic mutation command (e.g. PPTX speaker notes). stdout is reserved for
// machine-readable result data; this keeps stdout consumable by `ooxml apply`.
func reportSkipped(cmd *cobra.Command, hitsFound, opsCount int, skipped []int) {
	if len(skipped) == 0 {
		return
	}
	fmt.Fprintf(cmd.ErrOrStderr(),
		"find->ops: %d hit(s), %d op(s), %d skipped (no mutation command): hits %v\n",
		hitsFound, opsCount, len(skipped), skipped)
}

// reportPositionDependent warns when some emitted ops target a POSITIONAL
// selector (no stable handle existed for that hit). In a multi-op apply batch an
// earlier op can structurally shift such a target's position; these ops are the
// only ones at risk. Ops carrying a stable handle survive structural shifts and
// are not listed. The diagnostic goes to stderr so stdout stays pure data.
func reportPositionDependent(cmd *cobra.Command, positionDependent []int) {
	if len(positionDependent) == 0 {
		return
	}
	fmt.Fprintf(cmd.ErrOrStderr(),
		"find->ops: %d op(s) are position-dependent (no stable handle; may break if an earlier batch op shifts their position): hits %v\n",
		len(positionDependent), positionDependent)
}

// reportDuplicates warns when some hits were collapsed because their emitted op
// was identical to an earlier hit's op (e.g. the same substring recurring within
// one shape). The deduped op already covers every such occurrence, so this is a
// diagnostic only; it keeps stdout pure data.
func reportDuplicates(cmd *cobra.Command, duplicates []int) {
	if len(duplicates) == 0 {
		return
	}
	fmt.Fprintf(cmd.ErrOrStderr(),
		"find->ops: %d hit(s) collapsed into an earlier identical op (the op already covers them): hits %v\n",
		len(duplicates), duplicates)
}

// findPackageTypeKey maps an opened package to find's type key.
func findPackageTypeKey(pkg opc.PackageSession) (string, error) {
	switch opc.DetectType(pkg) {
	case opc.PackageTypePPTX:
		return "pptx", nil
	case opc.PackageTypeXLSX:
		return "xlsx", nil
	case opc.PackageTypeDOCX:
		return "docx", nil
	default:
		return "", UnsupportedTypeError(opc.DetectType(pkg).String())
	}
}
