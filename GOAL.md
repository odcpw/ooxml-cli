# Goal: Practical Codex-Native OOXML Workbench

Build `ooxml-cli` into a practical agent-facing Office workbench for ordinary business files:

- PPTX/PPTM decks: inspect, find targets, edit text/images/tables/charts/layouts, validate, render, and read back.
- XLSX/XLSM workbooks: inspect sheets/ranges/tables/charts/pivots, edit structured data, validate, and export typed readback.
- VBA projects: create Office-authored macro-enabled files from source modules on Windows, safely move Office-authored macro projects, inspect/extract source modules, and replace existing modules with guards.
- DOCX/DOCM documents: support common business reports, tables, headers/footers, images, comments, and find/replace.

Do not chase rare OOXML corners unless a real user file needs them. The main value is: an agent can inspect a file, discover stable handles, make semantic edits, validate/read back the result, and return a usable Office file with minimal guessing.

## Current Truth

Working and useful today:

- `capabilities --json`, `doctor`, `find`, `apply`, `serve`, and `mcp` exist as agent-facing surfaces.
- PPTX and XLSX have broad practical inspection and mutation coverage.
- DOCX covers common business-document edits.
- VBA package-level `vbaProject.bin` inspect/extract/attach/remove works for PPTX/PPTM and XLSX/XLSM.
- `ooxml vba create` creates fresh Office-authored `.xlsm` and `.pptm` files from `.bas` / `.cls` sources on Windows desktop Office and can extract a reusable seed `vbaProject.bin`.
- VBA source `list`/`extract` works for parseable projects.
- VBA `replace-module` works for existing parseable modules with stale-source guards.
- Windows VBA smoke creates Office-native `.xlsm` and `.pptm` seeds, proves attach/remove and existing-module replacement, validates with strict/Open XML SDK checks, and opens macro-enabled outputs through Excel/PowerPoint COM.

Important accepted limits:

- Real Office-shaped `add-module` and `remove-module` are intentionally refused because Office stores version-dependent `VBA/_VBA_PROJECT` module-set metadata.
- To change the module set for Office-facing files, use `ooxml vba create` for a fresh `.xlsm`/`.pptm`, or create/obtain an Office-authored `vbaProject.bin` and use `ooxml vba attach`.
- `tools/windows-office-vba-create.ps1` is the Windows Office-backed helper behind `ooxml vba create`; agents should use the CLI command first.
- Macro execution, VBE compile proof, signatures/resigning, forms, password/protection editing, and procedure-level helpers are out of scope for now.

## Operating Rules

- `ooxml capabilities --json` is the finite command contract.
- Every mutation must require `--out`, `--in-place`, or `--dry-run`.
- Every changed package must validate by default.
- JSON mutation output should include `file`, `output`, `dryRun`, changed-object readback where useful, and generated follow-up commands.
- Errors should return useful candidates or exact discovery/repair commands.
- For compatibility-sensitive work, proof strength is:
  1. `ooxml validate --strict`
  2. Microsoft Open XML SDK schema validation
  3. LibreOffice/render/open checks
  4. desktop Microsoft Office COM open proof

## Useful Next Work

Do these only when they remove real operational friction:

1. Keep `ooxml vba create` first-class in README, the agent skill, capabilities, and smoke gates.
2. Add small deterministic fixtures only if they let CI cover current supported behavior without requiring desktop Office.
3. Improve structured `dir`/PROJECT reference parsing when it helps diagnostics or safe replacement.
4. Harden code-page reporting/conversion only when a real file proves the need.
5. Keep the guard messages for unsupported real Office-shaped add/remove explicit and actionable.

Do not build inline procedure/function editing yet. Do not build macro execution. Do not do broad refactors without a green baseline and a scored isomorphism case.

## Verification Commands

Fast local gate:

```powershell
go vet ./...
go test ./...
```

If `make` is installed, `make verify` runs the same local gate. `make verify-strict` also enforces repo-wide gofmt.

Windows Office gates:

```powershell
make check-office-schema
make check-office-com
make check-office-vba-schema
make check-office-vba-com
make check-release-fast
make check-release-slow
```

Without `make`, run the PowerShell scripts directly:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -RequireOpenXmlSdk -RunConformance -SkipOffice
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -OfficeOracleTimeoutSeconds 120 -RequireOpenXmlSdk -RunConformance
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -RequireOpenXmlSdk -SkipOffice -EnableVbaObjectModelAccess
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -RequireOpenXmlSdk -EnableVbaObjectModelAccess -OfficeOracleTimeoutSeconds 120
```

## Rust Port Direction

Do not port now. The compatibility proof is the asset, and Go is currently good for this CLI. Revisit Rust only after the command contract and fixture/oracle harness are stable enough to run a strangler/parity port one operation at a time.
