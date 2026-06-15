---
name: ooxml
description: >-
  Use the ooxml CLI as a Codex-native workbench for Office Open XML artifacts
  and for improving the ooxml-cli repo. Use for PPTX/PPTM, XLSX/XLSM,
  DOCX/DOCM, VBA macro packages, PresentationML, SpreadsheetML,
  WordprocessingML, DrawingML, OPC packages, relationships, content types,
  validation, rendering, and cross-format automation.
---

# OOXML Workbench

Use this skill whenever Office Open XML files or the `ooxml-cli` repo are in scope. The working loop is:

```text
resolve runner -> inspect -> discover handles -> mutate semantically
-> validate -> read back/render/open-check -> report evidence or improve the CLI
```

## One Rule

Make the first reasonable command an agent tries either work or return the exact next command it should use. Prefer improving the CLI when a reliable workflow needs a missing handle, readback, guard, or error hint.

## Runner Discipline

Inside the repo, prefer the repo runner until the installed binary is proven current.

PowerShell:

```powershell
function oox { go run ./cmd/ooxml @args }
oox version
oox capabilities --json
oox robot-docs guide
```

Bash:

```bash
oox() { go run ./cmd/ooxml "$@"; }
oox version
oox capabilities --json
oox robot-docs guide
```

Outside the repo, use `ooxml` only after `ooxml doctor` says the PATH binary is not stale.

## Scope Router

- `pptx` / `pptm`: slides, layouts, masters, shapes, text, images, tables, charts, themes, notes, rendering, and visual verification.
- `xlsx` / `xlsm`: sheets, ranges, cells, formulas, tables, pivots, charts, comments, workbook metadata, and structured automation data.
- `docx` / `docm`: blocks, paragraphs, tables, styles, headers/footers, images, comments, and business-document find/replace.
- `vba`: macro project presence, `vbaProject.bin` attach/extract/remove, source list/extract, existing-module replacement, and Office-authored seed creation.
- shared OPC/OOXML: package detection, relationships, content types, validation, repair hints, and cross-format bridges.

For cross-format work, start with the source data, then the destination target, then shared validation.

## Agent Contract

Read commands should return:

- exact scope: file, slide, sheet, range, table, module, relationship, or part
- stable identity: `primarySelector`, `selectors`, ids, names, paths, hashes
- typed payloads: bounds, dimensions, formulas, cell types, text runs, media hashes, relationship targets
- generated next commands for likely follow-up work

Mutation commands should return:

- `file`, `output`, `dryRun`, and source/destination metadata
- stale-source guards checked or accepted
- changed-object readback when practical
- `readbackCommand`, `validateCommand`, and render/export/open-check commands where useful
- dry-run templates using `<out.pptx>`, `<out.xlsx>`, `<out.docx>`, or the macro-enabled extension

Errors should return:

- what failed and which input was invalid
- accepted selector/range/value syntax
- a copy-pasteable discovery, correction, or safe alternative command

## Non-Negotiables

- Stdout is data; diagnostics belong on stderr.
- Use `--json` for agent-facing reads and mutations.
- Prefer semantic commands over raw ZIP/XML edits.
- Prefer CLI-published handles over guessed part names.
- Mutate with `--out <new-file>` unless the user asked for `--in-place`.
- Use `--dry-run`, `--plan`, `--expect-*`, or `--confirm-*` for risky paths.
- Validate every changed package with `oox validate --strict <file>`.
- Render PPTX when visual placement, layout, or appearance matters.
- Treat package validation as necessary but not sufficient for Office compatibility claims.

## Common Workflows

### Discover

```bash
oox --json inspect file.pptx
oox --json find "Acme Corp" file.pptx
oox --json find "Acme Corp" file.pptx --replace "New Co" --to-ops
oox apply file.pptx --ops ops.json --out edited.pptx
```

### PPTX / PPTM

```bash
oox --json pptx slides list deck.pptx
oox --json pptx slides show deck.pptx --slide 1 --include-text --include-bounds
oox --json pptx shapes show deck.pptx --slide 1 --include-text --include-bounds
oox --json pptx replace text deck.pptx --slide 1 --target title --text "New title" --out edited.pptx
oox --json pptx tables set-cell deck.pptx --slide 1 --target table:1 --row 1 --col 1 --text Value --out edited.pptx
oox --json pptx place image deck.pptx --slide 1 --image hero.png --x 0 --y 0 --cx 4000000 --cy 2250000 --fit-mode cover --out edited.pptx
oox validate --strict edited.pptx
oox pptx render edited.pptx --out render-check
```

Use returned slide, shape, placeholder, table, layout, and master selectors. For deck-wide rebrands, dry-run `pptx replace text-occurrences` first, then use the returned plan hash with `--expect-plan-hash`.

### XLSX / XLSM

```bash
oox --json xlsx sheets list workbook.xlsx
oox --json xlsx ranges export workbook.xlsx --sheet Sheet1 --range A1:D10 --include-types
oox --json xlsx tables show workbook.xlsx --table Sales
oox --json xlsx tables append-records workbook.xlsx --table Sales --records-file rows.json --expect-range A1:D20 --out edited.xlsx
oox --json xlsx charts show workbook.xlsx --chart chart:1
oox --json xlsx pivots show workbook.xlsx --pivot pivot:1
oox validate --strict edited.xlsx
```

Treat workbooks as structured data, not lossy CSV. Prefer table/range/cell commands with stale-source guards.

### DOCX / DOCM

```bash
oox --json docx blocks list report.docx
oox --json docx styles list report.docx
oox --json docx tables show report.docx --table 1
oox --json docx replace report.docx --find "Draft" --replace "Final" --expect-count 3 --out edited.docx
oox validate --strict edited.docx
```

DOCX is secondary to PPTX/XLSX/VBA unless the user directly asks for documents or a shared abstraction helps.

## VBA And Macros

Current safe target: package-level macro handling plus source module inspection/extraction and guarded replacement of existing parseable modules.

```bash
oox --json vba inspect workbook.xlsm
oox --json vba extract-bin workbook.xlsm --out vbaProject.bin
oox --json vba inspect-bin vbaProject.bin --family xlsx
oox --json vba attach workbook.xlsx --bin vbaProject.bin --out workbook.xlsm
oox --json vba list workbook.xlsm
oox --json vba extract workbook.xlsm --out-dir macros
oox --json vba replace-module workbook.xlsm --module Module1 --source macros/Module1.bas --expect-sha256 <sha256-from-list> --allow-experimental-vba-source-rewrite --out edited.xlsm
oox validate --strict edited.xlsm
oox --json vba office-check edited.xlsm
```

For new `.xlsm` / `.pptm` files from `.bas` / `.cls` sources on Windows, use desktop Office to author the macro project, then optionally attach the extracted seed elsewhere:

```powershell
oox --json vba create .\out\seed.xlsm `
  --family xlsx `
  --source .\macros\Module1.bas `
  --source .\macros\Worker.cls `
  --extract-bin .\out\vbaProject.bin `
  --enable-vba-object-model-access `
  --force

oox --json vba attach .\testdata\xlsx\minimal-workbook\workbook.xlsx --bin .\out\vbaProject.bin --out .\out\workbook.xlsm
```

`tools/windows-office-vba-create.ps1` is the backend helper for direct Office COM troubleshooting. Agents should call `oox vba create` first because the CLI validates inputs, discovers the helper, and returns normalized follow-up commands.

VBA truth table:

- `vbaProject.bin` inspect/extract/attach/remove: supported for PPTX/PPTM and XLSX/XLSM.
- `vba list` / `vba extract`: supported for parseable projects.
- `vba replace-module`: supported for existing parseable modules; Windows smoke proves Office-open for Office-generated XLSM/PPTM replacement outputs.
- `vba add-module` / `vba remove-module`: only for synthetic/source-only projects; real Office-shaped projects are intentionally refused because `_VBA_PROJECT` must be regenerated for module-set changes.
- New Office-facing module sets: create or obtain an Office-authored `.xlsm`/`.pptm` or `vbaProject.bin`, then attach it.
- Inline procedure/function editing, macro execution, VBE compile proof, signatures/resigning, forms, and password/protection editing: do not build or promise now.

The Windows VBA smoke gate proves package attach/extract/remove and existing-module replacement with strict validation, Open XML SDK validation, and desktop Excel/PowerPoint COM open. It does not execute or compile macros.

## Improving The Repo

Read `GOAL.md`, inspect live capabilities/help, choose one useful slice, implement through the CLI boundary, add command-path tests, update only contract docs that reduce confusion, then run verification.

Highest leverage usually ranks this way:

1. Existing commands return durable handles and generated next commands.
2. Mutation JSON includes readback and validation/open-check commands.
3. Command-path tests execute generated commands.
4. Safe semantic mutations cover common PPTX/XLSX editing jobs.
5. Validation diagnostics explain package, relationship, content-type, and VBA consistency problems with exact repair hints.
6. VBA seed/create workflows stay Office-authored unless `_VBA_PROJECT` regeneration is implemented and proven.

Avoid broad refactors unless the baseline is green and a scored isomorphism case shows real simplification value. Do not de-monolithize by taste.

## Verification Gates

Repo work:

```bash
go test -count=1 ./internal/cli -run '<focused-regex>'
go test ./...
go vet ./...
git diff --check
```

Windows Office gates:

```bash
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

Use shared gates when shared surfaces changed:

- CLI dispatch/help/docs contract: help/capabilities checks.
- Package validation: validation-focused tests and fixture checks.
- PPTX visual changes: render representative decks.
- VBA changes: strict validation, Open XML SDK validation, and Office/LibreOffice load evidence before compatibility claims.

## Reference Discipline

Use repo-local docs first:

- `GOAL.md`
- `docs/vba-macro-support.md`
- `docs/testing-strategy.md`
- `docs/windows-office-oracle.md`
- `docs/layout-authoring.md`
- `docs/placeholder-key-rules.md`
- `docs/translation-id-rules.md`

Then use official references when touching format internals: Microsoft MS-OVBA, MS-CFB, Office implementation notes for ISO/IEC 29500, OPC documentation, and Open XML SDK docs.

## Final Evidence

End with the facts a user or future agent can act on:

- changed artifact paths or commit hash
- exact command paths used
- strict validation and readback/open-check result
- known limitations
- next most useful slice
