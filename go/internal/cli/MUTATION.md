# Mutation Safety Infrastructure

This document describes the shared mutation safety layer that enforces safe, atomic operations for all mutating commands (`pptx replace text`, `pptx replace images`, `clone-slide`, `new-slide-from-layout`).

## Overview

The mutation safety infrastructure provides a **shared safety layer** that all mutating commands must use. It enforces:

1. **Exactly one output mode**: Either `--out` (write to a new file) OR `--in-place` (modify original)
2. **Atomic writes**: Changes are written to a temp file first, then atomically moved to the final location
3. **Post-write validation**: The output is validated before finalizing (can be skipped with `--no-validate`)
4. **Backup support**: In-place mutations can optionally create a backup file

## Architecture

### Core Types

#### `MutationOptions`
```go
type MutationOptions struct {
    OutPath    string // --out flag: output file path
    InPlace    bool   // --in-place flag: modify original
    Backup     string // --backup flag: backup file path (--in-place only)
    NoValidate bool   // --no-validate flag: skip post-write validation
}
```

#### `MutationWriter`
Safe writer that handles temp file creation, validation, and atomic replacement:
```go
type MutationWriter struct {
    // Private fields manage temp file location, output path, backup path
}
```

### Core Functions

#### `ValidateMutationFlags(opts *MutationOptions) error`
Validates that:
- Exactly one of `OutPath` or `InPlace` is set
- `Backup` is only used with `InPlace`

Returns `InvalidArgsError` for violations.

#### `NewMutationWriter(inputPath string, opts *MutationOptions) (*MutationWriter, error)`
Creates a mutation writer that will:
- Use a temp file in the same directory as the output (for atomic rename)
- Track paths for backup and cleanup on error

#### `(w *MutationWriter) Write(fn func(*opc.Package) error) error`
Executes the mutation in a safe context:

1. Opens the input package
2. Calls the mutation function `fn` to apply changes
3. Saves modified package to temp file
4. If validation enabled: opens temp file, validates it, checks for errors
5. Creates backup if requested (for in-place mutations)
6. Atomically moves temp file to output location
7. On error: cleans up temp file and returns error

Validation runs only on the temp file, so if it fails, the original is not touched.

#### `AddMutationFlags(cmd *cobra.Command)`
Adds standard mutation flags to a command:
- `--out`: output file path (mutually exclusive with `--in-place`)
- `--in-place`: modify input file in place
- `--backup`: backup file path (only with `--in-place`)
- `--no-validate`: skip post-write validation

#### `GetMutationOptions(cmd *cobra.Command) (*MutationOptions, error)`
Extracts and returns `MutationOptions` from command flags.

#### `GetValidatedMutationOptions(cmd *cobra.Command) (*MutationOptions, error)`
Extracts mutation options and validates the shared write contract in the standard command-handler order.

## Global Flags

The following flags are registered at the root command level (available to all commands):
- `--out`: Output file for mutations
- `--in-place`: Modify input file in place
- `--backup`: Backup suffix for in-place mutations

These are attached to mutating commands so each command exposes the same write contract.

## Usage Pattern for Mutating Commands

When implementing a new mutating command such as `pptx replace text`, follow this pattern:

```go
// In your command's RunE function:
func RunE(cmd *cobra.Command, args []string) error {
    inputPath := args[0]
    
    // Get and validate mutation options from flags
    opts, err := GetValidatedMutationOptions(cmd)
    if err != nil {
        return err
    }
    
    // Create the mutation writer
    writer, err := NewMutationWriter(inputPath, opts)
    if err != nil {
        return err
    }
    
    // Execute the mutation in a safe context
    err = writer.Write(func(pkg *opc.Package) error {
        // Your mutation logic here:
        // - Read parts from pkg
        // - Modify them
        // - Write modified parts back to pkg
        
        // Example:
        doc, err := pkg.ReadXMLPart("/ppt/slides/slide1.xml")
        if err != nil {
            return err
        }
        
        // ... modify doc ...
        
        return pkg.ReplaceXMLPart("/ppt/slides/slide1.xml", doc)
    })
    
    if err != nil {
        return err
    }
    
    return nil
}
```

## Validation Behavior

### Default (validation enabled)

The mutation writer automatically:
1. Saves the mutated package to a temp file
2. Opens and validates the temp file using `pkg/validate.ValidatePackage`
3. Checks for validation errors
4. Only if validation passes, proceeds to write the final output

### With `--no-validate`

Validation is completely skipped. Use this only if you:
- Are confident the mutation cannot introduce validation errors
- Have already validated elsewhere
- Need the performance improvement

### Error Handling

If validation fails:
- The temp file is deleted
- The original file (or output path) is NOT modified
- An error is returned with `ExitValidationFailed` exit code

## Backup Behavior

### Without `--backup`
In-place mutations overwrite the original file directly (after temp file validation passes).

### With `--backup`
In-place mutations:
1. Create a backup copy of the original file at the specified path
2. Then overwrite the original with the mutated version

If the backup path already exists, it is silently overwritten.

## Exit Codes

The mutation infrastructure uses these exit codes:
- `0` (ExitSuccess): Mutation succeeded
- `1` (ExitUnexpected): Unexpected error (temp file I/O, etc.)
- `2` (ExitInvalidArgs): Invalid mutation flags
- `5` (ExitValidationFailed): Validation errors in output

## Examples

### Replace text, write to new file
```bash
ooxml pptx replace text input.pptx --slide 1 --target title --text "new" --out output.pptx
```

### Replace text in-place with backup
```bash
ooxml pptx replace text input.pptx --slide 1 --target title --text "new" --in-place --backup ".backup"
```

### Replace image with validation disabled
```bash
ooxml pptx replace images input.pptx --slide 1 --target "shape:2" --image new.jpg --out output.pptx --no-validate
```

### Clone slide in-place
```bash
ooxml clone-slide input.pptx --from 1 --to 3 --in-place
```

## Testing

The mutation infrastructure is covered by focused unit tests in `mutation_test.go`:

- `TestValidateMutationFlags`: Flag validation logic
- `TestNewMutationWriter`: Writer creation with various flag combinations
- `TestMutationOptionsStruct`: Options struct construction
- `TestCopyFileHelper`: File copy utility

Run tests with:
```bash
go test -v ./internal/cli -run Mutation
```

## Implementation Notes

### Temp File Location
Temp files are created in the same directory as the output file (not in system temp) to ensure the rename operation is atomic. This requires the output directory to be writable.

### Atomic Rename
`os.Rename()` is used for atomic file replacement on most systems. On Windows, this replaces the target file if it exists.

### Backup Creation
Backups are created before the atomic rename, so if the rename fails, the original remains unchanged. Existing backups are silently overwritten.

### Error Recovery
If any step fails after temp file creation:
1. The temp file is cleaned up via `defer` or explicit cleanup
2. Backups (if created) are left in place (not rolled back)
3. The original file remains unchanged

## Readback Symmetry (Canonical Shapes)

When a mutation command runs with `--format json`, it embeds a *readback* object
describing the changed object as it exists in the produced file. The contract is
**shape parity**: a mutation's readback uses the same JSON shape (ideally the
same Go type) as the inspect/show command for that object, so an agent can diff
before/after with a single struct on both sides.

| Mutation command | Readback field | Canonical type | Matching inspect command |
| --- | --- | --- | --- |
| `xlsx cells set` | `destination` | `XLSXRangeDestination` | `xlsx ranges export --include-types --include-formulas --include-formats` (data block: `range`, `rows`, `cols`, `values`, `types`, `formulas`, `styleIndexes`, `numberFormatIds`, `numberFormatCodes`, `formulaCount`) |
| `xlsx ranges set` | `destination` | `XLSXRangeDestination` | same as above |
| `pptx tables set-cell` | `destination` | `*PPTXTableSummary` | `pptx tables show` (each element of `tables`) |
| `pptx shapes set-bounds` | `destination` | `*PPTXShapeDestination` | `pptx shapes get --include-bounds` (shared fields of `PPTXShapeEntry`: `shapeId`, `shapeName`, `targetKind`, `primarySelector`, `selectors`, `bounds`, `geometry`, `imageRef`) |
| `docx blocks replace` | `destination` | `*extract.BlockReport` | `docx blocks --block N --include-runs` (each element of `blocks`) |

Each mutation also emits a generated readback command (e.g. `readbackCommand`,
`rangesExportCommand`) that already includes the inspect flags needed to surface
the parity fields — run it verbatim to reproduce the inspect side.

### Rules for changing readback shapes

- **Additive only.** Never rename or remove an existing JSON field; 200+ tests
  depend on the current tags. To align a divergent shape, add a field that
  mirrors the inspect type (as `docx blocks replace` now embeds
  `destination: extract.BlockReport`) rather than renaming the legacy fields.
- **Metadata is allowed to differ.** Mutation-only fields (`file`, `output`,
  `dryRun`) and inspect-only top-level metadata are not part of the parity
  contract. Parity is asserted on the shared *domain* fields only.
- **Flag-gating matters.** Inspect commands gate the interesting fields behind
  flags (`--include-types`, `--include-bounds`, `--include-runs`); the generated
  readback command sets them so the inspect output is populated.

Parity is enforced by `readback_symmetry_test.go`, which unmarshals both sides
into the same struct and asserts the shared fields match.

### Known divergences left as future work

- `PPTXShapeDestination` omits `shapeType`/`order`/`textCapable`/`tableInfo`
  that `PPTXShapeEntry` carries. These are not part of the shared parity set and
  adding them would be additive polish, not a parity fix.
- `pptx shapes set-bounds` populates `destination.textPreview`, but its advertised
  readback command (`pptx shapes get --include-bounds`) deliberately omits
  `--include-text`, so the inspect side reports no text. `textPreview` is therefore
  NOT a parity field for this surface; the bounds-focused readback intentionally
  excludes text.
- `docx paragraphs set` has no inspect counterpart for a single paragraph block;
  it keeps its independent result shape (readback parity is out of scope until a
  paragraph-level inspect surface exists).

## Future Enhancements

Potential improvements:
- Add `--backup-dir` to place backups in a specific directory
- Add timestamp-based backup suffix generation
- Add rollback capability that restores from backup on error
- Add dry-run mode that validates but doesn't write
- Extend readback symmetry to `docx paragraphs set` once a paragraph-level
  inspect command exists
