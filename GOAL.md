# GOAL: Safe, Practical ooxml-cli

This repo is now a Rust product. The old Go implementation is deprecated reference material only.
Do not build new behavior in Go.

The practical product promise is:

> An agent or human can use `ooxml-cli` as a precise OOXML scalpel to create, inspect, edit, repair,
> and verify the Office files they actually need, without producing files that desktop Office rejects.

This is not a giant Rust-port certification project. Do not run a massive "gauntlet" or chase every
possible command before the tool is useful. The bar is simpler and stricter: the core workflows we
advertise must be safe, bug-resistant, validated, and proven with Office where it matters.

## Non-Negotiables

- Rust is the active product path. Go is legacy reference material only.
- Do not advertise a workflow unless the current Rust CLI can do it.
- Validate changed packages by default. Use skip flags only for diagnosed, explicit cases.
- Treat desktop Office open proof as stronger than XML/package validation.
- Keep code small and boring. Refactor before adding to a growing monolith.
- Use subagents for independent audits or slices, but keep integration, commits, and Office COM gates
  serialized.
- Avoid churn. Add tests and docs only when they prevent real breakage or make the tool safer to use.

## Current Known-Good Evidence

Updated 2026-06-21.

- `tools/windows-office-edit-smoke.ps1` fast gate passed 63/63 scenarios with:
  - `validate --strict`
  - `conformance check`
  - Microsoft Open XML SDK validation
  - artifact proof matrix generation
- A focused desktop Office COM subset passed 7/7 representative generated outputs:
  - `ooxml xlsx scaffold`
  - `ooxml docx scaffold`
  - `ooxml pptx scaffold`
  - `ooxml xlsx pivots create` on a scaffold-derived workbook containing formulas, a table,
    conditional formatting, and a defined name
  - `ooxml pptx place table` on a scaffold-derived deck
  - `ooxml pptx place table-from-xlsx` on a scaffold-derived deck
  - `ooxml pptx charts create` on a scaffold-derived deck
- Workbook child ordering for `<definedNames>` is covered by focused regression tests, and local
  strict validation/conformance now catch the modeled bad order.
- `ooxml repair normalize` is available for recoverable XLSX workbook child-order damage and has
  focused strict-validation/conformance regression coverage.
- `ooxml docx tables create` can append a rectangular table to a scaffolded Word document and is
  covered by readback, structural XML, strict-validation, conformance, Open XML SDK validation, and a
  representative Word-open smoke row.
- VBA `.xlsm` / `.pptm` creation is real on this Windows host through desktop Office COM, but macro
  source editing remains intentionally conservative.

## Practical Safety Loop

Use this loop for ongoing work:

1. Pick one user workflow or one suspected bug class.
2. Reproduce it with the CLI, preferably from a from-scratch scaffold or a realistic fixture.
3. Add the smallest test that would catch the bug or prove the workflow.
4. Fix code only as needed.
5. Run focused verification:
   - `cargo fmt --check`
   - focused `cargo test`
   - `cargo check --all-targets` when Rust changed
   - `ooxml validate --strict` on generated outputs
   - `ooxml --json conformance check` on generated outputs
   - desktop Office COM only for representative release-grade files
6. Commit and push useful green checkpoints.

## Bug-Hunt Priorities

### 1. Artifact Proof Matrix Truthfulness

The matrix is how we avoid false confidence. Keep improving it until it answers:

- Which public mutating commands have no proof row?
- Which commands are proven on from-scratch scaffolds?
- Which commands are proven on realistic fixtures?
- Which representative outputs opened in desktop Office?
- Which rows are validation-only and need stronger evidence?

Current state: the matrix can ingest Office edit smoke summaries with explicit `commandPath` and can
classify Office-open proof. The remaining work is to add focused smoke rows only for commands that
matter operationally, not to blanket-test every edge command immediately.

### 2. XLSX Safety

Keep the realistic workbook path safe:

- scaffold -> formulas/data -> table -> conditional formatting -> defined name -> pivot/chart
- formula XML must be formulas, not cached literals pretending to be formulas
- formula writes must invalidate stale calculation state where appropriate
- workbook child order must stay Office-compatible

Next useful checks:

- A small Office-open smoke row for any formula-heavy workbook we intend users to rely on.
- Keep the formula-cache guard tests green: pivot/chart creation must reject formula-derived source
  ranges whose cached values have not been calculated yet.

### 3. DOCX Safety

Keep Word generation and editing safe for ordinary business docs:

- `docx scaffold` must remain Office-open proven.
- Existing block/table/comment/header/style mutations must not corrupt generated or fixture docs.
- Add new DOCX authoring only when it supports real user workflows.

Next useful checks:

- Keep the `docx scaffold -> docx tables create` Word-open smoke row green.
- Consider the next small DOCX authoring command only when it directly supports a real user workflow.

### 4. PPTX Safety

Keep generated decks safe for template adaptation:

- `pptx scaffold` must remain Office-open proven.
- Table placement, table-from-XLSX, and chart creation on scaffold-derived decks must remain
  Office-open proven.
- Template capture must not fail with misleading "missing spTree" diagnostics when shapes exist.

Next useful checks:

- Add a targeted regression for the real-deck `spTree` capture failure if a fixture can be kept small.
- Keep footer placeholder synthesis covered by focused tests.

### 5. Repair, Convert, And Diff Safety

Add these only where they remove real user pain:

- Keep `repair normalize` narrow: XLSX workbook child order is supported; do not build a broad repair
  framework until we have concrete damaged-package cases.
- Keep `convert xlsm-to-xlsx` covered. It already exists as a VBA-removal alias; the next useful
  proof is conformance on the converted `.xlsx`.
- Keep XLSX diff identity covered. Renamed-sheet alignment is already implemented; add edge tests only
  for real regressions such as rename plus reorder plus cell edits.

## When To Use Subagents

Use parallel agents for:

- Read-only audits across XLSX, DOCX, PPTX, repair/diff/convert, and proof-matrix slices.
- Disjoint implementation work with separate file ownership.
- Fresh-eyes review of a changed command family.

Do not use parallel agents for:

- Office COM gates.
- One-file edits where coordination costs more than the work.
- Vague "make it first-class" assignments.

## Done For Now Means

The tool is safe enough to use when:

- The README only describes workflows that work.
- The core scaffold and representative generated-file workflows open in desktop Office.
- Known Office-rejecting bugs have regression tests.
- The proof matrix is honest about what is proven and what is not.
- `doctor` tells the user clearly whether local Office/Open XML SDK proof is available.
- There are no stale docs or scripts that push agents toward broken workflows.

This file should stay short. If it starts becoming a giant certification document again, trim it back
to the current bugs, proofs, and next safe workflow.
