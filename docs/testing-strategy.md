# Testing Strategy

The test strategy is practical compatibility, not exhaustive OOXML conformance. Rust is the current/default product and proof path; the old implementation is historical source material, not an oracle. A change is trustworthy when it is covered at the right level for its risk: unit tests for pure logic, command-path tests for CLI contracts, validation for every mutation, release real-file traces, and Office-open proof for compatibility-sensitive package writes.

## Proof Ladder

1. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets`: Rust format, lint, unit, and contract tests.
2. Rust contract smoke tests and legacy frozen JSON contract fixtures. Historical `*_baseline_*` helpers rerun the current Rust subject by default, which proves repeatability and supports their command-specific assertions but is not external parity evidence. Set `OOXML_RUST_COMPARISON_BIN` only for an intentional differential run; the harness canonicalizes both paths and refuses the current executable, symlinks to it, and Unix hard links to it.
3. `ooxml validate --strict <file>`: package, relationship, XML, semantic, and VBA consistency checks.
4. Microsoft Open XML SDK validation: schema-order and enum checks that local validation can miss.
5. LibreOffice/render/open checks: useful headless evidence for generated artifacts.
6. Desktop Microsoft Office COM open proof: strongest local proof that Word, Excel, or PowerPoint opens the file without repair/failure.
7. Explicit opt-in VBA run smoke: local Excel COM executes a harmless generated XLSM macro.

General macro execution and VBE compile are not part of the normal automated proof ladder.

## Local Gates

Fast normal loop:

```powershell
$env:CARGO_PROFILE_DEV_DEBUG = "0"
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
make check-ci
```

`make check-ci` is the CI-equivalent Rust gate. It includes the full `cargo test --all-targets` suite, not a unit-only or one-smoke-test subset. CI also runs that full gate on Linux, macOS, and Windows.

Do not use legacy code as the normal product proof path. Historical code may still explain where an older contract fixture came from, but current gates should be Rust-native.

Focused loop:

```powershell
cargo test --test rust_contract_smoke <filter> -- --nocapture
cargo test --lib <module_filter> -- --nocapture
```

Windows Office proof:

```powershell
cargo build --bin ooxml
$targetDir = (cargo metadata --format-version 1 --no-deps | ConvertFrom-Json).target_directory
$debugBin = Join-Path $targetDir "debug\ooxml.exe"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -BinaryPath $debugBin -SkipBuild -MutationParallelism 4 -RequireOpenXmlSdk -RunConformance -SkipOffice
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -BinaryPath $debugBin -SkipBuild -MutationParallelism 4 -OfficeOracleTimeoutSeconds 120 -RequireOpenXmlSdk -RunConformance
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -BinaryPath $debugBin -SkipBuild -RequireOpenXmlSdk -SkipOffice -EnableVbaObjectModelAccess
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -BinaryPath $debugBin -SkipBuild -RequireOpenXmlSdk -EnableVbaObjectModelAccess -OfficeOracleTimeoutSeconds 120
& $debugBin --json vba run-smoke --timeout-seconds 45 --out-dir .\proof\xlsm-run-smoke
& $debugBin --json vba run-smoke --smoke-mode Class --timeout-seconds 45 --out-dir .\proof\xlsm-class-run-smoke
& $debugBin --json vba office-check .\path\to\pure-generated.pptm --out-dir .\proof\pure-pptm-office-check
& $debugBin --json vba office-check .\path\to\pure-generated.docm --out-dir .\proof\pure-docm-office-check
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

For VBA, avoid pretending synthetic projects prove Office compatibility. Synthetic fixtures are fine for parser/writer unit coverage; Office-facing claims require a real Office oracle such as `ooxml vba office-check` on the generated macro package, `tools/windows-office-vba-smoke.ps1`, or the explicit XLSM macro-run smoke.

## Golden Tests

Golden tests should compare normalized JSON, not console text or raw XML serialization. Good goldens are:

- command outputs that define an agent contract;
- validation/conformance summaries;
- generated-command summaries;
- workflow summaries such as find/apply/readback.

When intentional output changes occur, update goldens with `UPDATE_GOLDENS=1` and inspect the diff.

## Office And VBA Smoke Gates

`tools/windows-office-edit-smoke.ps1` mutates representative DOCX/XLSX/PPTX files, runs strict validation and Open XML SDK validation, optionally runs conformance, and optionally opens outputs in desktop Office.

When validating the Rust CLI, pass `-BinaryPath <path-to-ooxml.exe>` together
with `-SkipBuild`; an explicit `-BinaryPath` without `-SkipBuild` is rejected so
the script cannot overwrite the subject. The helper's implicit build path is
legacy behavior, not the normal Rust proof path.

`tools/windows-office-vba-smoke.ps1` creates Office-native `.xlsm` and `.pptm` seeds from `.bas` / `.cls` sources through legacy `ooxml vba create`, proves `vbaProject.bin` extract/attach/remove, proves existing-module replacement, validates outputs, asserts real Office-shaped add/remove are refused, creates pure Rust XLSM/PPTM/DOCM standard and class packages from scaffolded hosts, and optionally opens macro-enabled outputs in desktop Office.

For one-off pure Rust authoring proof, generate the macro package with `vba create --pure` or `build-bin` + `attach`, then run `ooxml --json vba office-check <file.xlsm|file.pptm|file.docm>`. On Windows this prefers the Microsoft Office COM oracle and records `microsoftOfficeVerified: true` only when Excel, PowerPoint, or Word opens the file without repair/failure.

`ooxml --json vba run-smoke` creates a pure Rust XLSM from a harmless `.bas` module, validates it, opens it in Excel, executes the macro, and verifies a marker value. Its opt-in `--smoke-mode Class` lane generates an `AgentSmoke.bas` entrypoint plus `Worker.cls` and only passes when the class method supplies the verified value. It is explicit opt-in because it runs VBA; internally it wraps `tools/windows-office-vba-run-smoke.ps1`.

`tools/windows-office-vba-create.ps1` is the backend helper for `ooxml vba create`. It is useful for troubleshooting Office COM directly, but the CLI command is the agent-facing workflow and the smoke gate is the proof.

## Release Gate

Before calling a Windows-compatible editing path ready:

1. Run `cargo fmt --check`.
2. Run `cargo clippy --all-targets -- -D warnings`.
3. Run `cargo test --all-targets`.
4. Run the appropriate Office schema gate against the Rust binary.
5. For compatibility-sensitive package writes, run the corresponding Office COM gate.
6. Report the proof level and the summary JSON path.

For a formal binary release, update the Cargo version and lockfile, land a green candidate, then push a matching `vX.Y.Z` tag. `.github/workflows/release.yml` rejects a tag/version mismatch, reruns format/clippy/all-target tests, builds native Linux x86_64, macOS arm64/x86_64, and Windows x86_64 archives, generates `SHA256SUMS`, and publishes those assets to the GitHub Release. `v0.1.0` is the first release under this process.
