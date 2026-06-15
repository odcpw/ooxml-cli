# ooxml-cli

[![CI](https://github.com/odcpw/ooxml-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/odcpw/ooxml-cli/actions/workflows/ci.yml)

`ooxml-cli` is a Go CLI for inspecting, editing, validating, and automating Office Open XML files. It is built for agent workflows: commands return stable JSON handles, safe mutation paths, validation/readback commands, and useful failure hints instead of forcing raw ZIP/XML surgery.

Supported families:

- PowerPoint: `.pptx` / `.pptm`
- Excel: `.xlsx` / `.xlsm`
- Word: `.docx` / `.docm` where implemented
- VBA macro packages via `vbaProject.bin`

## Install

```powershell
git clone https://github.com/odcpw/ooxml-cli.git
cd ooxml-cli
go build -o ooxml.exe .\cmd\ooxml
.\ooxml.exe version
```

During repo development, prefer the local runner so an old installed binary does not shadow your changes:

```powershell
go run .\cmd\ooxml --help
go run .\cmd\ooxml capabilities --json
go run .\cmd\ooxml doctor
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
go vet ./...
go test ./...
```

If `make` is installed, `make verify` runs the same local gate. `make verify-strict` also enforces repo-wide gofmt.

Windows Office proof gates:

```powershell
make check-office-schema      # strict validation + Open XML SDK, skips Office COM
make check-office-com         # Word/Excel/PowerPoint desktop open proof
make check-office-vba-schema  # Office-authored VBA seeds + strict/Open XML SDK
make check-office-vba-com     # VBA attach/remove/replace + Excel/PowerPoint open proof
make check-release-fast       # verify + schema smoke + conformance, skips COM
make check-release-slow       # verify + schema smoke + conformance + COM + VBA smoke
```

Without `make`:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -RequireOpenXmlSdk -RunConformance -SkipOffice
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -OfficeOracleTimeoutSeconds 120 -RequireOpenXmlSdk -RunConformance
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -RequireOpenXmlSdk -SkipOffice -EnableVbaObjectModelAccess
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -RequireOpenXmlSdk -EnableVbaObjectModelAccess -OfficeOracleTimeoutSeconds 120
```

The strongest current proof level is `microsoft-office-com-open`: desktop Office opened the edited file without repair/failure. This does not execute macros.

## Docs

- `skills/ooxml/SKILL.md`: agent-facing operating guide
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
