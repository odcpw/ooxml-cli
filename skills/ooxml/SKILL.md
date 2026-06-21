---
name: ooxml
description: >-
  Use the Rust ooxml CLI to inspect, edit, validate, and prove Office Open XML
  files. Use for PPTX/PPTM, XLSX/XLSM, DOCX/DOCM, VBA macro packages,
  PresentationML, SpreadsheetML, WordprocessingML, DrawingML, OPC packages,
  relationships, content types, rendering, Office proof, and repo work on
  ooxml-cli itself.
---

# OOXML CLI Workbench

Use this skill when an agent needs to work with Office Open XML files or the
`ooxml-cli` repo. Treat the Rust binary as the source of truth. The skill gives
the operating loop; `ooxml --json capabilities` gives the live command contract.

```text
build or resolve ooxml
-> read focused capabilities/help
-> inspect the package
-> choose CLI-published handles
-> mutate with semantic commands
-> run generated proof commands
-> validate, read back, render, or open-check
```

## Product State

- Rust is the product path.
- The Go code under `go/` is reference material only.
- Pure Rust VBA authoring is the preferred macro path for XLSM, PPTM, and DOCM.
- Desktop Office COM is a proof oracle and legacy helper, not a core dependency.
- The canonical distributable agent skill is this file: `skills/ooxml/SKILL.md`.

## Choose Runtime Mode

Use the mode that matches the tools available in the current agent environment:

| Mode | Use when | Main rule |
|---|---|---|
| CLI mode | The agent has shell access to the repo or to an installed `ooxml` binary. | Run `ooxml` commands directly and write changed files with `--out`. |
| Flue web mode | The agent has thread-scoped tools such as `get_thread_status`, `inspect_current_with_ooxml`, and `apply_ooxml_ops_to_current`. | Work through the provided tools and publish new document versions. |

Do not mix modes blindly. In the web app, never ask the user for filesystem
paths and never invent local paths. In CLI mode, do not assume web-only tools
exist.

## Flue Web Mode

Use this mode for the bundled web workbench. The app owns the upload library,
current document selection, output paths, validation, and version publishing.

1. Call `get_thread_status` to see uploaded documents, the selected document,
   and the current version.
2. Use `select_document` only when the user clearly asks to work on a different
   uploaded file.
3. Use `get_ooxml_capabilities` with a focused filter such as `pptx`, `xlsx`,
   `docx`, `vba`, `chart`, `slide`, `shape`, `table`, `range`, or `style`.
   Request full details only when the compact index is insufficient.
4. Use `get_ooxml_command_help` for exact flag syntax.
5. For reads, use `inspect_current_with_ooxml` with command words and a JSON
   flag object. The app supplies the current file through `ooxml serve`.
6. For generic mutations, use `apply_ooxml_ops_to_current` with commands where
   `opCompatible=true`. Do not include file, out, in-place, dry-run, or
   no-validate args; the app owns those.
7. Include `expectedDocumentId` and `expectedVersionId` when a mutation tool
   accepts them, so a changed selection fails instead of editing the wrong file.
8. Use convenience tools, such as `replace_text_in_current_document`,
   `set_current_presentation_slide_shape_text`,
   `apply_template_to_current_document`, and
   `create_template_form_slide_from_current`, only when they match the task.
9. Validate after changes when the mutation result did not already include
   strict validation.
10. For PPTX/PPTM, render a preview when visual feedback matters.

Flue boundaries:

- Every change publishes a new immutable version. Do not edit in place.
- Browser preview is PPTX/PPTM-focused unless the app advertises more.
- If a requested structural edit has no tool support, state the missing
  capability and suggest the smallest tool or CLI command that would cover it.

## Cold Start

Inside the repo, build or run the local Rust binary. Do not rely on a stale
`ooxml` found earlier on `PATH`.

PowerShell:

```powershell
$env:CARGO_TARGET_DIR = "$env:TEMP\ooxml-target"
cargo build --bin ooxml
$targetDir = (cargo metadata --format-version 1 --no-deps | ConvertFrom-Json).target_directory
$env:OOXML_BIN = Join-Path $targetDir "debug\ooxml.exe"
function oox { & $env:OOXML_BIN @args }
oox version
oox --json capabilities
oox --json doctor
```

Bash:

```bash
export CARGO_TARGET_DIR="${TMPDIR:-/tmp}/ooxml-target"
cargo build --bin ooxml
TARGET_DIR="$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
OOXML_BIN="$TARGET_DIR/debug/ooxml"
oox() { "$OOXML_BIN" "$@"; }
oox version
oox --json capabilities
oox --json doctor
```

`doctor` can report useful warnings, such as missing LibreOffice or an
unconfigured PATH binary. Read the findings and continue with the resolved local
binary when it works.

## Discovery

Use focused capability filters before guessing command syntax:

```bash
oox --json capabilities --for package
oox --json capabilities --for slide
oox --json capabilities --for shape
oox --json capabilities --for range
oox --json capabilities --for table
oox --json capabilities --for chart
oox --json capabilities --for docx
oox --json capabilities --for vba
oox help pptx
oox help xlsx
oox help docx
oox help vba
```

After a mutation, inspect the JSON result for generated proof commands such as
`validateCommand`, `readbackCommand`, `conformanceCommand`, `renderCommand`, or
`officeCheckCommand`. Run the generated commands when they apply.

## Rules For Agents

- Use `--json` for reads and mutations.
- Treat stdout as data and stderr as diagnostics.
- Prefer semantic commands over raw ZIP/XML edits.
- Use handles returned by the CLI: selectors, ids, names, paths, hashes, and
  generated command fields.
- Mutate with `--out <new-file>` unless the user explicitly asked for in-place
  editing.
- Use guards such as `--expect-*`, `--confirm-*`, `--dry-run`, or `--plan` when
  the command provides them.
- Validate every changed package with `oox validate --strict <file>`.
- Run `oox --json conformance check <file>` when package wiring or OOXML
  compatibility matters.
- Render PPTX/PPTM when layout or appearance matters.
- Do not claim Office compatibility from validators alone. Use Office, Open XML
  SDK, or LibreOffice proof appropriate to the claim.
- If docs, memory, and live capabilities disagree, trust the binary after one
  fresh build.

## Proof Matrix

| Claim | Minimum proof | Stronger proof |
|---|---|---|
| Package is structurally valid | `oox validate --strict <file>` | Open XML SDK schema gate |
| Package wiring is coherent | `oox --json conformance check <file>` | release/schema smoke |
| PPTX layout changed correctly | readback plus render | Office or LibreOffice open proof |
| Office opens the package | `office-check` or Windows Office gate | `make check-office-*` |
| A generated XLSM macro executes | `oox --json vba run-smoke` | Windows VBA smoke gate |

Always name what remains unproven. For example, Office open proof does not prove
arbitrary macro safety or execution.

## Common Workflows

### Inspect And Batch Edit

```bash
oox --json inspect file.pptx
oox --json find "Acme Corp" file.pptx
oox --json find "Acme Corp" file.pptx --replace "New Co" --to-ops > ops.json
oox apply file.pptx --ops ops.json --out edited.pptx
oox --json validate --strict edited.pptx
```

### PPTX / PPTM

```bash
oox --json pptx scaffold deck.pptx --title "Deck" --force
oox --json pptx slides list deck.pptx
oox --json pptx shapes show deck.pptx --slide 1 --include-text --include-bounds
oox --json pptx shapes get deck.pptx --slide 1 --target title --include-text
oox --json pptx replace text deck.pptx --slide 1 --target title --text "New title" --out edited.pptx
oox --json pptx tables set-cell deck.pptx --slide 1 --target table:1 --row 1 --col 1 --text Value --out edited.pptx
oox --json pptx place image deck.pptx --slide 1 --image hero.png --x 0 --y 0 --cx 4000000 --cy 2250000 --fit-mode cover --out edited.pptx
oox --json validate --strict edited.pptx
oox pptx render edited.pptx --out render-check
```

Use returned slide, shape, placeholder, table, layout, and master selectors.
PPTX coordinates are EMUs.

### XLSX / XLSM

```bash
oox --json xlsx scaffold workbook.xlsx --force
oox --json xlsx sheets list workbook.xlsx
oox --json xlsx ranges export workbook.xlsx --sheet Sheet1 --range A1:D10 --include-types
oox --json xlsx cells set workbook.xlsx --sheet Sheet1 --cell A1 --value "Hello" --out edited.xlsx
oox --json xlsx tables create edited.xlsx --sheet Sheet1 --range A1:D10 --table Sales --out tabled.xlsx
oox --json xlsx conditional-formats add tabled.xlsx --sheet Sheet1 --range D2:D10 --type cell-is --operator greaterThan --formula 100 --out formatted.xlsx
oox --json validate --strict formatted.xlsx
```

Treat workbooks as structured data, not CSV. Prefer table, range, and cell
commands with stale-source guards and readback fields.

### DOCX / DOCM

```bash
oox --json docx scaffold report.docx --text "Draft report" --force
oox --json docx blocks report.docx
oox --json docx styles list report.docx
oox --json docx tables show report.docx --table 1
oox --json docx replace report.docx --find "Draft" --replace "Final" --expect-count 1 --out final.docx
oox --json validate --strict final.docx
```

Use DOCX commands when the user asks for documents or when a shared OOXML
workflow applies across formats.

## VBA And Macros

Use pure Rust authoring first:

```bash
oox --json xlsx scaffold workbook.xlsx --force
oox --json vba build-bin --family xlsx --source macros/Module1.bas --out vbaProject.bin
oox --json vba attach workbook.xlsx --bin vbaProject.bin --out workbook.xlsm
oox --json vba create workbook.xlsx --pure --family xlsx --source macros/Module1.bas --out workbook.xlsm
oox --json vba rebuild workbook.xlsm --source-dir macros --out rebuilt.xlsm
oox --json vba list workbook.xlsm
oox --json vba extract workbook.xlsm --out-dir macros
oox --json validate --strict workbook.xlsm
oox --json conformance check workbook.xlsm
```

Use the same shape for PowerPoint and Word:

```bash
oox --json pptx scaffold deck.pptx --title "Macro Deck" --force
oox --json vba create deck.pptx --pure --family pptx --source macros/Module1.bas --out deck.pptm

oox --json docx scaffold document.docx --text "Macro document" --force
oox --json vba create document.docx --pure --family docx --source macros/Module1.bas --source macros/Worker.cls --out document.docm
```

For Office open proof:

```bash
oox --json vba office-check workbook.xlsm
oox --json vba office-check deck.pptm
oox --json vba office-check document.docm
```

For explicit XLSM macro execution proof on Windows:

```powershell
oox --json vba run-smoke --timeout-seconds 45 --out-dir .\proof\xlsm-run-smoke
oox --json vba run-smoke --smoke-mode Class --timeout-seconds 45 --out-dir .\proof\xlsm-class-run-smoke
```

VBA limits:

- `vba create --pure`, `vba build-bin`, `vba attach`, `vba rebuild`,
  `vba list`, and `vba extract` are the normal macro workflows.
- `vba add-module`, `replace-module`, and `remove-module` are guarded and are
  not the preferred path for Office-shaped projects.
- Macro execution automation, VBE compile proof, signatures, UserForms, and
  password/protection editing are not general features.
- `run-smoke` is the explicit local proof harness for harmless generated XLSM
  macros. It runs Excel and should only be used when macro execution is wanted.

## Repo Work

When improving `ooxml-cli`:

1. Check `git status` and keep unrelated user changes.
2. Reproduce the awkward command or missing capability through the CLI boundary.
3. Patch behavior, capabilities, help, robot docs, tests, or this skill where the
   fix belongs.
4. Add a command-path test when the user-visible contract changes.
5. Run the focused command, generated proof commands, and relevant tests.
6. Update docs only when the workflow or binary contract changed.

Good improvement targets:

- commands returning durable handles and generated proof commands
- mutation JSON containing readback, validation, conformance, render, or open
  proof commands
- capabilities and help routing agents to the right command without source
  reading
- validation diagnostics that explain the broken package part and repair path
- pure Rust VBA authoring staying deterministic and honest about unsupported
  features

Avoid broad refactors unless the tests are green and the simplification is
measurable.

## Verification

Focused Rust loop:

```bash
cargo fmt --check
cargo check --all-targets
cargo test --test rust_contract_smoke '<focused-filter>' -- --nocapture
```

Broader local loop:

```bash
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
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

Run Go reference targets only when intentionally refreshing or comparing the
legacy oracle.

## Reference Docs

Use repo-local docs first:

- `README.md`
- `docs/vba-macro-support.md`
- `docs/testing-strategy.md`
- `docs/windows-office-oracle.md`
- `docs/layout-authoring.md`
- `docs/placeholder-key-rules.md`
- `docs/translation-id-rules.md`

Use official Microsoft/Open XML references when touching format internals,
especially MS-OVBA, MS-CFB, OPC, ISO/IEC 29500 notes, and Open XML SDK docs.

## Final Evidence

Report concrete evidence:

- changed artifact paths or commit hash
- exact commands used
- validation, conformance, readback, render, or Office proof result
- known limitations
- the next useful slice when work remains
