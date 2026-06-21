---
name: ooxml
description: >-
  Use the Rust ooxml CLI as a Codex-native workbench for Office Open XML
  artifacts and for improving the ooxml-cli repo. Use for PPTX/PPTM,
  XLSX/XLSM, DOCX/DOCM, VBA macro packages, PresentationML, SpreadsheetML,
  WordprocessingML, DrawingML, OPC packages, relationships, content types,
  validation, rendering, Office proof, generated command contracts, and
  cross-format automation.
---

# OOXML Workbench

Use this skill whenever Office Open XML files or the `ooxml-cli` repo are in
scope. Treat the CLI itself as the source of truth: this skill gives the loop,
the decision rules, and the proof discipline; `ooxml --json capabilities` gives
the live command contract.

```text
resolve local runner -> read live capabilities -> inspect package
-> discover stable handles -> mutate semantically -> run generated proof commands
-> validate/read back/render/open-check -> report evidence or improve the CLI
```

## One Rule

Make the first reasonable command an agent tries either work or return the exact
next command it should use. When a reliable workflow needs a missing handle,
readback, guard, generated command, or error hint, prefer improving the CLI
contract over teaching agents to memorize a workaround.

## Cold Start

Inside the repo, use the Rust product binary. Build once, bind a short local
runner, then ask the tool what it can do now. Do not trust stale examples over
live capabilities/help.

PowerShell:

```powershell
cargo build --bin ooxml
$env:OOXML_BIN = (Resolve-Path .\target\debug\ooxml.exe).Path
function oox { & $env:OOXML_BIN @args }
oox version
oox --json capabilities
oox robot-docs guide
oox --json doctor
```

Bash:

```bash
cargo build --bin ooxml
OOXML_BIN="$PWD/target/debug/ooxml"
oox() { "$OOXML_BIN" "$@"; }
oox version
oox --json capabilities
oox robot-docs guide
oox --json doctor
```

`doctor` may exit nonzero for warnings such as "PATH binary missing" or
"LibreOffice unavailable". Read the JSON/text findings and remediation commands;
do not abandon the local `target/debug/ooxml` runner just because PATH is not
configured.

The deprecated Go implementation in `go/` is legacy reference material only. Do
not use it for normal product work, docs, or proofs unless the task explicitly
requires refreshing or comparing a parity oracle.

## Agent-Ergonomic Lessons

Borrow these rules from the agent-ergonomics methodology and apply them to both
the CLI and this skill:

- First-try inevitability: start with commands that either work or teach the
  next command.
- Live self-documentation: use `oox --json capabilities --for <filter>` before
  guessing flags.
- Stable handles: use selectors, ids, names, hashes, and generated command
  fields returned by the CLI; avoid raw ZIP/XML paths until the semantic surface
  is missing.
- Stdout is data: use `--json`; diagnostics and failures belong on stderr.
- Generated proof commands win: when mutation JSON contains `validateCommand`,
  `readbackCommand`, `renderCommand`, `officeCheckCommand`, or
  `conformanceCommand`, run those exact commands.
- Safe mutation: prefer `--out <new-file>`; use `--in-place` only when the user
  explicitly asks and a backup/guard exists.
- Drift resistance: if this skill's example conflicts with capabilities/help,
  capabilities/help wins and the skill should be fixed.

## Live Discovery Router

Use focused capability filters to reduce guesswork:

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
oox help vba
```

Route work by object kind:

- `package`: inspect, validate, repair, diff, apply ops, conformance.
- `pptx` / `pptm`: slides, layouts, masters, shapes, text, images, tables,
  charts, themes, notes, rendering, visual verification.
- `xlsx` / `xlsm`: sheets, ranges, cells, formulas, tables, pivots, charts,
  comments, workbook metadata, structured automation data.
- `docx` / `docm`: blocks, paragraphs, tables, styles, headers/footers, images,
  comments, business-document find/replace.
- `vba`: pure Rust macro authoring, `vbaProject.bin` build/attach/extract,
  source list/extract/rebuild, Office/open proof, explicit XLSM run smoke.
- shared OPC/OOXML: relationships, content types, package type conversion,
  validation and repair hints, cross-format bridges.

For cross-format work, inspect the source data first, then the destination
target, then use a semantic bridge or `apply` ops, then validate/read back both
the package and the changed object.

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

If a command cannot provide this contract, that is a CLI improvement target.

## Non-Negotiables

- Stdout is data; diagnostics belong on stderr.
- Use `--json` for agent-facing reads and mutations.
- Prefer semantic commands over raw ZIP/XML edits.
- Prefer CLI-published handles over guessed part names.
- Mutate with `--out <new-file>` unless the user asked for `--in-place`.
- Use `--dry-run`, `--plan`, `--expect-*`, or `--confirm-*` for risky paths.
- Validate every changed package with `oox validate --strict <file>`.
- Run `oox --json conformance check <file>` when package wiring or compatibility
  semantics matter.
- Render PPTX when visual placement, layout, or appearance matters.
- Treat validation as necessary but not sufficient for Office compatibility
  claims; use Office/Open XML SDK/LibreOffice proof appropriate to the claim.

## Common Workflows

### Discover And Batch Edit

```bash
oox --json capabilities --for package
oox --json inspect file.pptx
oox --json find "Acme Corp" file.pptx
oox --json find "Acme Corp" file.pptx --replace "New Co" --to-ops
oox apply file.pptx --ops ops.json --out edited.pptx
```

### PPTX / PPTM

```bash
oox --json capabilities --for slide
oox --json pptx slides list deck.pptx
oox --json pptx slides selectors deck.pptx --slide 1
oox --json pptx slides show deck.pptx --slide 1 --include-text --include-bounds
oox --json pptx shapes show deck.pptx --slide 1 --include-text --include-bounds
oox --json pptx shapes get deck.pptx --slide 1 --target title --include-text --include-bounds
oox --json pptx replace text deck.pptx --slide 1 --target title --text "New title" --out edited.pptx
oox --json pptx tables set-cell deck.pptx --slide 1 --target table:1 --row 1 --col 1 --text Value --out edited.pptx
oox --json pptx place image deck.pptx --slide 1 --image hero.png --x 0 --y 0 --cx 4000000 --cy 2250000 --fit-mode cover --out edited.pptx
oox validate --strict edited.pptx
oox pptx render edited.pptx --out render-check
```

Use returned slide, shape, placeholder, table, layout, and master selectors.
PPTX coordinates are EMUs. For deck-wide rebrands, prefer `find --to-ops` or
dry-run/plan style commands before applying broad replacements.

### XLSX / XLSM

```bash
oox --json capabilities --for range
oox --json xlsx sheets list workbook.xlsx
oox --json xlsx ranges export workbook.xlsx --sheet Sheet1 --range A1:D10 --include-types
oox --json xlsx tables show workbook.xlsx --table Sales
oox --json xlsx tables append-records workbook.xlsx --table Sales --records-file rows.json --expect-range A1:D20 --out edited.xlsx
oox --json xlsx charts show workbook.xlsx --chart chart:1
oox --json xlsx pivots show workbook.xlsx --pivot pivot:1
oox validate --strict edited.xlsx
```

Treat workbooks as structured data, not lossy CSV. Prefer table/range/cell
commands with stale-source guards, sheet ids, table names, expected ranges, and
formula-invalidation/readback fields.

### DOCX / DOCM

```bash
oox --json capabilities --for docx
oox --json docx blocks show report.docx
oox --json docx styles list report.docx
oox --json docx tables show report.docx --table 1
oox --json docx replace report.docx --find "Draft" --replace "Final" --expect-count 3 --out edited.docx
oox validate --strict edited.docx
```

DOCX is secondary to PPTX/XLSX/VBA unless the user directly asks for documents or a shared abstraction helps.

## VBA And Macros

Current safe target: pure Rust macro authoring plus package-level wiring and
source readback. The preferred cross-platform path is `build-bin` plus `attach`,
or `vba create --pure` when starting from a host package. Desktop Office COM is
a proof oracle and legacy seed helper, not the core implementation dependency.

```bash
oox --json capabilities --for vba
oox --json xlsx scaffold workbook.xlsx --force
oox --json vba build-bin --family xlsx --source macros/Module1.bas --out vbaProject.bin
oox --json vba attach workbook.xlsx --bin vbaProject.bin --out workbook.xlsm
oox --json vba create workbook.xlsx --pure --family xlsx --source macros/Module1.bas --out workbook.xlsm
oox --json vba rebuild workbook.xlsm --source-dir macros --out rebuilt.xlsm
oox --json vba inspect workbook.xlsm
oox --json vba extract-bin workbook.xlsm --out vbaProject.bin
oox --json vba inspect-bin vbaProject.bin --family xlsx
oox --json vba list workbook.xlsm
oox --json vba extract workbook.xlsm --out-dir macros
oox validate --strict workbook.xlsm
oox --json conformance check workbook.xlsm
oox --json vba office-check workbook.xlsm
```

For PPTM/DOCM, use the same pure-authoring shape after checking capabilities
and family-specific proof status. For an explicit XLSM macro execution proof on
Windows, use the opt-in smoke harness:

```powershell
oox --json vba run-smoke --timeout-seconds 45 --out-dir .\proof\xlsm-run-smoke
oox --json vba run-smoke --smoke-mode Class --timeout-seconds 45 --out-dir .\proof\xlsm-class-run-smoke
```

Legacy Office-COM `vba create` without `--pure` remains useful for
Office-authored seeds and troubleshooting. Agents should still call the CLI
first because it validates inputs, discovers helper scripts, and returns
normalized follow-up commands.

VBA truth table:

- `vbaProject.bin` build/inspect/extract/attach/remove: supported where
  capabilities advertise it.
- `vba create --pure`: preferred cross-platform macro package authoring path for
  proven host families.
- `vba rebuild --source-dir`: preferred module-set replacement path; rebuilds a
  fresh source-only project instead of patching Office-authored binary metadata.
- `vba list` / `vba extract`: supported for parseable projects.
- `vba add-module` / `replace-module` / `remove-module`: not the first-class path
  for Office-shaped projects; expect guards. Use `rebuild`, `create --pure`, or
  opaque `attach` instead unless the task explicitly targets synthetic projects.
- Macro execution automation, VBE compile proof, signatures/resigning, forms,
  and password/protection editing: do not promise. `run-smoke` is the explicit
  local proof harness for harmless generated XLSM macros.

The Windows VBA smoke gate proves generated XLSM/PPTM/DOCM packages with strict
validation, Open XML SDK validation, conformance, and desktop Office open proof.
The explicit run smoke is separate because it executes VBA.

## Improving The Repo

Read `GOAL.md`, `README.md`, and live capabilities/help. Check `git status`
before editing and do not revert unrelated user work. Choose one useful slice,
implement through the CLI boundary, add command-path tests, update capabilities
and robot docs when the command contract changes, then run verification.

For each improvement loop:

1. Capture the agent-hostile moment: stale example, missing command, bad error,
   missing handle, missing generated proof command, or no validation hint.
2. Decide whether the fix belongs in CLI behavior, capabilities/robot docs, this
   skill, or repo docs. Prefer CLI self-documentation for command contracts.
3. Add or update tests that execute the same command path an agent will run.
4. Re-run the focused command, generated proof command, and relevant test.
5. Patch this skill only for stable workflow knowledge, not for every flag.

Highest leverage usually ranks this way:

1. Existing commands return durable handles and generated next/proof commands.
2. Mutation JSON includes readback, validation, conformance, render, or
   open-check commands.
3. Capabilities and robot docs route agents to the right command without reading
   source.
4. Command-path tests execute generated commands, not just helper functions.
5. Safe semantic mutations cover common PPTX/XLSX/DOCX/VBA editing jobs.
6. Validation diagnostics explain package, relationship, content-type, and VBA
   consistency problems with exact repair hints.
7. Pure Rust VBA authoring stays deterministic, proven, and honest about gaps.

Avoid broad refactors unless the baseline is green and a scored isomorphism case
shows real simplification value. Do not de-monolithize by taste.

## Verification Gates

Skill-only changes:

```bash
jsm validate skills/ooxml --offline
git diff --check -- skills/ooxml/SKILL.md
```

Repo work:

```bash
cargo fmt --check
cargo check --all-targets
cargo test --all-targets
cargo test --test rust_contract_smoke '<focused-filter>' -- --nocapture
cargo clippy --all-targets -- -D warnings
git diff --check
```

Run `make go-reference-*` targets only for deliberate legacy oracle refreshes.

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
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -BinaryPath .\target\debug\ooxml.exe -SkipBuild -RequireOpenXmlSdk -RunConformance -SkipOffice -EnableVbaObjectModelAccess
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -BinaryPath .\target\debug\ooxml.exe -SkipBuild -RequireOpenXmlSdk -RunConformance -EnableVbaObjectModelAccess -OfficeOracleTimeoutSeconds 120
.\target\debug\ooxml.exe --json vba run-smoke --timeout-seconds 45 --out-dir .\proof\xlsm-run-smoke
```

Use shared gates when shared surfaces changed:

- CLI dispatch/help/docs contract: help/capabilities checks.
- Package validation: validation-focused tests and fixture checks.
- PPTX visual changes: render representative decks.
- VBA changes: strict validation, conformance, Open XML SDK validation, and
  Office/LibreOffice load evidence before compatibility claims; macro execution
  claims require explicit `vba run-smoke` proof.

## Reference Discipline

Use repo-local docs first:

- `GOAL.md`
- `README.md`
- `docs/vba-macro-support.md`
- `docs/testing-strategy.md`
- `docs/windows-office-oracle.md`
- `docs/layout-authoring.md`
- `docs/placeholder-key-rules.md`
- `docs/translation-id-rules.md`

Then use official references when touching format internals: Microsoft MS-OVBA, MS-CFB, Office implementation notes for ISO/IEC 29500, OPC documentation, and Open XML SDK docs.

## Skill Improvement Loop

This skill should itself be treated as an agent surface. Improve it whenever a
fresh agent would waste time or make a stale guess.

Score it informally on:

- activation: does the description trigger for every OOXML/ooxml-cli task?
- first-run path: can a cold agent get to a working local runner in under one
  minute?
- live discovery: does it force capabilities/help before stale memorized flags?
- task routing: can the agent pick a PPTX/XLSX/DOCX/VBA lane quickly?
- proof discipline: does it name the minimum evidence for the claim?
- drift resistance: does it tell agents what to do when examples conflict with
  live capabilities?

Each pass should land one concrete improvement: remove stale commands, add a
missing live-discovery route, tighten a proof ladder, or encode a repeated
agent failure as a short rule. Keep SKILL.md compact; move only genuinely large
reference material out of the skill.

## Final Evidence

End with the facts a user or future agent can act on:

- changed artifact paths or commit hash
- exact command paths used
- strict validation and readback/open-check result
- known limitations
- next most useful slice
