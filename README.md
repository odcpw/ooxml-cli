# ooxml-cli

[![CI](https://github.com/odcpw/ooxml-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/odcpw/ooxml-cli/actions/workflows/ci.yml)

`ooxml-cli` is a Rust CLI for inspecting, editing, validating, and automating Office Open XML files. It is built for agent workflows: commands return stable JSON handles, safe mutation paths, validation/readback commands, and useful failure hints instead of forcing raw ZIP/XML surgery.

Rust is the current/default product path. The old Go implementation is deprecated for product development and kept only as a legacy oracle/reference for parity checks and historical behavior.

Supported families:

- PowerPoint: `.pptx` / `.pptm`
- Excel: `.xlsx` / `.xlsm`
- Word: `.docx` / `.docm` where implemented
- VBA macro packages via `vbaProject.bin`

## Install

```powershell
git clone https://github.com/odcpw/ooxml-cli.git
cd ooxml-cli
cargo build --bin ooxml
.\target\debug\ooxml.exe version
```

During repo development, prefer the local runner so an old installed binary does not shadow your changes:

```powershell
cargo run -- --help
cargo run -- --json capabilities
cargo run -- doctor
```

## Everyday Commands

```powershell
# Inspect and validate
ooxml --json inspect deck.pptx
ooxml validate --strict deck.pptx

# Find stable edit targets
ooxml --json find "Acme Corp" deck.pptx
ooxml --json find "Acme Corp" deck.pptx --replace "New Co" --to-ops

# Apply many edits atomically
ooxml apply deck.pptx --ops ops.json --out edited.pptx

# PowerPoint read/mutate/read back
ooxml --json pptx slides list deck.pptx
ooxml --json pptx shapes show deck.pptx --slide 1 --include-text --include-bounds
ooxml --json pptx replace text deck.pptx --slide 1 --target title --text "New title" --out edited.pptx

# Excel structured data
ooxml --json xlsx sheets list workbook.xlsx
ooxml --json xlsx ranges export workbook.xlsx --sheet Sheet1 --range A1:D10 --include-types
ooxml --json xlsx tables append-records workbook.xlsx --table Sales --records-file rows.json --expect-range A1:D20 --out edited.xlsx

# DOCX business documents
ooxml --json docx blocks list report.docx
ooxml --json docx replace report.docx --find "Draft" --replace "Final" --expect-count 3 --out edited.docx
```

## VBA And Macro Files

The safe VBA path is:

1. For a new macro file, write `.bas` / `.cls` modules and run `ooxml vba create`.
2. For an existing package, create or obtain an Office-authored `vbaProject.bin` and attach it with `ooxml vba attach`.
3. Use `vba list` / `vba extract` for source readback.
4. Use `vba replace-module` only for existing modules, with a source hash guard.

```powershell
ooxml --json vba create workbook.xlsm --family xlsx --source .\macros\Module1.bas --source .\macros\Worker.cls --extract-bin .\out\vbaProject.bin --enable-vba-object-model-access --force
ooxml --json vba inspect workbook.xlsm
ooxml --json vba extract-bin workbook.xlsm --out vbaProject.bin
ooxml --json vba inspect-bin vbaProject.bin --family xlsx
ooxml --json vba attach workbook.xlsx --bin vbaProject.bin --out workbook.xlsm
ooxml --json vba list workbook.xlsm
ooxml --json vba extract workbook.xlsm --out-dir macros
ooxml --json vba replace-module workbook.xlsm --module module:SeedModule --source .\macros\SeedModule.bas --expect-sha256 <sha256> --allow-experimental-vba-source-rewrite --out edited.xlsm
```

On Windows with desktop Office installed, `vba create` drives the repo helper below. Agents should normally call the CLI command, not the script directly:

```powershell
ooxml --json vba create .\out\seed.xlsm --family xlsx --source .\macros\Module1.bas --source .\macros\Worker.cls --extract-bin .\out\vbaProject.bin --enable-vba-object-model-access --force

ooxml --json vba attach .\testdata\xlsx\minimal-workbook\workbook.xlsx --bin .\out\vbaProject.bin --out .\out\workbook.xlsm
```

Real Office-shaped module add/remove is intentionally refused today because Office stores version-dependent `_VBA_PROJECT` module metadata. Use `vba create` or an Office-authored seed plus `vba attach` when the module set must change. Macro execution, VBE compile, signatures, forms, and password/protection editing are out of scope.

## Verification

Fast local loop:

```powershell
$env:CARGO_PROFILE_DEV_DEBUG = "0"
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Run Go only when deliberately refreshing or checking the legacy oracle/reference, not as the normal product build.

Windows Office proof gates for the Rust CLI:

```powershell
cargo build --bin ooxml
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -BinaryPath .\target\debug\ooxml.exe -SkipBuild -MutationParallelism 4 -RequireOpenXmlSdk -RunConformance -SkipOffice -WriteArtifactProofMatrix
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -BinaryPath .\target\debug\ooxml.exe -SkipBuild -MutationParallelism 4 -OfficeOracleTimeoutSeconds 120 -RequireOpenXmlSdk -RunConformance -WriteArtifactProofMatrix
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -BinaryPath .\target\debug\ooxml.exe -SkipBuild -RequireOpenXmlSdk -SkipOffice -EnableVbaObjectModelAccess
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -BinaryPath .\target\debug\ooxml.exe -SkipBuild -RequireOpenXmlSdk -EnableVbaObjectModelAccess -OfficeOracleTimeoutSeconds 120
```

The edit-smoke commands emit a command-by-command proof ledger next to the smoke `summary.json`. Add `-FailOnArtifactProofGap` only for a full release gate once every public mutator has structural, readback, strict validation, conformance, and required Office-open evidence.

The first and third commands are schema/Open XML SDK gates without Office COM. The second and fourth add desktop Word/Excel/PowerPoint open proof.

Focused checks are usually enough while developing one command family:

```powershell
cargo test --test rust_contract_smoke <filter> -- --nocapture
cargo test <module_filter> --bin ooxml -- --nocapture
```

The strongest current proof level is `microsoft-office-com-open`: desktop Office opened the edited file without repair/failure. This does not execute macros.

## Docs

- `skills/ooxml/SKILL.md`: agent-facing operating guide
- `docs/rust-port-status.md`: Rust default path and legacy Go oracle status
- `docs/vba-macro-support.md`: VBA implementation status and limits
- `docs/testing-strategy.md`: fixture and proof-gate strategy
- `docs/windows-office-oracle.md`: Windows Office COM open oracle
- `docs/layout-authoring.md`: PowerPoint layout authoring workflow
- `docs/placeholder-key-rules.md`: stable PowerPoint placeholder keys
- `docs/translation-id-rules.md`: stable translation identifiers

## Design Rules

- Stdout is data; diagnostics go to stderr.
- Prefer `--json` for agent-facing reads and mutations.
- Mutations require `--out`, `--in-place`, or `--dry-run`.
- Use CLI-published handles instead of guessed XML paths.
- Validate every changed package.
- Treat Office-open proof as stronger than package validation for compatibility claims.
