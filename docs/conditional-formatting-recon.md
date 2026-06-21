# XLSX Conditional Formatting Reconnaissance

Date: 2026-06-21
Worker: P73
Branch: codex/worker-p73-conditional-formatting-recon-20260621
Base: origin/codex/ooxml-rust-port at 0fcc7d22fe1cd03a783b924a6d9b675eae5e7f2c

## Summary

This reconnaissance was written before the conditional-formatting surface was
implemented. It is retained as design history, not as current status.

Current status on 2026-06-21:

- Go and Rust both expose `ooxml xlsx conditional-formats` with
  `list`, `show`, `add`, and `delete`.
- The promoted add surface covers expression, `cellIs`, `colorScale`, and
  `dataBar`
  rules, with stable JSON readback, readback commands, strict validation, and
  Go-vs-Rust contract coverage.
- Serve/MCP supports read-only `list`/`show` through `inspect` and mutating
  `add`/`delete` through `op`.
- XLSM package-artifact preservation is covered for conditional-formatting
  worksheet mutations. Rust-generated XLSX outputs for the promoted rules have
  passed strict validation, Open XML SDK validation, and desktop Excel open
  proof.
- Icon sets, richer x14 extension authoring, and style/dxf creation remain
  intentionally deferred feature slices.

## Historical Surface At Recon Time

### Go CLI and capabilities

- `internal/cli/xlsx.go`: registered the top-level XLSX command and the XLSX groups that existed at the recon base. No conditional-formatting group was registered then.
- `internal/cli/xlsx_cells.go`, `internal/cli/xlsx_ranges.go`, `internal/cli/xlsx_data_validations.go`, `internal/cli/xlsx_hyperlinks.go`, `internal/cli/xlsx_structure.go`: adjacent command patterns for command registration, JSON output, mutation writing, and workbook/sheet selection.
- Runtime probe at the recon base, `go run ./cmd/ooxml --json capabilities`: no `ooxml xlsx ... conditional...` path and no conditional-formatting object kind.
- Runtime probe at the recon base, `go run ./cmd/ooxml xlsx --help`: no conditional-formatting command group.
- `internal/cli/xlsx_structure.go`: maps `ErrWorksheetHasConditionalFormatting` to an invalid-args CLI error for structure edits.

### Rust CLI and capabilities

- `src/cli_dispatch/xlsx.rs`: dispatched the XLSX groups that existed at the recon base. No conditional-formatting branch existed then.
- `src/capabilities/commands/xlsx.rs`: declared XLSX capability metadata. No conditional-formatting group or object kind existed then.
- `src/help.rs`: help text covered adjacent XLSX groups, including data validations, filters/sorts, freeze panes, hyperlinks, names, pivots, ranges, sheets, tables, and workbook metadata. No conditional-formatting help existed then.
- `src/main.rs`: had no conditional-formatting module or re-export then.
- Runtime probe at the recon base, `cargo run --quiet -- --json capabilities`: no conditional-formatting path.
- Runtime probe at the recon base, `cargo run --quiet -- xlsx --help`: no conditional-formatting command group.

### Mutation and preservation helpers

Go:

- `pkg/xlsx/mutate/worksheet.go`: `insertWorksheetChild` uses `worksheetChildOrder`; `conditionalFormatting` is ordered after `phoneticPr` and before `dataValidations`.
- `pkg/xlsx/mutate/structure.go`: structure mutation validation rejects worksheets with direct `conditionalFormatting` children using `ErrWorksheetHasConditionalFormatting`.
- `pkg/xlsx/mutate/structure_test.go`: includes a synthetic conditional-formatting hazard case for structure edits.

Rust:

- `src/xlsx_structure.rs`: validates structure-edit hazards and returns `worksheet has conditional formatting`; `insert_worksheet_child` and `worksheet_child_order` also place `conditionalFormatting` before `dataValidations`.
- `src/xlsx_comments.rs`, `src/xlsx_data_validations.rs`, `src/xlsx_dimensions.rs`, `src/xlsx_filters_sorts/xml_support.rs`, `src/xlsx_freeze.rs`, `src/xlsx_hyperlinks.rs`, and `src/xlsx_charts/model.rs`: duplicated worksheet child-order maps already include `conditionalFormatting`.

These helpers are preservation/order infrastructure, not creation, editing, deletion, inspection, or semantic validation support.

### Conformance

- `pkg/conformance/invariants.go`: worksheet child-order invariants include `conditionalFormatting`; pivot-table child-order invariants include `conditionalFormats`.
- `pkg/conformance/conformance_test.go`: tests the worksheet child-order invariant generally, but not a conditional-formatting-specific fixture.
- `src/conformance_invariants/xml_parts.rs`: Rust worksheet child-order invariant includes `conditionalFormatting`.
- `src/conformance_invariants/table_pivot.rs`: Rust pivot-table child-order invariant includes `conditionalFormats`.

### Tests and fixtures

- Go has one synthetic guard test in `pkg/xlsx/mutate/structure_test.go`.
- Go CLI structure tests cover other hazards but do not appear to include a conditional-formatting CLI hazard case.
- Rust contract smoke tests cover several structure mutation errors against the Go oracle, but do not appear to include the conditional-formatting guard.
- No existing `testdata` XLSX/XLSM fixture with conditional-formatting markup was found.

## Likely Go Oracle Requirement

At the time of this reconnaissance, Go had no user-facing conditional-formatting command, so the first operational slice needed to add the smallest Go oracle behavior worth porting and proving. That recommendation has since been implemented for list/show/add/delete plus expression, `cellIs`, and `colorScale` rules.

Recommended Go-first order:

1. Read-only inventory: list/show conditional formatting rules from worksheet XML.
2. Minimal mutation: add/delete expression and cell-is rules using existing worksheet child ordering.
3. Priority operations: set priority, reorder, and preserve `stopIfTrue`.
4. Visual rules: color scales, data bars, and icon sets once the simple rule path is proved.
5. XLSM macro-preservation proof for every mutating command before declaring parity.

## Proposed First Operational Command Set

Command family: `ooxml xlsx conditional-formats`.

Alias candidates: `conditional-format`, `cf`.

Initial commands:

- `list <file> --sheet <selector> [--range <sqref>] [--json]`: returns worksheet conditional-formatting blocks and rules in document order, including `sqref`, rule type, priority, operator, formulas, `dxfId`, `stopIfTrue`, and extension presence.
- `show <file> --sheet <selector> --rule <id-or-priority> [--json]`: returns one normalized rule plus its raw XML fallback for unsupported fields.
- `add <file> --sheet <selector> --range <sqref> --type expression --formula <formula> [--priority <n>] [--stop-if-true] [--dxf-id <id>] --out <file>`: adds a formula rule without rewriting unrelated conditional-formatting blocks.
- `add <file> --sheet <selector> --range <sqref> --type cell-is --operator <op> --formula <formula> [--formula2 <formula>] [--priority <n>] [--stop-if-true] [--dxf-id <id>] --out <file>`: adds a cell comparison rule.
- `delete <file> --sheet <selector> --rule <id-or-priority> --out <file>`: removes a rule and removes an empty `conditionalFormatting` block.
- `reorder <file> --sheet <selector> --rule <id-or-priority> --priority <n> --out <file>`: changes priorities with deterministic renumbering.
- Later visual commands or subtypes: `--type color-scale`, `--type data-bar`, and `--type icon-set`, with a conservative normalized model and raw XML preservation for extensions.

The command should avoid promising style authoring beyond `dxfId` until the project has a clear style mutation oracle. If inline style creation is required, that should be a separate, proved style/dxf helper.

## Exact Files and Functions to Touch

Go oracle:

- `pkg/xlsx/mutate/conditional_formatting.go`: new parse/list/add/delete/reorder helpers and normalized rule model.
- `pkg/xlsx/mutate/worksheet.go`: reuse `insertWorksheetChild`; expose or factor it only if the new helper cannot live in the same package cleanly.
- `internal/cli/xlsx_conditional_formatting.go`: new command family, argument validation, JSON result shape, output writing, and errors.
- `internal/cli/xlsx.go`: only if direct registration is needed beyond the normal `init` registration pattern.
- `pkg/capabilities` and capability tests: add command metadata/object kinds if the Go capability surface is generated or centralized.
- `pkg/xlsx/mutate/conditional_formatting_test.go`: unit tests for order, priority, stop-if-true, delete, and raw unsupported-field preservation.
- `internal/cli/xlsx_conditional_formatting_test.go`: CLI tests for list/show/add/delete/reorder and error behavior.
- `testdata/xlsx/conditional-formatting/*`: fixtures with expression, cell-is, color-scale, data-bar, icon-set, existing priorities, and unsupported extension content.

Rust port:

- `src/xlsx_conditional_formatting.rs`: Rust normalized model and mutation helpers.
- `src/cli_dispatch/xlsx/conditional_formatting.rs`: command parser/runner.
- `src/cli_dispatch/xlsx.rs`: register the new dispatch branch and aliases.
- `src/capabilities/commands/xlsx/conditional_formatting.rs`: capability metadata for the new command family.
- `src/capabilities/commands/xlsx.rs`: include the new metadata module/group.
- `src/help.rs`: add help text that matches the Go user surface.
- `src/main.rs`: module declaration and any public re-export required by dispatch/tests.
- `src/serve/op_dispatch/xlsx.rs`: update only if the serve/op layer is expected to expose the new command family.
- `tests/rust_contract_smoke/xlsx.rs` or a new included module: Go-vs-Rust contract tests for list/show/add/delete/reorder.

Shared cleanup candidate:

- The Rust worksheet child-order maps are duplicated across several XLSX feature modules. Do not refactor this as part of the first conditional-formatting slice unless the implementation needs it; if touched, add a focused regression test because ordering mistakes can cause Excel repair prompts.

## Proof Gates

Docs-only reconnaissance:

- `git diff --check`

Operational implementation:

- Go oracle unit and CLI tests for every command in the first slice.
- Rust contract tests that compare Rust output and package mutations with the Go oracle.
- Strict OOXML validation after each mutating command.
- Open XML SDK validation for generated XLSX and XLSM files.
- Excel desktop COM open/save-open smoke where available.
- Round-trip preservation: unchanged ZIP entries remain byte-identical where feasible; changed entries are limited to the target worksheet and any intentional styles part changes.
- Ordering proof: new `conditionalFormatting` appears in the valid worksheet child position before `dataValidations`, `hyperlinks`, drawing anchors, and related later children.
- Priority proof: adds, deletes, and reorders have deterministic priority behavior and do not create duplicate priorities unless explicitly matching Excel-compatible behavior.
- Extension preservation: unsupported rule attributes, nested extension lists, and x14 conditional-formatting content are preserved unless the command explicitly rejects them.
- XLSM macro preservation:
  - Seed an XLSM using the existing VBA attach path.
  - Run each conditional-formatting mutation on the XLSM.
  - Assert `xl/vbaProject.bin` SHA-256 is unchanged.
  - Assert macro content types and workbook relationships are preserved.
  - Run `vba inspect`, `vba list`, and `vba extract-bin` before/after.
  - Run Open XML SDK validation and Excel desktop open proof without executing macros.

## Risks

- Conditional formatting spans simple `cfRule` elements, differential styles, visual rules, priorities, `stopIfTrue`, formulas, and extension namespaces. A broad first slice is likely to hide parity gaps.
- `dxfId` handling may require style table authoring or deduplication. Treat style creation as a separate oracle unless the first slice only references existing differential styles.
- Excel may renumber priorities or repair malformed order. Priority behavior must be proved with Excel/SDK rather than inferred from XML alone.
- Formula and `sqref` handling is worksheet-local and interacts with row/column structure edits. Existing structure edits refuse conditional formatting because they do not rewrite ranges or formulas.
- Rust has several duplicated worksheet child-order maps. A conditional-formatting writer that uses only one map can still drift from adjacent feature insertion behavior.
- XLSM handling is easy to regress if package rewriting drops macro relationships, content types, or `vbaProject.bin`.
- No current fixtures means early tests must create both ordinary and macro-enabled workbooks before claiming coverage.

## Suggested First Slice

Start with Go read-only `list`/`show` plus fixture creation. Then add Go mutation for expression and cell-is rules only, with existing `dxfId` references rather than new style authoring. Once that oracle is stable, port the same normalized JSON and mutation behavior to Rust and add Go-vs-Rust contract tests. Defer color scales, data bars, icon sets, x14 extension authoring, and style/dxf creation until the simple path survives Excel, SDK, strict validation, and XLSM macro-preservation gates.
