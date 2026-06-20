# Windows Microsoft Office Oracle

This project can catch many OOXML mistakes on Linux with strict validation,
golden artifacts, and LibreOffice smoke tests. The final repair-prompt oracle is
desktop Microsoft Office on Windows.

Use this workflow when a generated `.pptx`, `.xlsx`, or macro-enabled variant
opens on Linux but Excel or PowerPoint says the file needs repair.

## Windows Setup

Install:

- Git
- Go
- PowerShell 7, optional but preferred
- Microsoft 365 or desktop Microsoft Office with Excel, PowerPoint, and Word
- Codex, if development will continue on the Windows machine

Clone the repo normally and run:

```powershell
go test ./...
```

## Office Open Check

The first Windows oracle is a COM open check:

```powershell
pwsh -File tools/windows-office-oracle.ps1 `
  -RepoRoot . `
  -InputFile .\testdata\xlsx\minimal-workbook\workbook.xlsx,.\testdata\pptx\minimal-title\presentation.pptx `
  -OutputDir .\office-oracle-proof
```

The script opens each file read-only in the matching desktop Office app and
writes:

- `summary.json`
- `results.jsonl`

It exits non-zero if any file fails to open. Macro execution is disabled by
default; this is a package/XML repair check, not a macro execution harness.
Each file open runs in a bounded child PowerShell process; use
`-TimeoutSeconds` to tune the per-file COM-open timeout when Office is slow to
start.

## Edit Smoke Gate

For routine development, use the fastest gate that proves what you need:

```powershell
make check-office-schema  # strict validation + Open XML SDK, skips Office COM
make check-office-com     # full desktop Word/Excel/PowerPoint open proof
make check-release-fast   # verify + schema smoke + conformance, skips Office COM
make check-release-slow   # verify + schema smoke + conformance + Office COM
```

If `make` is not installed, run the Go `verify` equivalent first:

```powershell
$gofmt = gofmt -l (git ls-files '*.go')
if ($gofmt) { $gofmt; exit 1 }
go vet ./...
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
go test ./...
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -RequireOpenXmlSdk -RunConformance -SkipOffice
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -OfficeOracleTimeoutSeconds 120 -RequireOpenXmlSdk -RunConformance
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -RequireOpenXmlSdk -SkipOffice -EnableVbaObjectModelAccess
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -RequireOpenXmlSdk -EnableVbaObjectModelAccess -OfficeOracleTimeoutSeconds 120
```

The edit and VBA smoke scripts build the Go CLI by default. To prove an
already-built Rust binary or any other external subject, pass both
`-BinaryPath <path-to-ooxml.exe>` and `-SkipBuild`; the scripts refuse to build
Go into an explicit `-BinaryPath`.

This script builds `ooxml`, runs representative XLSX, PPTX, and DOCX mutations,
runs `ooxml validate --strict` and Microsoft Open XML SDK schema validation on
every edited output. Release gates also run repair conformance checks.
`check-office-com` and `check-release-slow` then call
`tools/windows-office-oracle.ps1` on those outputs. Its `summary.json` records
per-scenario stage results and the final proof level. A scenario reaches
`microsoft-office-com-open` only after the matching desktop Office application
opens the edited file without repair/failure.

Use `tools/windows-office-vba-smoke.ps1` for macro-enabled XLSM/PPTM proof. It
temporarily enables the per-user Office VBOM access flag while generating
Office-native seeds from `.bas`/`.cls` source, restores the previous registry
state afterward, proves package-level `vbaProject.bin` attach/remove and
existing-module replacement, confirms real Office-shaped add/remove are refused
before writing output, and optionally opens the macro-enabled outputs through
Excel and PowerPoint COM. Macro execution is disabled in the open oracle.

For a suspected modal repair prompt, rerun visibly:

```powershell
pwsh -File tools/windows-office-oracle.ps1 `
  -RepoRoot . `
  -InputFile .\path\to\suspect.xlsx `
  -OutputDir .\office-oracle-proof `
  -Visible
```

If Office repairs the file, save the repaired copy next to the original and
bring back:

- the original file
- the repaired file
- `summary.json`
- `results.jsonl`
- a short note saying which Office app and version showed the repair

That pair becomes the fastest way to add a focused regression test.

## Codex Loop On Windows

The useful loop is:

1. Reproduce the repair with `tools/windows-office-oracle.ps1`.
2. Inspect the broken package with `ooxml inspect` and strict validation.
3. Fix the writer or mutation path that emitted invalid OOXML.
4. Add a focused regression fixture or golden artifact.
5. Run `go test ./...`.
6. Run the Windows Office oracle again on the fixed output.

Do not start by automating Office UI clicks. The high-value signal is whether
real Office opens the file cleanly, and when it does not, the exact original vs
repaired package diff.

## VBA Note

This harness intentionally disables macro execution. VBA authoring and package
round-trip checks belong in normal repo tests and in
`tools/windows-office-vba-smoke.ps1`; executing macros is a separate, explicit
Windows workflow because it changes the security and environment model.
