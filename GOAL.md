# GOAL: Rust-Only First-Class ooxml-cli

This repo is now a Rust product. The old Go implementation is deprecated reference material only.
Do not build new behavior in Go. Do not keep measuring success as "matches Go" when Go lacks the
feature, misses an Office failure, or encodes a historical limitation.

The product promise is simple:

> An agent or human can use `ooxml-cli` as a precise OOXML scalpel to create, inspect, edit, repair,
> and verify Word, Excel, and PowerPoint files that desktop Office accepts.

That promise is stronger than "the zip parses" and stronger than "our tests are green". It means the
file opens in the relevant Office app, the command surface is predictable for agents, and validation
guards catch the classes of mistakes that Office rejects.

## Current Baseline

- Rust is the default CLI, docs, CI target, and implementation lane.
- Go lives under `go/` as deprecated reference. Use it only for frozen historical comparison when it
  is helpful; never let it veto a correct Rust improvement.
- Conditional formatting exists in Rust: `xlsx conditional-formats list/show/add/delete/reorder`.
- VBA macro creation for `.xlsm` and `.pptm` has been proven on this Windows machine through desktop
  Office COM automation.
- Manual Office smoke has opened generated `.docx`, `.xlsx`, and `.pptx` in Word, Excel, and
  PowerPoint.
- Known urgent defect: `xlsx names add` exposed an Office-rejecting workbook child-order failure
  around `<definedNames>`. The correct CT_Workbook order is:
  `workbookPr -> bookViews -> sheets -> definedNames -> calcPr`.
- Known validator defect: `validate --strict` and `conformance check` must catch OOXML child-order
  errors that desktop Office rejects, even when the Open XML SDK tier is unavailable.

## Non-Negotiables

- Rust-only feature work. Go changes are limited to archival moves, build isolation, or frozen
  reference harness maintenance.
- Every mutating command that writes an Office package must either run strict validation or clearly
  require an explicit skip flag.
- "Valid" means structurally valid enough for Office, not just parseable XML.
- Generated proof artifacts must be opened locally with desktop Office on Windows for release-grade
  gates.
- Keep command output useful for agents: JSON, command templates, selectors/handles, validation
  commands, and clear diagnostics.
- Keep code simple. Prefer small focused modules over another Rust monolith.
- Use subagents aggressively for independent slices, but serialize integration, Office COM, and
  full-suite gates.
- Do not churn tests for vanity. Add tests that prevent real breakage or prove new user value.

## Skills To Apply

Use these skills intentionally, in this rough order:

1. `$planning-workflow` for this file and milestone discipline.
2. `$agent-fungibility-philosophy` for parallel subagent lanes with clear ownership boundaries.
3. `$de-monolithize-your-codebase-isomorphically` before large additions touch already-large Rust
   files.
4. `$simplify-and-refactor-code-isomorphically` when a slice starts duplicating OOXML helpers.
5. `$testing-conformance-harnesses` for validation, Office-open, and regression proof.
6. `$multi-pass-bug-hunting` after each integration pass, especially on OOXML insertion/rewrite code.
7. `$agent-ergonomics-and-intuitiveness-maximization-for-cli-tools` before finalizing new command
   names and JSON output.
8. `$world-class-doctor-mode-for-cli-tools` if doctor output needs to explain local Office/SDK proof
   availability.
9. `$readme-writing` only after behavior is real and stable.
10. `$running-the-gauntlet-on-your-rust-port` for the final honest parity/readiness audit.

## Parallel Work Model

Use one integration lane and multiple worker lanes. Workers may develop and write code in separate
worktrees. The integration lane owns conflict resolution, final command naming, Office gates, and
commits to the main Rust branch.

### Integration Lane

- Own `GOAL.md`, branch hygiene, commits, push, and release readiness.
- Keep subagents pointed at narrow command-family gaps.
- Merge worker results in small batches.
- Run the serialized gates:
  - `cargo fmt --check`
  - `cargo check --all-targets`
  - focused `cargo test` for changed command families
  - broad Rust test suite when stable
  - `ooxml validate --strict`
  - `ooxml --json conformance check`
  - Windows Office oracle for representative generated files

### Worker A: XLSX Core Authoring

Own XLSX from-nothing and spreadsheet authoring gaps:

- Implement pure Rust `xlsx` scaffold creation without desktop Office.
- Add table creation, not just append-to-existing-table.
- Prove formulas written by `xlsx cells set` and range commands preserve formula XML correctly.
- Ensure formula writes set recalc state and do not leave stale cached values that mislead Excel.
- Ensure pivots, tables, formulas, conditional formats, comments, freeze panes, styles, hyperlinks,
  and data validations can coexist in one generated workbook.
- Add Office-open proof for a generated `.xlsx` with formulas, a table, conditional formatting, and
  a pivot/table-derived view.

### Worker B: DOCX Authoring

Own Word from-nothing and block construction:

- Implement pure Rust `docx` scaffold creation.
- Add real block constructors for paragraphs, headings, lists, tables, and styled runs where the
  current surface is mutation-only.
- Ensure style creation/application is usable from commands and JSON.
- Prove generated `.docx` opens in Word and survives strict validation/conformance.
- Avoid giant XML string blobs when a reusable document builder helper is warranted.

### Worker C: PPTX Authoring

Own PowerPoint creation and fragile template areas:

- Implement pure Rust `pptx` scaffold creation.
- Harden `pptx template capture` against real decks where shapes exist but the expected `spTree`
  lookup path fails.
- Make footer field updates visible by synthesizing missing footer placeholders when requested.
- Ensure table/text/chart placement from XLSX works on generated decks, not only fixtures.
- Prove generated `.pptx` opens in PowerPoint and survives strict validation/conformance.

### Worker D: Repair, Normalize, and Validation

Own the safety rails:

- Add or improve `repair`/`normalize` behavior for recoverable OOXML ordering and relationship
  problems that block unrelated edits.
- Ensure `validate --strict` catches Office-rejecting child-element ordering for workbook, worksheet,
  presentation, slides, charts, tables, pivots, and any other locally modeled package parts.
- Keep `conformance check` as the deeper package proof, with optional Office-open tier.
- Add focused tests for every validator false-confidence bug discovered.
- Current first target: workbook `<definedNames>` ordering must be caught by both `validate --strict`
  and `conformance check`.

### Worker E: Diff and Conversion Ergonomics

Own agent-facing sharp edges:

- Fix XLSX diff so renamed sheets still cell-align using stable identity such as sheetId, relationship
  id, or part URI when names change.
- Add discoverable conversion aliases where behavior already exists, for example `.xlsm` to `.xlsx`
  as a macro-removal/save-as workflow.
- Keep JSON output explicit about what changed, what was removed, and what proof command to run.

### Worker F: CLI Ergonomics and Docs

Own the human/agent surface:

- Review new commands for predictable names, required flags, examples, and JSON shape.
- Update capabilities so agents can discover from-scratch creation and safety commands.
- Keep README short, honest, and operational.
- Avoid docs that advertise features before they pass local proof gates.

## Implementation Priorities

### Milestone 0: Stabilize The Current Branch

- Keep the Rust-default commits.
- Keep the Windows VBA helper fallback fix.
- Fix the workbook child-order validator gap.
- Run focused tests for XLSX names and validation.
- Commit and push a stable checkpoint.

### Milestone 1: Pure Package Scaffolds

Add from-nothing creation for:

- `.xlsx`: workbook, workbook rels, content types, one sheet, styles baseline, calc settings.
- `.docx`: document, relationships, content types, body, section properties, minimal styles if useful.
- `.pptx`: presentation, slide master/layout, one slide, relationships, content types, theme baseline.

Acceptance:

- Each scaffold validates strictly.
- Each scaffold passes conformance check.
- Each scaffold opens in the corresponding desktop Office app on this Windows host.
- Each scaffold can be used as input to existing mutation commands.

### Milestone 2: XLSX First-Class Authoring

Add and prove:

- Table creation.
- Formula authoring with calc invalidation/recalc proof.
- Conditional formatting in a from-scratch workbook.
- Pivot/table/formula coexistence.
- Office-open proof for a generated workbook that uses a realistic mix of features.

Acceptance:

- Excel opens the generated workbook without repair prompts.
- Formula cells are formulas, not only cached literals.
- Validation catches stale/order-invalid workbook XML.

### Milestone 3: DOCX First-Class Authoring

Add and prove:

- Create document from scratch.
- Append/insert block commands that build paragraphs, headings, lists, and tables.
- Style application suitable for template adaptation work.

Acceptance:

- Word opens generated documents.
- Existing mutation commands work on generated documents.
- No giant monolith accumulates in DOCX code.

### Milestone 4: PPTX First-Class Authoring

Add and prove:

- Create presentation from scratch.
- Add slides and place text, tables, and charts.
- Synthesize visible footer placeholders when fields require them.
- Harden template capture on real-world decks.

Acceptance:

- PowerPoint opens generated decks.
- Existing table/chart/text commands work on generated decks.
- Template capture fails with actionable diagnostics or succeeds; no misleading "missing spTree"
  when shapes are present.

### Milestone 5: Repair, Diff, Convert

Add and prove:

- Repair/normalize for common safe OOXML ordering and relationship issues.
- Diff aligns renamed sheets by stable identity.
- Conversion aliases for common workflows, especially macro removal/save-as.

Acceptance:

- Broken-but-recoverable packages can be normalized before edit.
- Diff reports real cell changes across sheet renames.
- Conversion commands are discoverable through capabilities and README.

### Milestone 6: Gauntlet

Run the honest release-readiness pass:

- Full Rust test suite.
- Conformance check on representative generated and mutated artifacts.
- Windows Office open proof for `.docx`, `.xlsx`, `.xlsm`, `.pptx`, and `.pptm` where applicable.
- Doctor output reviewed for local SDK/Office availability.
- README is short and matches reality.
- No stale TODO docs, dead scripts, or Go-first references remain in the active path.

## Current Gap Ledger

The following findings are treated as live work until closed by code and proof:

1. No pure from-nothing scaffold for `.docx`, `.xlsx`, or `.pptx`.
2. XLSX tables can append/update existing tables but lack a create verb.
3. Formula-heavy workbook proof is incomplete, especially around recalc/cache invalidation and Office
   open behavior.
4. DOCX authoring is still too mutation-oriented and lacks a clean block construction layer.
5. PPTX template capture may fail on real decks with present shapes but unexpected tree layout.
6. No repair/normalize command for safe recoverable OOXML issues.
7. PPTX footer field setting creates metadata but not visible missing footer placeholders.
8. XLSM-to-XLSX works through VBA removal but needs a clear conversion alias.
9. XLSX diff aligns sheets by name, so renamed sheets do not cell-align.
10. Validator false confidence: modeled schema child ordering must be checked locally, starting with
    workbook `<definedNames>` placement.

## Proof Rules

Every new feature lands with:

- A focused Rust integration test.
- A saved-readback assertion through the CLI where possible.
- `validate --strict` success for generated outputs.
- `conformance check` success for generated outputs.
- A negative test for at least one realistic broken case when adding validator coverage.
- Office-open proof for release-grade artifact families on Windows.

Office COM gates are serialized. Rust unit/integration tests can run in parallel. Subagents can work
in parallel worktrees, but integration commits must stay small enough to review.

## Generated Artifact Matrix

The repo needs a command-to-artifact proof matrix. This is how bugs like invalid workbook child order
stop slipping through.

Scope the matrix around public user-facing commands, not every private Rust helper. Internal helper
tests are useful, but users care whether `ooxml xlsx names add ... --out file.xlsx` creates a file
that Excel can open.

Each mutating command family should have at least one row with:

- The exact CLI command.
- The input fixture type: scaffold, clean fixture, realistic fixture, or intentionally damaged file.
- The generated output path.
- Structural assertions against the OOXML part most likely to break.
- Saved-readback through the CLI.
- `validate --strict`.
- `conformance check`.
- Office-open status for representative rows.
- Golden/semantic snapshot where the output structure is complex enough that field assertions are
  too weak.

Coverage target:

- Fast CI: all mutators get structural/readback/validate/conformance proof where practical.
- Local Windows release gate: representative generated files for every Office family open in desktop
  Word, Excel, and PowerPoint.
- Nightly/deep gate: broader Office-open matrix across command families, serialized to avoid COM
  flakiness.

The matrix must be able to answer:

- Which commands create or mutate files?
- Which commands are proven on from-scratch scaffolds?
- Which commands are proven on realistic files?
- Which commands have Office-open proof?
- Which commands only have parser-level proof and need a stronger row?

## Commit Discipline

- Commit stable milestones.
- Push after meaningful green checkpoints.
- Mention Office proof status in commit messages or follow-up notes when it is part of the change.
- Keep generated manual artifacts out of git unless they are deliberate small fixtures.
- Do not hide failures behind `--no-validate`; use it only for diagnosed pre-existing package damage
  and then add repair/normalize work if the pattern matters.
