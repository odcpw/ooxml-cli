# Testing Strategy

The test strategy is practical compatibility, not exhaustive OOXML conformance. A change is trustworthy when it is covered at the right level for its risk: unit tests for pure logic, command-path tests for CLI contracts, validation for every mutation, and Office-open proof for compatibility-sensitive package writes.

## Proof Ladder

1. `go test ./...`: Go unit, package, integration, and golden tests.
2. `ooxml validate --strict <file>`: package, relationship, XML, semantic, and VBA consistency checks.
3. Microsoft Open XML SDK validation: schema-order and enum checks that local validation can miss.
4. LibreOffice/render/open checks: useful headless evidence for generated artifacts.
5. Desktop Microsoft Office COM open proof: strongest local proof that Word, Excel, or PowerPoint opens the file without repair/failure.

Macro execution and VBE compile are not part of the automated proof ladder.

## Local Gates

Fast normal loop:

```powershell
go vet ./...
go test ./...
```

If `make` is installed, `make verify` runs the same local gate. `make verify-strict` also enforces repo-wide gofmt.

Focused loop:

```powershell
go test -count=1 ./internal/cli -run '<focused-regex>'
go test -count=1 ./pkg/vba ./pkg/validate
```

Windows Office proof:

```powershell
make check-office-schema
make check-office-com
make check-office-vba-schema
make check-office-vba-com
make check-release-fast
make check-release-slow
```

PowerShell equivalents:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -RequireOpenXmlSdk -RunConformance -SkipOffice
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -OfficeOracleTimeoutSeconds 120 -RequireOpenXmlSdk -RunConformance
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -RequireOpenXmlSdk -SkipOffice -EnableVbaObjectModelAccess
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -RequireOpenXmlSdk -EnableVbaObjectModelAccess -OfficeOracleTimeoutSeconds 120
```

## What To Test

Use the narrowest test that proves the behavior:

- Pure parsing/normalization: package tests in `pkg/...`.
- CLI flags, JSON contracts, command hints, and generated commands: command-path tests in `internal/cli`.
- Package writes: mutation tests that validate output and read back changed objects.
- Shared OPC/content-type/relationship changes: validation and conformance tests.
- PPTX visual placement/layout: render representative decks.
- VBA changes: strict validation, Open XML SDK validation, and Office/LibreOffice load evidence before compatibility claims.

Every mutating command should prove:

- exactly one destination mode: `--out`, `--in-place`, or `--dry-run`;
- output validates unless `--no-validate` is explicit;
- stale-target guards fail before writing when supplied;
- JSON output includes the changed readback or enough follow-up commands to verify manually;
- untouched package parts remain preserved where practical.

## Fixture Policy

Use generated fixtures for small deterministic cases and committed real/exported fixtures for producer behavior.

Keep:

- `testdata/pptx/**` for decks from python-pptx, LibreOffice, Google Slides, PowerPoint, and targeted edge cases.
- `testdata/xlsx/**` for workbook structures, tables, charts, pivots, names, and validation cases.
- `testdata/docx/**` for document body/table/style/header/image/comment cases.
- generated fixture scripts under `testdata/generate/python` when they remain reproducible.

For VBA, avoid pretending synthetic projects prove Office compatibility. Synthetic fixtures are fine for parser/writer unit coverage; Office-facing claims require Office-authored `.xlsm`/`.pptm` files or `tools/windows-office-vba-smoke.ps1`.

## Golden Tests

Golden tests should compare normalized JSON, not console text or raw XML serialization. Good goldens are:

- command outputs that define an agent contract;
- validation/conformance summaries;
- generated-command summaries;
- workflow summaries such as find/apply/readback.

When intentional output changes occur, update goldens with `UPDATE_GOLDENS=1` and inspect the diff.

## Office And VBA Smoke Gates

`tools/windows-office-edit-smoke.ps1` builds the CLI, mutates representative DOCX/XLSX/PPTX files, runs strict validation and Open XML SDK validation, optionally runs conformance, and optionally opens outputs in desktop Office.

`tools/windows-office-vba-smoke.ps1` creates Office-native `.xlsm` and `.pptm` seeds from `.bas` / `.cls` sources through `ooxml vba create`, proves `vbaProject.bin` extract/attach/remove, proves existing-module replacement, validates outputs, asserts real Office-shaped add/remove are refused, and optionally opens macro-enabled outputs in Excel and PowerPoint.

`tools/windows-office-vba-create.ps1` is the backend helper for `ooxml vba create`. It is useful for troubleshooting Office COM directly, but the CLI command is the agent-facing workflow and the smoke gate is the proof.

## Release Gate

Before calling a Windows-compatible editing path ready:

1. Run `go test ./...`.
2. Run `go vet ./...`.
3. Run the appropriate Office schema gate.
4. For compatibility-sensitive package writes, run the corresponding Office COM gate.
5. Report the proof level and the summary JSON path.
