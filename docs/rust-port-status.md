# Rust Port Status

The Go implementation remains the reference on `codex/ooxml-go-reference`.
Rust work lands on `codex/ooxml-rust-port`. The Rust smoke harness builds its
Go oracle from a detached `codex/ooxml-go-reference` worktree by default, or
from `OOXML_GO_ORACLE_DIR`/`OOXML_GO_ORACLE_REF` when deliberately overridden.

The frozen Go contract lives in `testdata/golden/rust-port-contract/baseline.json`.

Latest milestone, 2026-06-20:

- Rust PPTX media parity landed for direct `pptx media list`, `add`, and
  `replace`. The slice lists embedded media, adds video/media with poster
  metadata, and replaces existing media bytes while preserving Go-shaped
  saved-output, readback, and error behavior. Rust capabilities now advertise
  166 Go-oracle command paths, leaving a pinned 124-command gap; these commands
  are direct CLI operations with `opCompatible=false` because serve/MCP operation
  dispatch is not wired for `pptx media` yet. Proof: focused Go-vs-Rust PPTX
  media tests; focused capability ratchet/discovery tests; strict validation for
  both generated proof decks; Open XML SDK Office2019 schema validation (zero
  errors) for both proof decks; PowerPoint COM open oracle for both proof decks;
  `cargo fmt --check`; `cargo check --all-targets`; `cargo clippy --all-targets
  -- -D warnings`; and `cargo test --all-targets` passed with 4 ZIP guard unit
  tests plus 150 Rust contract tests.
- Rust PPTX XLSX-backed text replacement parity landed for direct `pptx replace
  text-from-xlsx` and `text-map-from-xlsx`. The slice reads workbook ranges or
  mapping tables and applies the resolved text replacements to PPTX shapes
  while preserving Go-shaped saved-output, dry-run, readback, and error
  behavior. Rust capabilities now advertise 163 Go-oracle command paths,
  leaving a pinned 127-command gap; these commands are direct CLI mutations with
  `opCompatible=false` because serve/MCP operation dispatch is not wired for
  these `pptx replace` variants yet. Proof: focused Go-vs-Rust XLSX-backed PPTX
  replacement tests; focused capability ratchet/discovery tests; strict
  validation for both generated proof decks; Open XML SDK Office2019 schema
  validation (zero errors) for both proof decks; PowerPoint COM open oracle for
  both proof decks; `cargo fmt --check`; `cargo check --all-targets`; `cargo
  clippy --all-targets -- -D warnings`; and `cargo test --all-targets` passed
  with 4 ZIP guard unit tests plus 148 Rust contract tests.
- Rust PPTX layout mutation parity landed for direct `pptx layouts rename`,
  `set-bounds`, `delete-shape`, and `add-placeholder`. The slice edits layout
  names, placeholder geometry, layout shape removal, and new placeholders while
  preserving Go-shaped saved-output, dry-run, readback, and error behavior.
  Rust capabilities now advertise 161 Go-oracle command paths, leaving a pinned
  129-command gap; these commands are direct CLI mutations with
  `opCompatible=false` because serve/MCP operation dispatch is not wired for
  `pptx layouts` mutations yet. Proof: focused Go-vs-Rust layout mutation
  tests; focused capability ratchet/discovery tests; strict validation for all
  four generated proof decks; Open XML SDK Office2019 schema validation (zero
  errors) for all four proof decks; PowerPoint COM open oracle for all four
  proof decks; `cargo fmt --check`; `cargo check --all-targets`; `cargo clippy
  --all-targets -- -D warnings`; and `cargo test --all-targets` passed with 4
  ZIP guard unit tests plus 146 Rust contract tests.
- Rust PPTX chart style mutation parity landed for direct `pptx charts
  set-title`, `set-legend`, `set-chart-area-fill`, `set-plot-area-fill`, and
  `set-series-style`. The slice mutates embedded chart title text/font fields,
  legend position/overlay, chart-area and plot-area fills, and per-series
  fill/line styling while preserving Go-shaped saved-output, dry-run, readback,
  and error behavior. Rust capabilities now advertise 157 Go-oracle command
  paths, leaving a pinned 133-command gap; these commands are direct CLI
  mutations with `opCompatible=false` because serve/MCP operation dispatch is
  not wired for `pptx charts` yet. Proof: focused Go-vs-Rust chart style
  mutation tests; focused capability ratchet/discovery tests; strict validation
  for all five generated proof decks; Open XML SDK Office2019 comparison
  showing the same inherited 12 `/ppt/charts/chart2.xml` axis schema errors in
  the source fixture and all five proof decks, with no new mutation-added
  errors; PowerPoint COM open oracle for all five proof decks; `cargo fmt
  --check`; `cargo check --all-targets`; `cargo clippy --all-targets -- -D
  warnings`; and `cargo test --all-targets` passed with 4 ZIP guard unit tests
  plus 145 Rust contract tests.
- Rust XLSX chart style mutation parity landed for direct `xlsx charts
  set-title`, `set-legend`, `set-chart-area-fill`, `set-plot-area-fill`, and
  `set-series-style`. The slice mutates chart title text/font fields, legend
  position/overlay, chart-area and plot-area fills, and per-series fill/line
  styling while preserving Go-shaped saved-output, dry-run, readback, and error
  behavior. Rust capabilities now advertise 152 Go-oracle command paths,
  leaving a pinned 138-command gap; these commands are direct CLI mutations with
  `opCompatible=false` because serve/MCP operation dispatch is not wired for
  `xlsx charts` yet. Proof: focused Go-vs-Rust chart style mutation tests;
  focused capability ratchet/discovery tests; strict validation for all five
  generated proof workbooks; Open XML SDK Office2019 schema validation (zero
  errors) for all five proof workbooks; Excel COM open oracle for all five proof
  workbooks; `cargo fmt --check`; `cargo check --all-targets`; `cargo clippy
  --all-targets -- -D warnings`; and `cargo test --all-targets` passed with 4
  ZIP guard unit tests plus 144 Rust contract tests.
- Rust PPTX replacement parity expanded for agent deck-editing workflows with
  direct CLI support for `pptx replace text-occurrences` and `pptx replace
  images`. The slice matches the Go oracle for occurrence dry-runs, stale
  `--expect-count`/`--expect-plan-hash` guards, saved-output JSON, no-match
  errors, shape readback commands, single-picture replacement by slide/selector,
  image destination metadata, missing-target hints, extracted-image artifact
  readback, and strict validation of mutated PPTX outputs. Rust capabilities
  now advertise 147 Go-oracle command paths, leaving a pinned 143-command gap.
  `pptx media list/replace`, `pptx replace images --for-slides` batch output,
  and the XLSX-backed PPTX text replacement variants remain explicit follow-up
  seams. Proof: focused Go-vs-Rust contract tests for text-occurrences and image
  replacement; focused capability ratchet/discovery tests; strict validation for
  fresh occurrence/image proof PPTX files; Open XML SDK Office2019 schema
  validation (zero errors); PowerPoint COM open oracle for both proof decks;
  `cargo fmt --check`; `cargo check --all-targets`; `cargo clippy --all-targets
  -- -D warnings`; and `cargo test --all-targets` passed with 4 ZIP guard unit
  tests plus 142 Rust contract tests.
- Rust VBA source-module workflow parity expanded to direct `vba create` and
  `vba office-check`. `vba create` validates `.bas`/`.cls` sources, creates
  `.xlsm` and `.pptm` packages from scratch through Windows desktop Office COM,
  imports modules, optionally extracts the authored `vbaProject.bin`, and emits
  inspect/list/validate/office-check/readback follow-up commands. `vba
  office-check` now prefers the Microsoft Office COM open oracle on Windows and
  falls back to the compatibility engine path elsewhere. Source-changing
  `add-module`, `replace-module`, and `remove-module` remain unadvertised in
  Rust. Rust capabilities now advertise 145 Go-oracle command paths, leaving a
  pinned 145-command gap. Proof: focused Go-vs-Rust VBA create/office-check
  contract tests; focused capability ratchet/discovery tests; Office-authored
  XLSM and PPTM proof files from `.bas`/`.cls` sources; VBA list readback for
  both proof files; strict Rust validation; Open XML SDK Office2019 schema
  validation (zero errors); Excel and PowerPoint COM open oracle; `vba
  office-check` returning `microsoftOfficeVerified: true` for both proof files;
  `cargo fmt --check`; `cargo check --all-targets`; `cargo clippy --all-targets
  -- -D warnings`; and `cargo test --all-targets` passed with 4 ZIP guard unit
  tests plus 140 Rust contract tests.
- Rust PPTX charts read-only parity landed for direct `pptx charts list` and
  `pptx charts show`. The slice discovers chart relationships from slides,
  reads chart parts, selectors, titles, type hints, series references and cache
  previews, and reports representative Go-compatible errors. Rust capabilities
  now advertise 143 Go-oracle command paths, leaving a pinned 147-command gap.
  Proof: focused Go-vs-Rust chart list/show contract tests; focused capability
  ratchet/discovery tests; `cargo fmt --check`; `cargo check --all-targets`;
  `cargo clippy --all-targets -- -D warnings`; and `cargo test --all-targets`
  passed with 4 ZIP guard unit tests plus 138 Rust contract tests. No Office
  COM or Open XML SDK proof is required because this slice only reads package
  parts.
- Rust PPTX shapes/bounds parity landed for direct `pptx shapes get`,
  `pptx shapes set-bounds`, and `pptx shapes delete`. The slice adds
  Go-compatible single-shape readback, bounds mutation with dry-run/saved-output
  JSON, delete dry-run/saved-output JSON, readback/validate/render command
  fields for bounds mutations, target/error parity, strict validation, and
  capability discovery. Rust capabilities now advertise 141 Go-oracle command
  paths, leaving a pinned 149-command gap. Proof: focused Go-vs-Rust shape
  contract tests for output JSON, saved mutation output, readback commands,
  dry-run behavior, and errors; focused capability ratchet/discovery tests;
  strict validation for generated PPTX proof files; Open XML SDK Office2019
  schema validation (zero errors) for set-bounds/delete proof files; PowerPoint
  COM open oracle for both proof decks; `cargo fmt --check`; `cargo check
  --all-targets`; `cargo clippy --all-targets -- -D warnings`; and `cargo test
  --all-targets` passed with 4 ZIP guard unit tests plus 137 Rust contract
  tests.
- Rust XLSX batch/style direct CLI parity landed for `xlsx cells clear`,
  `xlsx cells set-batch`, and `xlsx ranges set-style`. The slice clears cell
  values/formulas with Go-compatible handle/range validation and readback
  truncation, sets sparse JSON/stdin cell batches with details and formula
  recalc behavior, applies font/fill/border/alignment styles while preserving
  existing number formats, updates capability discovery, and keeps all three
  commands as direct CLI mutations with serve/MCP operation dispatch unwired.
  Rust capabilities now advertise 138 Go-oracle command paths, leaving a
  pinned 152-command gap. Proof: focused Go-vs-Rust contract tests for
  `xlsx_cells_clear`, `xlsx_cells_set_batch`, and `xlsx_ranges_set_style`,
  focused capability ratchet/discovery tests, strict validation for
  Rust-generated XLSX outputs inside the focused tests plus generated proof
  workbooks, Open XML SDK Office2019 schema validation (zero errors) for clear,
  set-batch, and set-style proof workbooks, Excel COM open oracle for those
  proof workbooks, `cargo fmt --check`, `cargo check --all-targets`, `cargo
  clippy --all-targets -- -D warnings`, and `cargo test --all-targets` passed
  with 4 ZIP guard unit tests plus 136 Rust contract tests.
- Rust XLSX charts read-only parity landed for direct `xlsx charts list` and
  `xlsx charts show`. The slice discovers charts through worksheet drawing
  relationships, reads chart parts, selectors, series source references and
  cache previews, anchor metadata, source export commands, and practical style
  readback. Rust capabilities now advertise 135 Go-oracle command paths,
  leaving a pinned 155-command gap; `xlsx charts show` is intentionally
  implemented but not advertised because the Go capability oracle does not
  publish that path. Proof: focused Go-vs-Rust chart list/show contract tests,
  focused capability ratchet/discovery tests, `cargo fmt --check`,
  `cargo check --all-targets`, and `cargo clippy --all-targets -- -D warnings`.
  No Office COM or Open XML SDK proof is required because this slice only reads
  package parts. Chart mutations (`update-source`, `set-title`, and style
  setters) remain for a follow-up lane with saved-output/readback, dry-run,
  strict validation, and Office/Open XML proof.
- Rust PPTX slide lifecycle parity landed for direct `pptx slides delete`,
  `pptx slides move`, and `pptx slides reorder`. The slice deletes slide
  references, presentation relationships, slide parts, slide rels, notes parts
  when present, and content-type overrides; move/reorder preserve slide parts
  while rewriting presentation slide order. Saved-output JSON, dry-run
  templates, readback/validate command fields, representative Go-compatible
  errors, strict Rust validation, and capability discovery are covered by a
  focused Go-vs-Rust contract. Rust capabilities now advertise 134 Go-oracle
  command paths, leaving a pinned 156-command gap; the commands are direct CLI
  mutations with `opCompatible=false` because serve/MCP operation dispatch is
  not wired in this slice. Proof: focused Go-vs-Rust lifecycle tests,
  capability ratchet/discovery tests, strict validation for generated delete,
  move, and reorder proof decks, Open XML SDK Office2019 schema validation
  (zero errors) for those proof decks, PowerPoint COM open oracle for all three
  proof decks, `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy
  --all-targets -- -D warnings`, and `cargo test --all-targets` passed with 4
  ZIP guard unit tests plus 131 Rust contract tests.
- Rust PPTX table column and XLSX-backed update parity landed for
  `pptx tables insert-col`, `pptx tables delete-col`, and
  `pptx tables update-from-xlsx`. The slice inserts/deletes table columns with
  Go-compatible dimensions, grid widths, merge-safety errors, readback command
  fields, dry-run behavior, and strict validation. It also refreshes a PPTX
  table from XLSX ranges or named tables, including formula/value mode,
  expected-source-range guards, max-cell guards, dimension mismatch rejection,
  merged-table rejection, capability discovery, and serve op dispatch for all
  three leaves. Rust capabilities now advertise 131 Go-oracle command paths,
  leaving a pinned 159-command gap. Proof: focused Go-vs-Rust table column and
  update-from-XLSX contract tests, serve op tests, capability ratchet/discovery
  tests, strict validation for generated PPTX proof files, Open XML SDK
  Office2019 schema validation (zero errors) for proof files, PowerPoint COM
  open oracle for proof files, `cargo fmt --check`, `cargo check --all-targets`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets`
  passed with 4 ZIP guard unit tests plus 130 Rust contract tests.
- Rust PPTX extract parity expanded for the remaining useful read/export leaves
  `pptx extract images` and `pptx extract xml`. The slice writes Go-shaped image
  extraction manifests and image artifacts, preserves the Go oracle's no-image
  `images: null` shape, writes raw slide/layout/master XML extraction
  directories with summary files, mirrors selector flags and representative
  error envelopes, and keeps both commands as direct CLI exports rather than
  serve/MCP inspect operations. Rust capabilities now advertise 128 Go-oracle
  command paths, leaving a pinned 162-command gap. Proof: focused Go-vs-Rust
  contract tests compare JSON and output artifacts for images/XML plus error
  parity, focused capability ratchet/discovery tests, `cargo fmt --check`,
  `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`,
  and `cargo test --all-targets` passed with 4 ZIP guard unit tests plus 128
  Rust contract tests. No Office COM proof is required because this slice only
  reads/export package parts without mutating Office files.
- Rust XLSX hyperlinks parity landed for direct `xlsx hyperlinks list`,
  `xlsx hyperlinks show`, `xlsx hyperlinks add`,
  `xlsx hyperlinks update`, and `xlsx hyperlinks delete` with the Go aliases
  `hyperlink` and `links`. The slice reads internal, external, and broken
  worksheet hyperlinks, preserves stable cell/range selectors, creates and
  updates worksheet hyperlink relationships, removes orphaned hyperlink rels,
  supports stale `--expect-url` / `--expect-location` guards, and matches
  Go-shaped mutation JSON, dry-run, error envelopes, validation/readback command
  fields, and saved-output readback. Rust capabilities now advertise 126
  Go-oracle command paths, leaving a pinned 164-command gap. Proof:
  `cargo fmt --check`, `cargo check --all-targets`, focused `cargo test --test
  rust_contract_smoke xlsx_hyperlinks -- --nocapture`, focused capability
  ratchet/discovery tests, strict validation for generated add/update/delete
  XLSX proof files, Open XML SDK Office2019 schema validation (zero errors) for
  those proof files, Excel COM open oracle for those proof files, `cargo clippy
  --all-targets -- -D warnings`, and `cargo test --all-targets` passed with 4
  ZIP guard unit tests plus 126 Rust contract tests.
- Rust XLSX row/column structural mutation parity landed for direct
  `xlsx rows insert`, `xlsx rows delete`, `xlsx cols insert`, and
  `xlsx cols delete`. The slice rewrites worksheet row and cell references,
  updates worksheet dimensions, preserves Go-oracle guardrails for formulas,
  merged cells, tables, column metadata, invalid row/cell references, and
  related structural hazards, and emits Go-shaped saved/dry-run readback
  command fields. Rust capabilities now advertise 121 Go-oracle command paths,
  leaving a pinned 169-command gap. Serve mutation ops remain intentionally
  unwired for this direct-CLI-only structural worksheet slice. Proof: focused
  Go-vs-Rust saved-output/readback, dry-run, and error parity with `cargo test
  --test rust_contract_smoke xlsx_structure -- --nocapture`, focused
  capability ratchet/discovery tests, strict validation for generated Rust XLSX
  outputs in the contract harness plus four integration proof files, Open XML
  SDK Office2019 schema validation (zero errors) for all four proof files,
  Excel COM open oracle for all four proof files, `cargo clippy --all-targets
  -- -D warnings`, and `cargo test --all-targets` passed with 4 ZIP guard unit
  tests plus 123 Rust contract tests.
- Rust XLSX worksheet data-validations direct CLI parity landed for
  `xlsx data-validations list`, `show`, `create`, `update`, and `delete`.
  The slice reads and mutates worksheet `dataValidations` XML in schema order,
  preserves Go-compatible `sqref` normalization including absolute markers,
  supports list values/ranges, formulas, operators, prompt/error attributes,
  dry-run, `--expect-type` / `--expect-formula1` guards, saved-output
  readback commands, and strict validation for saved XLSX outputs. Rust
  capabilities now advertise 117 Go-oracle command paths, leaving a pinned
  173-command gap. Serve mutation ops remain intentionally unwired for this
  direct-CLI-only worksheet validation slice. Proof: `cargo fmt --check`, `cargo check --all-targets`,
  focused `cargo test --test rust_contract_smoke xlsx_data_validations --
  --nocapture`, focused `cargo test --test rust_contract_smoke capabilities --
  --nocapture`, strict validation of generated create/update/delete XLSX
  outputs, Open XML SDK Office2019 schema validation (zero errors) for all
  three proof files, Excel COM open oracle for all three proof files, `cargo
  clippy --all-targets -- -D warnings`, and `cargo test --all-targets` passed
  with 4 ZIP guard unit tests plus 121 Rust contract tests.
- Rust XLSX sheet lifecycle mutation parity landed for direct
  `xlsx sheets add`, `xlsx sheets rename`, `xlsx sheets move`, and
  `xlsx sheets delete`. The slice preserves existing `sheets list/show`
  readback, creates worksheet parts and workbook relationships/content-types
  for added sheets, renames workbook sheet metadata, reorders sheet tabs while
  remapping workbook-view indexes, and deletes worksheet parts plus orphaned
  relationships/content-type overrides. Saved mutation JSON, dry-run
  templates, validation/readback command fields, required-flag and
  representative error behavior, emitted readback execution, capability
  indexing, and strict validation of saved outputs are covered against the Go
  oracle; the add test normalizes the oracle's intentionally variable new
  sheetId while preserving all structural invariants. Rust capabilities now
  advertise 112 Go-oracle command paths, leaving a pinned 178-command gap.
  Proof: `cargo fmt --check`, `cargo check --all-targets`, focused `cargo
  test --test rust_contract_smoke xlsx_sheets_ -- --nocapture`, focused
  capability ratchet/discovery tests, strict validation for saved
  add/rename/move/delete XLSX proof files, Open XML SDK Office2019 schema
  validation (zero errors) for all four proof files, Excel COM open oracle for
  all four proof files, `cargo clippy --all-targets -- -D warnings`, and
  `cargo test --all-targets` passed with 4 ZIP guard unit tests plus 118 Rust
  contract tests.
- Rust PPTX speaker-notes mutation capability advertisement and serve operation
  parity landed for the already-ported direct `pptx notes set` and
  `pptx notes clear` commands. Rust now advertises both leaves as
  `opCompatible=true`, wires serve ops through the existing notes mutation
  functions against the session working copy, preserves `pptx notes show`
  inspect/readback behavior, and leaves the Go-only `ooxml pptx notes` group
  path unadvertised because the Rust capability inventory lists implemented
  operational command paths rather than command groups. Rust capabilities now
  advertise 108 Go-oracle command paths, leaving a pinned 182-command gap.
  Proof: `cargo fmt --check`, `cargo check --all-targets`, focused
  `cargo test --test rust_contract_smoke pptx_notes -- --nocapture`, focused
  `cargo test --test rust_contract_smoke capabilit -- --nocapture`, strict
  validation for the serve-mutated PPTX output inside
  `serve_op_supports_pptx_notes_mutations`, strict validation for a generated
  `pptx notes set` proof deck, Open XML SDK Office2019 schema validation (zero
  errors) for that proof deck, PowerPoint COM open oracle on that proof deck,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets`
  passed with 4 ZIP guard unit tests plus 116 Rust contract tests.
- Rust XLSX filters/sorts remaining direct CLI parity landed for
  `xlsx filters-sorts clear-column-filter`, `xlsx filters-sorts set-sort`, and
  `xlsx filters-sorts clear-sort`. The slice removes worksheet filter-column
  criteria, creates/appends/replaces worksheet sortState conditions with
  Go-compatible single-column condition refs, clears worksheet sortState, and
  preserves Go-shaped mutation JSON/readback/validation command fields with
  dry-run and representative error parity. Rust capabilities now advertise 106
  Go-oracle command paths, leaving a pinned 184-command gap. Serve mutation ops
  remain intentionally unwired for this direct-CLI-only filters/sorts mutation
  slice. Proof: `cargo fmt --check`, `cargo check --all-targets`, focused
  `cargo test --test rust_contract_smoke xlsx_filters_sorts -- --nocapture`,
  focused capability ratchet/discovery tests, strict validation for generated
  clear-column-filter/set-sort/clear-sort XLSX proof files, Open XML SDK
  Office2019 schema validation (zero errors) for the proof files, Excel COM
  open oracle for all three proof files, `cargo clippy --all-targets -- -D
  warnings`, and `cargo test --all-targets` passed with 4 ZIP guard unit tests
  plus 115 Rust contract tests.
- Rust PPTX table serve/MCP operation dispatch is wired for the already-ported
  `pptx tables set-cell`, `pptx tables delete-row`, and
  `pptx tables insert-row` direct CLI mutations. Serve ops now call the existing
  Rust PPTX table mutation functions against the session working copy, preserve
  direct-CLI-shaped plan argv/readback fields, and advertise
  `opCompatible=true` only for those three table mutation commands; `pptx tables
  show` remains inspect-only. Rust capabilities still advertise 103 Go-oracle
  command paths, leaving a pinned 187-command gap. Proof: `cargo fmt --check`,
  `cargo check --all-targets`, focused `cargo test --test rust_contract_smoke
  serve_op_supports_pptx_table_mutations -- --nocapture`, focused `cargo test
  --test rust_contract_smoke capabilities_advertise_supported_web_agent_surface
  -- --nocapture`, strict validation for the saved PPTX serve output, Open XML
  SDK Office2019 schema validation (zero errors), PowerPoint COM open oracle,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets`
  passed with 4 ZIP guard unit tests plus 113 Rust contract tests.
- Rust VBA source readback parity landed for direct `vba inspect-bin`,
  `vba list`, and `vba extract`. The slice ports a read-only CFB/MS-OVBA reader
  for parseable `vbaProject.bin` payloads, reports source-module selectors,
  decoded-source hashes, line metadata, host-family compatibility warnings, and
  extracts `.bas`/`.cls` source files. Source-changing VBA mutation, `vba
  create`, and `vba office-check` remain unadvertised in Rust until their
  Office-facing proof is owned by a mutation lane. Rust capabilities now
  advertise 103 Go-oracle command paths, leaving a pinned 187-command gap.
  Proof: `cargo fmt --check`, `cargo check --all-targets`, focused `cargo test
  --test rust_contract_smoke vba -- --nocapture`, focused capability
  ratchet/discovery tests, `cargo clippy --all-targets -- -D warnings`, and
  `cargo test --all-targets` passed with 4 ZIP guard unit tests plus 112 Rust
  contract tests. Office/Open XML mutation proof was not run because this slice
  is readback/source extraction only.
- Rust XLSX worksheet dimension mutation parity landed for direct
  `xlsx colwidths set` and `xlsx rowheights set`. The slice preserves existing
  column span attributes while carving target widths, creates missing row
  records for height updates, supports dry-run and stale `--expect-width` /
  `--expect-height` guards like the Go oracle, and is wired through capability
  discovery plus serve operation dispatch. Rust capabilities now advertise 100
  Go-oracle command paths, leaving a pinned 190-command gap. Proof: `cargo fmt
  --check`, `cargo check --all-targets`, focused `cargo test --test
  rust_contract_smoke xlsx_dimension_setters -- --nocapture`, focused
  capability ratchet/discovery tests, strict validation for generated
  colwidth/rowheight XLSX proof files, Open XML SDK Office2019 schema
  validation (zero errors) for both proof files, Excel COM open oracle for both
  proof files, `cargo clippy --all-targets -- -D warnings`, and `cargo test
  --all-targets` passed with 4 ZIP guard unit tests plus 111 Rust contract
  tests.
- Rust XLSX filters/sorts parity expanded for direct
  `xlsx filters-sorts clear-autofilter` and
  `xlsx filters-sorts add-column-filter`. The slice removes worksheet/table
  autoFilter state with `--expect-range` guards, adds worksheet value/custom
  column filters with `--expect-filter` guards, preserves Go-shaped mutation
  JSON/readback/validation command fields, and covers saved output/readback,
  dry-run non-mutation, table clear, and representative error behavior with
  Go-vs-Rust contract tests. Rust capabilities now advertise 98 Go-oracle
  command paths, leaving a pinned 192-command gap. Serve mutation ops remain
  intentionally unwired for this direct-CLI-only filters/sorts mutation slice.
  Proof: `cargo fmt --check`, `cargo check --all-targets`, focused `cargo test
  --test rust_contract_smoke xlsx_filters_sorts -- --nocapture`, focused
  capability ratchet/discovery tests, strict validation for generated
  clear/add-column-filter XLSX proof files, Open XML SDK Office2019 schema
  validation (zero errors) for both proof files, Excel COM open oracle for both
  proof files, `cargo clippy --all-targets -- -D warnings`, and `cargo test
  --all-targets` passed with 4 ZIP guard unit tests plus 109 Rust contract
  tests.
- Rust PPTX table row insertion parity landed for direct
  `pptx tables insert-row`. The slice inserts an empty row into a selected table
  graphic frame, preserves generated destination/readback command fields,
  rejects out-of-range insertion points and unsafe vertical-merge splits like
  the Go oracle, and matches Go-oracle saved output, `pptx tables show`
  readback, dry-run, and representative error behavior. Rust capabilities now
  advertise 96 Go-oracle command paths, leaving a pinned 194-command gap; the
  command is advertised with `opCompatible=false` because serve/MCP operation
  dispatch for PPTX table mutations is not wired yet. Proof: `cargo fmt
  --check`, `cargo check --all-targets`, focused `cargo test --test
  rust_contract_smoke pptx_tables_insert_row -- --nocapture`, focused
  capability ratchet/discovery tests, strict validation for the generated PPTX
  proof file, Open XML SDK Office2019 schema validation (zero errors),
  PowerPoint COM open oracle, `cargo clippy --all-targets -- -D warnings`, and
  `cargo test --all-targets` passed with 4 ZIP guard unit tests plus 107 Rust
  contract tests.
- Rust XLSX comments parity landed for direct
  `xlsx comments list/add/update/remove` plus serve operations for
  add/update/remove and serve inspect for list. The slice creates and updates
  worksheet comments parts, worksheet relationships, content-type overrides,
  VML drawing parts, and `<legacyDrawing>` refs; it removes orphaned comments
  and VML parts when the last comment is deleted; and it matches Go-oracle
  JSON, readback commands, dry-run, hash-guard, duplicate-cell, and missing
  comment behavior. Rust capabilities now advertise 95 Go-oracle command paths,
  leaving a pinned 195-command gap. Proof: `cargo fmt --check`, `cargo check
  --all-targets`, focused `cargo test --test rust_contract_smoke xlsx_comments
  -- --nocapture`, focused capability ratchet/discovery tests, strict
  validation for generated add/update/remove XLSX proof files, Open XML SDK
  Office2019 schema validation (zero errors) for all three proof files, Excel
  COM open oracle for all three proof files, `cargo clippy --all-targets --
  -D warnings`, and `cargo test --all-targets` passed with 4 ZIP guard unit
  tests plus 106 Rust contract tests.
- Rust PPTX comment mutation parity landed for direct
  `pptx comments add`, `pptx comments edit`, and `pptx comments remove`.
  The slice creates legacy slide comments and shared comment-author parts,
  reuses or creates authors, edits by stable comment handle or compound
  `comment-id`/`author-id`, removes the final per-slide comments part and
  relationship, emits comment-list readback plus validate/render command
  templates, and matches Go-oracle dry-run, hash-guard, missing-target, and
  invalid-argument behavior. Rust capabilities now advertise 91 Go-oracle
  command paths, leaving a pinned 199-command gap; these commands are direct
  CLI mutations with `opCompatible=false` because PPTX comment serve/MCP op
  dispatch is not wired in this slice. Proof: `cargo check --all-targets`,
  focused `cargo test --test rust_contract_smoke
  pptx_comments_add_edit_remove_saved_readback_dry_run_and_errors_match_go_oracle
  -- --nocapture`, Rust strict validation through the emitted validate commands
  for saved add/edit/remove PPTX outputs, Open XML SDK Office2019 schema
  validation (zero errors) for generated add/edit/remove proof files,
  PowerPoint COM open oracle for all three proof files, `cargo clippy
  --all-targets -- -D warnings`, and `cargo test --all-targets` passed with 4
  ZIP guard unit tests plus 104 Rust contract tests.
- Rust DOCX image mutation parity landed for direct `docx images replace` and
  `docx images insert`. The slice replaces image payloads in place or via a new
  media part when content type changes, resizes existing inline drawings, inserts
  a new inline image paragraph before/after body blocks, enforces Go-compatible
  hash guards, dry-run, and target-not-found errors, and keeps serve/MCP op
  compatibility disabled until an image-mutation op pattern is established. Rust
  capabilities now advertise 88 Go-oracle command paths, leaving a pinned
  202-command gap. Proof: `cargo fmt --check`, `cargo check --all-targets`,
  focused `cargo test --test rust_contract_smoke docx_images -- --nocapture`,
  focused capability ratchet/discovery tests, `cargo clippy --all-targets --
  -D warnings`, strict validation/readback for generated Go and Rust DOCX
  outputs in the contract harness, strict validation for generated replace and
  insert DOCX proof files, Open XML SDK Office2019 schema validation (zero
  errors) for both proof files, Word COM open oracle for both proof files, and
  `cargo test --all-targets` passed with 4 ZIP guard unit tests plus 103 Rust
  contract tests.
- Rust XLSX filters/sorts parity landed for direct
  `xlsx filters-sorts show` and `xlsx filters-sorts set-autofilter`. The slice
  reads worksheet/table autoFilter state plus worksheet sortState, adds or
  replaces worksheet/table autoFilter refs, preserves existing filter columns
  when only the ref changes, emits Go-matching honesty/readback/validation
  command fields, and covers saved output, dry-run, invalid range, table target,
  and serve inspect readback behavior with Go-vs-Rust contract tests. Rust
  capabilities now advertise 86 Go-oracle command paths, leaving a pinned
  204-command gap. Proof: `cargo fmt --check`, `cargo check --all-targets`,
  focused `cargo test --test rust_contract_smoke xlsx_filters_sorts --
  --nocapture`, focused serve inspect coverage, focused capability
  ratchet/discovery tests, strict Rust validation for a generated XLSX, Open
  XML SDK Office2019 schema validation (zero errors), Excel COM open oracle,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets`
  passed with 4 ZIP guard unit tests plus 102 Rust contract tests.
- Rust PPTX table cell mutation parity landed for direct
  `pptx tables set-cell`. The slice sets or clears one table cell by slide plus
  table selector/shape ID, preserves the existing table destination/readback
  command contract, supports `--text-file`, and matches Go-oracle saved output,
  `pptx tables show` readback, dry-run, and representative error behavior. Rust
  capabilities now advertise 84 Go-oracle command paths, leaving a pinned
  206-command gap; the command is advertised with `opCompatible=false` because
  serve/MCP operation dispatch for PPTX table mutations is not wired yet. Proof:
  `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy
  --all-targets -- -D warnings`, focused `cargo test --test rust_contract_smoke
  pptx_tables_set_cell_saved_readback_dry_run_text_file_and_errors_match_go_oracle
  -- --nocapture`, focused capability ratchet/MCP command-resource tests, Rust
  strict validation for a generated PPTX with zero diagnostics, Open XML SDK
  Office2019 schema validation (zero errors), PowerPoint COM open oracle, and
  `cargo test --all-targets` passed with 4 ZIP guard unit tests plus 98 Rust
  contract tests.
- Rust DOCX table row insertion parity landed for direct
  `docx tables insert-row`. The slice clones an existing main-document table row
  structure, clears inserted cell text, rejects stale hashes, bad row targets,
  and merged tables like the Go oracle, and is wired through DOCX serve
  operation dispatch. Rust capabilities now advertise 83 Go-oracle command
  paths, leaving a pinned 207-command gap. Proof: `cargo fmt --check`,
  `cargo check --all-targets`, focused `cargo test --test rust_contract_smoke
  docx_tables_insert_row -- --nocapture`, focused capability ratchet/discovery
  tests, focused serve session coverage for set-cell, clear-cell, insert-row,
  delete-row, commit, strict validation, and readback; strict repo validation
  for a generated DOCX, Open XML SDK Office2019 schema validation (zero
  errors), Word COM open oracle, `cargo clippy --all-targets -- -D warnings`,
  and `cargo test --all-targets` passed with 4 ZIP guard unit tests plus 97
  Rust contract tests.
- Rust PPTX table row deletion parity landed for direct
  `pptx tables delete-row`. The slice deletes one row from a selected table
  graphic frame, rejects unsafe/out-of-range rows like the Go oracle, emits
  destination readback plus validate/render command templates, and has
  Go-vs-Rust coverage for saved mutation, `pptx tables show` readback,
  dry-run, and error behavior. Rust capabilities now advertise 82 Go-oracle
  command paths, leaving a pinned 208-command gap; the command is advertised
  with `opCompatible=false` because serve/MCP operation dispatch for PPTX table
  mutations is not wired yet. Proof: `cargo fmt --check`, `cargo check
  --all-targets`, focused `cargo test --test rust_contract_smoke
  pptx_tables_delete_row_saved_readback_dry_run_and_errors_match_go_oracle --
  --nocapture`, focused capability ratchet/MCP resource tests, strict repo
  validation for a generated PPTX, Open XML SDK Office2019 schema validation
  (zero errors), PowerPoint COM open oracle, `cargo clippy --all-targets --
  -D warnings`, and `cargo test --all-targets`.
- Rust PPTX speaker-notes mutation parity landed for direct
  `pptx notes set` and `pptx notes clear`. The slice can create a missing
  notes slide and notes master relationship graph, update existing notes,
  clear notes, emit readback/validate/render commands, and match Go-oracle
  dry-run and error behavior. This is direct CLI surface only, so the Rust
  capability ratchet remains 80 Go-oracle command paths with a pinned
  210-command gap. Proof: `cargo fmt --check`, `cargo check --all-targets`,
  focused `cargo test --test rust_contract_smoke pptx_notes -- --nocapture`,
  strict repo validation for generated set/clear PPTX outputs, Open XML SDK
  Office2019 schema validation (zero errors) for both outputs, PowerPoint COM
  open oracle for both outputs, `cargo clippy --all-targets -- -D warnings`,
  and `cargo test --all-targets` passed with 4 ZIP guard unit tests plus 94
  Rust contract tests.
- Rust XLSX row-height readback parity landed for direct
  `xlsx rowheights show`. The slice reports default and explicit worksheet row
  heights, hidden/custom flags, normalized row spans, uniformity, and the
  generated `rowheights set` command template through the shared
  `src/xlsx_dimensions.rs` module. It is read-only, so no Office/Open XML
  mutation proof is required. Rust capabilities now advertise 81 Go-oracle
  command paths, leaving a pinned 209-command gap. Proof: `cargo fmt --check`,
  `cargo check --all-targets`, focused `cargo test --test rust_contract_smoke
  xlsx_rowheights_show -- --nocapture`, focused capability ratchet/discovery
  tests, `cargo clippy --all-targets -- -D warnings`, and `cargo test
  --all-targets` passed with 4 ZIP guard unit tests plus 95 Rust contract
  tests.
- Rust XLSX column-width readback parity landed for direct
  `xlsx colwidths show`. The slice reports default and explicit worksheet
  column widths, hidden/custom flags, normalized column spans, uniformity, and
  the generated `colwidths set` command template from a focused
  `src/xlsx_dimensions.rs` module. It is read-only, so no Office/Open XML
  mutation proof is required. Rust capabilities now advertise 80 Go-oracle
  command paths, leaving a pinned 210-command gap. Proof: `cargo fmt --check`,
  `cargo check --all-targets`, focused `cargo test --test rust_contract_smoke
  xlsx_colwidths_show -- --nocapture`, focused capability ratchet/discovery
  tests, `cargo clippy --all-targets -- -D warnings`, and `cargo test
  --all-targets` passed with 4 ZIP guard unit tests plus 93 Rust contract
  tests.
- Rust DOCX table row deletion parity landed for direct
  `docx tables delete-row`. The slice deletes one hash-guarded main-document
  table row, rejects merged tables and last-row deletion like the Go oracle,
  emits the existing table validate/show/list readback commands, and is wired
  through DOCX serve operation dispatch to match the advertised op-compatible
  leaf. Rust capabilities now advertise 79 Go-oracle command paths, leaving a
  pinned 211-command gap. Proof: `cargo fmt --check`, `cargo check
  --all-targets`, focused `cargo test --test rust_contract_smoke
  docx_tables_delete_row -- --nocapture`, and focused capability subset test;
  focused serve session test covering set-cell, clear-cell, delete-row, commit,
  strict validation, and readback; strict repo validation, Open XML SDK
  Office2019 schema validation (zero errors), and Word COM open oracle on the
  generated row-deleted DOCX.
- Merged current `origin/master` hardening, including OPC inflate limits, CFB
  traversal guards, and new Go ingest fuzz harnesses.
- Mirrored the Go OPC inflate hardening in Rust shared ZIP I/O: package opens
  now reject oversized declared uncompressed ZIP parts and total packages,
  `zip_text` reads through a hard cap, and ZIP mutation-copy paths stream
  existing entries through the same per-part ceiling.
- Repaired the Windows Rust contract comparator for quoted and JSON-escaped
  temp paths after the merge, keeping the frozen Go contract stable on Windows.
- First de-monolithization seam landed: the Rust capability inventory moved from
  `src/main.rs` into `src/capabilities.rs` with no behavior changes.
- Foundational CLI core types moved from `src/main.rs` into `src/cli_core.rs`,
  giving future command-family modules a small shared error/result/flag surface.
- Shared CLI argument parsing helpers moved from `src/main.rs` into
  `src/cli_args.rs`, reducing future command-family coupling.
- CLI command dispatch moved from `src/main.rs` into `src/cli_dispatch.rs`,
  leaving `main.rs` as executable entrypoint, global flag parsing, and crate
  facade exports. The DOCX block-hash validator moved with the router and is
  re-exported for `serve`.
- JSON argument/resource helpers moved from `src/main.rs` into
  `src/json_util.rs`, giving serve/MCP and future command modules a shared
  typed JSON parsing and stable field-serialization surface.
- Generated command quoting moved from `src/main.rs` into `src/command_text.rs`,
  preserving one shared readback-command quoting contract across DOCX, XLSX,
  PPTX, serve, and MCP output.
- Capability command inventory and local flag metadata moved from
  `src/capabilities.rs` into `src/capabilities/commands.rs`, leaving the
  capability facade focused on filtering and top-level document assembly.
- Shared OPC relationship/content-type helpers moved from `src/main.rs` into
  `src/opc.rs`, creating a common package substrate for future DOCX, XLSX, and
  PPTX module splits.
- ZIP package read/write helpers moved from `src/main.rs` into `src/zip_io.rs`,
  separating shared package I/O from command-family logic while preserving the
  existing mutation copy path; ZIP entry-existence checks now live there too.
- Core XML attribute, namespace, and escape/unescape helpers moved from
  `src/main.rs` into `src/xml_util.rs`, giving future OOXML modules a shared
  lexical XML layer.
- Shared XML attribute rendering, decoded attribute maps, whitespace-preserve,
  and simple span replacement helpers also moved into `src/xml_util.rs`,
  reducing duplicated XML utility coupling before command-family splits.
- Runtime timestamp/counter and mutation temp-path helpers moved from
  `src/main.rs` into `src/runtime_util.rs`, keeping generated dates and
  mutation scratch paths in one shared utility module.
- Shared selector de-duplication and candidate suggestion helpers moved from
  `src/main.rs` into `src/selector_util.rs`.
- OPC package mutation helpers for root relationships, content-type overrides,
  relationship XML insertion, and relative relationship targets moved from
  `src/main.rs` into `src/opc.rs`.
- Validation report and diagnostics logic moved from `src/main.rs` into
  `src/validation.rs`, separating package validation from command dispatch and
  document-family mutation code.
- The `verify` command wrapper and lightweight package validation summary moved
  from `src/main.rs` into `src/verify.rs`, while shared package type detection
  remains at the crate facade.
- OOXML package kind detection and DOCX/XLSX part-classification helpers moved
  from `src/main.rs` into `src/package_discovery.rs`, giving inspect,
  validation, and document-family commands a shared discovery layer. The
  lightweight package-family fallback helper also now lives in this module.
- The `inspect` command and its DOCX/XLSX/PPTX summary helpers moved from
  `src/main.rs` into `src/inspect.rs`, separating package summary reporting from
  the remaining command-family implementations.
- PPTX slide, shape, text, comments, masters, layouts, notes, table, and diff
  readback/reporting helpers moved from `src/main.rs` into
  `src/pptx_readback.rs`, leaving mutation/render orchestration at the crate
  root.
- PPTX comment readback, author discovery, stable selector/hash reporting, and
  comment-target errors split from `src/pptx_readback.rs` into
  `src/pptx_readback/comments.rs`, while the crate-facing command remains
  re-exported through the PPTX readback facade.
- PPTX slide-part reference discovery moved from `src/pptx_readback.rs` into
  `src/pptx_readback/slide_parts.rs`, giving comments, notes, and table
  readback a shared child-module helper without widening the crate facade.
- PPTX notes extraction/show reporting moved from `src/pptx_readback.rs` into
  `src/pptx_readback/notes.rs`, reusing the shared slide-part helper and parent
  shape text parser through the PPTX readback facade.
- PPTX text extraction, text JSON rendering, and slide text snapshots moved from
  `src/pptx_readback.rs` into `src/pptx_readback/text.rs`, leaving `pptx diff`
  at the readback facade while sharing the same shape model parser.
- PPTX shared shape model types, shape parsing, selector generation, placeholder
  metadata, bounds rendering, and slide object counting moved from
  `src/pptx_readback.rs` into `src/pptx_readback/shape_model.rs`, removing the
  readback facade as a hidden dependency hub for text, notes, layouts, and
  tables.
- PPTX master/layout/theme readback moved from `src/pptx_readback.rs` into
  `src/pptx_readback/layouts.rs`, preserving the existing crate-facing
  `pptx masters` and `pptx layouts` command facade.
- PPTX table readback and table-detail JSON rendering moved from
  `src/pptx_readback.rs` into `src/pptx_readback/tables.rs`, reusing the shared
  slide-part helper and shape model parser through the PPTX readback facade.
- PPTX render orchestration, slide-list parsing, mock render output, and local
  `soffice`/`pdftoppm` invocation helpers moved from `src/main.rs` into
  `src/pptx_render.rs`, leaving PPTX text mutation and serve routing at the
  crate root.
- PPTX replace-text CLI, in-place mutation, and serve readback helpers moved
  from `src/main.rs` into `src/pptx_mutation.rs`, while the serve operation
  router remains at the crate root.
- MCP tool response shaping, resource schemas, capability resources, command
  resource lookup, and URI decoding moved from `src/main.rs` into
  `src/mcp_support.rs`, leaving MCP state logic separate from protocol support
  helpers.
- MCP stdio runner and protocol state moved from `src/main.rs` into
  `src/mcp.rs`, and the serve/session engine, JSON-RPC routing, commit/abort
  flow, and working-copy management moved from `src/main.rs` into
  `src/serve.rs`; stored-operation modeling, planned argv rendering, and
  committed readback rewriting now live in `src/serve/op.rs`.
- Serve inspect command dispatch for XLSX, DOCX, and PPTX read-only session
  commands moved from `src/serve.rs` into `src/serve/inspect.rs`, leaving
  session lookup, RPC response framing, and mutation operations in the serve
  facade.
- Serve operation command dispatch for XLSX, DOCX, and PPTX session mutations
  moved from `src/serve.rs` into `src/serve/op_dispatch.rs`, leaving op
  indexing, readback framing, session commit/abort, and working-copy ownership
  in the serve facade.
- XLSX serve operation dispatch for range/cell/workbook-metadata session
  mutations split from `src/serve/op_dispatch.rs` into
  `src/serve/op_dispatch/xlsx.rs`, keeping the top-level serve op dispatcher
  responsible for family routing and unsupported-command errors.
- DOCX serve operation dispatch for header/footer, field, paragraph, style,
  block, comment, and table session mutations split from
  `src/serve/op_dispatch.rs` into `src/serve/op_dispatch/docx.rs`, leaving the
  top-level serve op dispatcher as family routing plus the remaining PPTX text
  replacement operation.
- DOCX serve operation dispatch for table `set-cell` and `clear-cell` moved
  from `src/serve/op_dispatch/docx.rs` into
  `src/serve/op_dispatch/docx/tables.rs`, preserving table coordinate
  validation, plan flag ordering, `DocxTablesOp` readback shaping, and
  unsupported-command fallback behavior.
- DOCX serve operation dispatch for comment `add`, `edit`, and `remove` moved
  from `src/serve/op_dispatch/docx.rs` into
  `src/serve/op_dispatch/docx/comments.rs`, preserving handle/comment-id
  validation, `textFile` alias handling, plan flag ordering,
  `DocxCommentsOp` readback shaping, and unsupported-command fallback
  behavior.
- DOCX serve operation dispatch for field `insert` and `set-result` moved from
  `src/serve/op_dispatch/docx.rs` into
  `src/serve/op_dispatch/docx/fields.rs`, preserving required argument
  validation, `fieldCode`/`expectHash` alias handling, plan flag ordering,
  `DocxFieldsOp` readback shaping, and unsupported-command fallback behavior.
- DOCX serve operation dispatch for paragraph `append`, `insert`, `set`, and
  `clear` moved from `src/serve/op_dispatch/docx.rs` into
  `src/serve/op_dispatch/docx/paragraphs.rs`, preserving handle/index
  validation, required text resolution, plan flag ordering,
  `DocxParagraphsOp` readback shaping, and unsupported-command fallback
  behavior.
- DOCX serve operation dispatch for style `apply` moved from
  `src/serve/op_dispatch/docx.rs` into
  `src/serve/op_dispatch/docx/styles.rs`, preserving paragraph/table target
  validation, handle/index exclusivity, style validation flags, plan flag
  ordering, `DocxStylesOp` readback shaping, and unsupported-command fallback
  behavior.
- DOCX serve operation dispatch for block `replace`, `delete`, and
  `insert-after` moved from `src/serve/op_dispatch/docx.rs` into
  `src/serve/op_dispatch/docx/blocks.rs`, preserving block/hash validation,
  hash rules for insertion at document start, text/style alias handling, plan
  flag ordering, `DocxBlocksOp` readback shaping, and unsupported-command
  fallback behavior.
- DOCX serve operation dispatch for header/footer `set-text` moved from
  `src/serve/op_dispatch/docx.rs` into
  `src/serve/op_dispatch/docx/headers_footers.rs`, preserving header/footer
  kind routing, section/index/default-reference handling, selector and
  `textFile` alias behavior, plan flag ordering, `DocxHeaderFooterSetText`
  readback shaping, and unsupported-command fallback behavior.
- The top-level serve op dispatcher now routes XLSX and DOCX commands by
  family prefix, leaving exact command matching and unsupported-command
  fallbacks inside each child dispatcher.
- DOCX CLI dispatch for text, block, style, comment, field, header/footer,
  image, table, and paragraph commands moved from `src/cli_dispatch.rs` into
  `src/cli_dispatch/docx.rs`, leaving the top-level CLI dispatcher responsible
  for core command routing plus PPTX/XLSX families.
- DOCX CLI table command dispatch for `show`, `set-cell`, and `clear-cell`
  moved from `src/cli_dispatch/docx.rs` into
  `src/cli_dispatch/docx/tables.rs`, preserving exact table flag validation,
  mutation option wiring, and unsupported-command fallback text.
- DOCX CLI paragraph command dispatch for `append`, `insert`, `set`, and
  `clear` moved from `src/cli_dispatch/docx.rs` into
  `src/cli_dispatch/docx/paragraphs.rs`, preserving paragraph handle/index
  validation, required text resolution, mutation option wiring, and
  unsupported-command fallback text.
- The static capability command inventory moved from
  `src/capabilities/commands.rs` into family modules under
  `src/capabilities/commands/`, preserving the emitted command order as core,
  PPTX, XLSX, then DOCX for CLI capabilities and MCP command-resource lookups.
- The DOCX capability inventory split further into body/block, paragraph,
  style, comment, field, header/footer, image, and table submodules under
  `src/capabilities/commands/docx/`, keeping the DOCX capability facade as an
  ordered family registry.
- XLSX workbook metadata inspect/update types, XML readers, property renderers,
  and calc-setting mutation helpers moved from `src/main.rs` into
  `src/xlsx_metadata.rs`, keeping CLI and serve call sites stable through the
  crate facade.
- XLSX workbook `calcPr` parsing/updating and workbook child-order logic moved
  from `src/xlsx_metadata.rs` into `src/xlsx_metadata/calc.rs`, keeping the
  metadata facade responsible for orchestration and shared XML insertion.
- XLSX core/app document-properties XML reading, rendering, namespace repair,
  and direct-child update helpers moved from `src/xlsx_metadata.rs` into
  `src/xlsx_metadata/props_xml.rs`, while the facade keeps workbook metadata
  orchestration and shared insertion ordering for the calc child module.
- Rust XLSX workbook metadata creation now emits the Open XML SDK-expected OPC
  core-properties content type
  `application/vnd.openxmlformats-package.core-properties+xml`; the Rust
  contract test now asserts the invalid `officedocument.core-properties` type
  is not written.
- The Go reference XLSX workbook metadata emitter and XML-part classifier now
  use the same standards-correct core-properties content type. Regression
  coverage asserts fresh Go-created `docProps/core.xml` parts avoid the invalid
  legacy MIME, and the produced workbook passes strict `ooxml validate`, .NET
  OpenXML SDK validation, and the Windows Excel oracle.
- The Rust contract smoke test monolith started its B9 split: XLSX command-family
  parity tests moved from `tests/rust_contract_smoke.rs` into
  `tests/rust_contract_smoke/xlsx.rs`, preserving the shared Go-oracle helpers
  and the 78-test contract count.
- Capability inventory and filter contract tests moved from
  `tests/rust_contract_smoke.rs` into
  `tests/rust_contract_smoke/capabilities.rs`, keeping shared capability helper
  assertions in the parent harness while preserving the 78-test contract count.
- The frozen PPTX mutation/render/verify contract test moved from
  `tests/rust_contract_smoke.rs` into `tests/rust_contract_smoke/pptx.rs`,
  preserving the shared baseline/process helpers and the 78-test contract count.
- Serve/session contract tests moved from `tests/rust_contract_smoke.rs` into
  `tests/rust_contract_smoke/serve.rs`, keeping shared JSON-RPC and scrub
  helpers in the parent harness while preserving the 78-test contract count.
- DOCX command-family parity tests moved from `tests/rust_contract_smoke.rs`
  into `tests/rust_contract_smoke/docx.rs`, preserving shared Go-oracle helper
  access and the 78-test contract count.
- MCP and web-smoke agent-surface contract tests moved from
  `tests/rust_contract_smoke.rs` into
  `tests/rust_contract_smoke/agent_surface.rs`, leaving shared protocol
  helpers in the parent harness while preserving the 78-test contract count.
- XLSX formula recalculation metadata updates, calcChain content-type cleanup,
  workbook relationship cleanup, and calcChain part removal moved from
  `src/xlsx_mutation.rs` into `src/xlsx_formula_recalc.rs`, with the mutation
  module passing only the formula state needed by that package-update layer.
- XLSX defined-name model, list/show commands, selector resolution, handle
  parsing, JSON rendering, and readback-command helpers moved from `src/main.rs`
  into `src/xlsx_names.rs`.
- XLSX table model, list/show/export commands, relationship scanning,
  table-part parsing, selector resolution, table readback-command templates,
  and XLSX-to-PPTX source command templates moved from `src/main.rs` into
  `src/xlsx_tables.rs`.
- XLSX sheet/cell read commands for `xlsx cells extract` and `xlsx sheets
  list/show` moved from `src/main.rs` into `src/xlsx_sheets.rs`, leaving the
  shared worksheet parser in place for later, lower-risk extraction.
- DOCX image listing, relationship-target resolution, drawing scan state, and
  image reference extraction moved from `src/main.rs` into `src/docx_images.rs`.
- DOCX comment list/add/edit/remove commands, comment-part discovery,
  comment-handle parsing, marker insertion/removal, comments-part rendering,
  and comment content hashing moved from `src/main.rs` into
  `src/docx_comments.rs`.
- DOCX comment handle parsing/resolution moved from `src/docx_comments.rs`
  into `src/docx_comments/handles.rs`, preserving a single `pub(super)`
  resolver entry point while keeping handle parsing private to the child
  module.
- DOCX comment XML rendering, comment-part append, new-comment id selection,
  and document comment marker insertion moved from `src/docx_comments.rs` into
  `src/docx_comments/render.rs`, keeping mutation entrypoints in the facade and
  preserving direct DOCX/serve comment contract coverage.
- DOCX comment marker removal and XML range deletion moved from
  `src/docx_comments.rs` into `src/docx_comments/markers.rs`, keeping
  comment removal orchestration in the facade and preserving direct DOCX/serve
  comment contract coverage.
- DOCX comment list readback, comment/anchor parsing, list JSON rendering,
  content hashing, and fragment readback moved from `src/docx_comments.rs`
  into `src/docx_comments/read.rs`, leaving add/edit/remove orchestration and
  package wiring in the comments facade.
- DOCX body block readers, rich block reports, paragraph/table text extraction,
  run formatting capture, namespace-aware paragraph handles, and block content
  hashing moved from `src/main.rs` into `src/docx_block_readers.rs`.
- DOCX rich body-block reporting, run formatting capture, namespace-aware
  paragraph-handle counting, table merge detection, and block content hashing
  moved from `src/docx_block_readers.rs` into
  `src/docx_block_readers/rich.rs`, leaving the original block-reader facade
  responsible for simple paragraph/table extraction and shared namespace
  helpers.
- DOCX text and body block command wrappers for show, insert-after, replace,
  and delete moved from `src/main.rs` into `src/docx_block_commands.rs`, with
  shared body/paragraph XML helpers now provided by `src/docx_xml.rs` through
  the crate facade.
- DOCX paragraph append/insert/set/clear command wrappers and required
  set-text argument validation moved from `src/main.rs` into
  `src/docx_paragraph_commands.rs`, using the shared `src/docx_xml.rs` body XML
  and handle mutation helpers also used by styles.
- Shared DOCX mutation primitives for output path resolution, package writes,
  text-file resolution, DOCX package guards, strict-validate command text, and
  paragraph handle errors/resolution moved from `src/main.rs` into
  `src/docx_mutation_core.rs`.
- DOCX style list/show/apply commands, style catalog parsing, style handle
  parsing, and style-specific XML rewrite helpers moved from `src/main.rs`
  into `src/docx_styles.rs`, while shared body/table XML helpers live in
  `src/docx_xml.rs`.
- DOCX style list/show readback, styles-part discovery, style catalog parsing,
  style type filtering, and list/show JSON rendering moved from
  `src/docx_styles.rs` into `src/docx_styles/read.rs`, leaving style apply
  validation and XML mutation in the DOCX styles facade.
- DOCX style apply XML rewriting, style child rendering, table/paragraph/run
  style fragment mutation, and previous-style fragment readback moved from
  `src/docx_styles.rs` into `src/docx_styles/mutation.rs`, leaving the facade
  focused on command orchestration, target validation, and result shaping.
- DOCX field listing, field insertion, cached-result mutation, field-location
  parsing, simple/complex field detection, and field XML rewrite helpers moved
  from `src/main.rs` into `src/docx_fields.rs`.
- DOCX field list command routing, document/header/footer field scanning,
  simple/complex field readback, field filtering, and list JSON rendering moved
  from `src/docx_fields.rs` into `src/docx_fields/read.rs`, leaving insert and
  cached-result mutation helpers in the DOCX fields facade.
- DOCX header/footer list/show/set-text commands, selector parsing,
  section/reference creation, relationship/content-type wiring, part templates,
  and readback command generation moved from `src/main.rs` into
  `src/docx_headers.rs`, using shared paragraph-fragment text extraction from
  `src/docx_xml.rs` through the crate facade.
- DOCX header/footer selector parsing, reference-info JSON reconstruction, and
  paragraph-selector generation moved from `src/docx_headers.rs` into
  `src/docx_headers/selectors.rs`, leaving the command facade and mutation
  orchestration in `src/docx_headers.rs`.
- DOCX header/footer part-URI discovery moved from `src/docx_headers.rs` into
  `src/docx_headers/parts.rs`, preserving the existing crate-facing
  `docx_header_footer_part_uris` facade while keeping the relationship
  reference helper private to the child module.
- DOCX header/footer paragraph extraction and JSON rendering moved from
  `src/docx_headers.rs` into `src/docx_headers/paragraphs.rs`, leaving the
  facade responsible for list/show/set-text orchestration while preserving the
  direct DOCX and serve header/footer contract coverage.
- DOCX header/footer section parsing, reference normalization, and list JSON
  rendering moved from `src/docx_headers.rs` into
  `src/docx_headers/sections.rs`, keeping mutation orchestration in the facade
  and preserving header/footer list/show/serve contract coverage.
- DOCX header/footer list/show command handling, package/type guards,
  relationship target collection, selector resolution, and paragraph readback
  wiring moved from `src/docx_headers.rs` into `src/docx_headers/read.rs`,
  leaving set-text mutation orchestration in the header/footer facade.
- DOCX header/footer paragraph text replacement, header/footer root-tag
  parsing, previous-text capture, and set-text XML mutation moved from
  `src/docx_headers.rs` into `src/docx_headers/text_mutation.rs`, preserving the
  existing crate-facing root-tag helper for field edits.
- DOCX table show/set-cell/clear-cell commands, table summary rendering,
  table-cell XML rewrites, and table readback command generation moved from
  `src/main.rs` into `src/docx_tables.rs`.
- Shared DOCX Word XML constants, paragraph/text fragment readers, body block
  range walkers, paragraph rendering/insertion/replacement, table scaffolding,
  `w14:paraId` stamping, and namespace helpers moved from `src/main.rs` into
  `src/docx_xml.rs`, while existing command modules keep importing through the
  crate facade.
- DOCX table scaffold helpers moved from `src/docx_xml.rs` into
  `src/docx_xml/table_scaffold.rs`, keeping
  `ensure_docx_body_table_scaffolds_xml` and
  `ensure_docx_table_scaffold_fragment` available through the same
  `docx_xml` facade while leaving generic XML range walkers in place.
- DOCX body paragraph mutation helpers moved from `src/docx_xml.rs` into
  `src/docx_xml/body_paragraphs.rs`, keeping append/insert/set/clear,
  paragraph replacement, section-property insertion, and paragraph rendering
  behind the same `docx_xml` facade while leaving shared text-child rendering
  and generic XML range walkers in place.
- DOCX paragraph id and `w14` namespace helpers moved from `src/docx_xml.rs`
  into `src/docx_xml/paragraph_ids.rs`, keeping para-id stamping, namespace
  insertion, existing-id scanning, and id minting behind the same `docx_xml`
  facade for paragraph and style mutation paths.
- DOCX XML text readback helpers moved from `src/docx_xml.rs` into
  `src/docx_xml/text_read.rs`, keeping fragment text extraction and
  namespace-aware Word attribute reads behind the same `docx_xml` facade for
  fields and header/footer readback paths.
- XLSX range export, JSON range rendering, data-out writing, data-format
  normalization, and range max-cell guards moved from `src/main.rs` into
  `src/xlsx_ranges.rs`.
- Shared XLSX workbook sheet resolution, cell/style decoding, used-range
  summaries, sparse/dense cell row rendering, cell/range parsing, and column
  naming moved from `src/main.rs` into `src/xlsx_model.rs`; XLSX sheet selector
  generation and relationship-target normalization are part of that model layer.
- XLSX A1 cell/range parsing, `RangeBounds`, range containment, and column-name
  rendering moved from `src/xlsx_model.rs` into `src/xlsx_model/range.rs`,
  leaving `src/xlsx_model.rs` as the facade for existing crate imports.
- XLSX sorted cell entries, used-range JSON/ref rendering, and sparse/dense row
  JSON rendering moved from `src/xlsx_model.rs` into
  `src/xlsx_model/render.rs`, preserving the sheet/cell readback facade and
  Go-oracle coverage for `xlsx cells extract` and `xlsx sheets show`.
- XLSX style readback, built-in number-format lookup, and date-style detection
  moved from `src/xlsx_model.rs` into `src/xlsx_model/styles.rs`, preserving
  the existing `XlsxStyle`, `xlsx_styles`, and `builtin_num_format_code` crate
  facade used by sheet readback and range-format writes.
- XLSX range/cell mutation commands, range formatting, calc-chain
  invalidation, style XML mutation, sheet-data rewrites, and mutation readback
  command generation moved from `src/main.rs` into `src/xlsx_mutation.rs`.
- Shared XLSX worksheet XML span parsing, row/cell span capture,
  `<sheetData>` rebuilding, used-range detection, merged-range intersection
  checks, and A1 range rendering moved from `src/xlsx_mutation.rs` into
  `src/xlsx_sheet_xml.rs`, keeping range writes and range-format writes on one
  shared worksheet substrate.
- XLSX range set-format command routing, number-format resolution, styles part
  scaffolding, and cell style XML updates moved from `src/xlsx_mutation.rs`
  into `src/xlsx_mutation/format.rs`, leaving shared worksheet write helpers in
  the mutation facade.
- XLSX range set-format number-format preset/custom-code resolution moved from
  `src/xlsx_mutation/format.rs` into
  `src/xlsx_mutation/format/number_format.rs`, keeping the set-format facade
  focused on package orchestration and style XML mutation.
- XLSX set-format styles relationship discovery and default `xl/styles.xml`
  scaffolding moved from `src/xlsx_mutation/format.rs` into
  `src/xlsx_mutation/format/styles_part.rs`, isolating package plumbing from
  style collection mutation.
- XLSX set-format style collection insertion, element-span discovery, and
  collection count repair moved from `src/xlsx_mutation/format.rs` into
  `src/xlsx_mutation/format/styles_xml.rs`, separating generic style XML
  structure editing from set-format orchestration.
- XLSX set-format `cellXfs` parsing, style entry rendering, and style-index
  reuse/creation moved from `src/xlsx_mutation/format.rs` into
  `src/xlsx_mutation/format/style_xfs.rs`, keeping cell style record mechanics
  separate from range application.
- XLSX set-format `numFmt` parsing, custom format-id reuse/allocation, and
  number-format collection insertion moved from `src/xlsx_mutation/format.rs`
  into `src/xlsx_mutation/format/num_formats_xml.rs`, leaving the set-format
  facade free of direct XML parser dependencies.
- XLSX single-cell mutation, handle resolution, previous-value reporting, and
  emitted readback command generation moved from `src/xlsx_mutation.rs` into
  `src/xlsx_mutation/cells.rs`, sharing the same range-write and recalculation
  package-update path used by range writes.
- XLSX range-set command orchestration, inline/file/stdin input parsing,
  JSON/CSV/TSV matrix normalization, null/ragged policy handling, and range
  bounds resolution moved from `src/xlsx_mutation.rs` into
  `src/xlsx_mutation/ranges.rs`; the parent module now holds the shared
  worksheet XML rewrite substrate used by range, cell, and format mutations.
- Serve `xlsx cells set` now delegates to the shared `xlsx_cells_set`
  mutation path, and the old direct cell-XML replacement/readback shim was
  removed.
- Proof after the latest de-monolithization slice: `cargo fmt --check`, `cargo
  check --all-targets`, targeted Go-oracle checks for `xlsx workbook metadata`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets`
  all pass with 4 ZIP guard unit tests plus 79 Rust contract tests. A generated
  metadata-update XLSX passed `ooxml --json validate --strict`, Microsoft Open
  XML SDK validation (`Valid: true`, `ErrorCount: 0`), and desktop Excel COM
  open proof (`1 passed, 0 failed`).
- The opaque Rust VBA package implementation split from `src/vba.rs` into
  `src/vba/` child modules for model/spec data, package inspection, package XML
  rewrites, mutation transactions, and JSON/readback rendering. This was a
  behavior-preserving split: `cargo fmt --check`, `cargo check --all-targets`,
  `cargo clippy --all-targets -- -D warnings`, focused VBA Go-oracle parity,
  capability subset tests, and `cargo test --all-targets` all passed after the
  move.
- Rust XLSX defined-name mutation parity landed for `xlsx names
  add/update/rename/delete`. The slice matches the Go oracle for saved mutation
  JSON, generated readback commands, dry-run output, validation/error envelopes,
  stale `--expect-ref` guards, empty `<definedNames>` cleanup, capability
  advertising, and post-save list/show readback. Proof: `cargo fmt --check`,
  `cargo check --all-targets`, focused `xlsx_names` Go-oracle tests,
  capability ratchet tests, `cargo clippy --all-targets -- -D warnings`, and
  `cargo test --all-targets` passed with 4 ZIP guard unit tests plus 81 Rust
  contract tests. A Rust-generated workbook after add/update/rename passed
  Rust `validate --strict`, Microsoft Open XML SDK validation (`Valid: true`,
  `ErrorCount: 0`), and desktop Excel COM open proof (`1 passed, 0 failed`,
  Excel 16.0 build 20026).
- The Rust XLSX defined-name implementation then split from the 1179-line
  `src/xlsx_names.rs` into a small facade plus private `src/xlsx_names/`
  modules for model data, package parsing/selection, JSON/readback output,
  mutation orchestration, and workbook XML rendering. This was an isomorphic
  code-move slice: the focused `xlsx_names` Go-oracle tests, capability subset
  tests, `cargo fmt --check`, `cargo check --all-targets`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets`
  all passed after the split with the same 4 ZIP guard unit tests plus 81 Rust
  contract tests.
- Rust XLSX freeze-pane parity landed for `xlsx freeze show/set/clear`.
  The slice matches the Go oracle for unfrozen/frozen readback JSON, saved
  mutation JSON, generated `validateCommand` and `showCommand` fields, dry-run
  output, invalid row/column guards, stale `--expect-state` guards, and clear
  failure on unfrozen sheets. The implementation lives in focused
  `src/xlsx_freeze.rs`, keeping sheet-view pane XML mutation out of the shared
  XLSX mutation and table modules. Proof: `cargo fmt --check`, `cargo
  check --all-targets`, focused `xlsx_freeze` Go-oracle tests, capability
  ratchet tests, `cargo clippy --all-targets -- -D warnings`, and `cargo test
  --all-targets` passed with 4 ZIP guard unit tests plus 83 Rust contract tests.
  A Rust-generated frozen workbook passed Rust `validate --strict`, Microsoft
  Open XML SDK validation (`Valid: true`, `ErrorCount: 0`), and desktop Excel
  COM open proof (`1 passed, 0 failed`).
- XLSX CLI dispatch split from the top-level `src/cli_dispatch.rs` into
  `src/cli_dispatch/xlsx.rs`, mirroring the existing DOCX family dispatcher and
  reducing the main collision point before the larger table-append slice. This
  was an isomorphic code-move slice: `cargo fmt --check`, `cargo
  check --all-targets`, focused `xlsx_` Go-oracle tests, focused serve XLSX
  inspect/op tests, `cargo clippy --all-targets -- -D warnings`, and `cargo
  test --all-targets` all passed with 4 ZIP guard unit tests plus 83 Rust
  contract tests.
- Rust XLSX table append-row parity landed for direct
  `xlsx tables append-rows`. The slice appends JSON/CSV/TSV matrices below an
  existing table, expands the table and autoFilter ranges, reuses the shared
  XLSX range writer for cell XML/formula handling, rejects totals/calculated
  columns/unsafe overwrites, and emits validation, range readback, and table
  readback commands matching the Go oracle. It is now advertised in Rust
  capabilities and supported through serve/MCP operation routing because the Go
  oracle advertises it as op-compatible. Rust capabilities now advertise 78
  Go-oracle command paths, leaving a pinned 212-command gap. Proof: `cargo fmt
  --check`, `cargo check --all-targets`, focused `xlsx_tables_append_rows`
  Go-oracle tests, focused `xlsx_tables` tests, focused serve-op and capability
  tests, MCP command-resource coverage, `cargo clippy --all-targets -- -D
  warnings`, and `cargo test --all-targets` passed with 4 ZIP guard unit tests
  plus 89 Rust contract tests. A Rust-generated appended workbook at
  `.tmp\xlsx-tables-append-rows-promotion\rust-append-rows.xlsx` passed Rust
  `validate --strict`, Microsoft Open XML SDK validation (`Valid: true`,
  `ErrorCount: 0`, schema `Office2019`), and desktop Excel COM open proof
  (`1 passed, 0 failed`).
- Rust XLSX table append-record parity landed for
  `xlsx tables append-records`. The slice decodes inline/file JSON records,
  maps fields to exact table column names, enforces `--expect-range`, missing
  and extra-field policies, reuses the shared table append matrix core, and
  supports serve/MCP op routing because the Go oracle advertises the command as
  op-compatible. The record decoder lives in `src/xlsx_table_append/records.rs`
  so the table append parent remains focused on target resolution and OOXML
  mutation. Rust capabilities now advertise 77 Go-oracle command paths, leaving
  a pinned 213-command gap. Proof: `cargo fmt --check`, `cargo check
  --all-targets`, focused `xlsx_tables_append_records` Go-oracle and serve-op
  tests, focused `xlsx_tables` tests, MCP command-resource coverage, capability
  ratchet tests, `cargo clippy --all-targets -- -D warnings`, and `cargo test
  --all-targets` passed with 4 ZIP guard unit tests plus 88 Rust contract tests.
  A Rust-generated appended workbook at
  `.tmp\xlsx-tables-append-records\rust-append-records.xlsx` passed Rust
  `validate --strict`, Microsoft Open XML SDK validation (`Valid: true`,
  `ErrorCount: 0`, schema `Office2019`), and desktop Excel COM open proof
  (`1 passed, 0 failed`).
- XLSX table CLI dispatch and XLSX table capability metadata split into focused
  child modules at `src/cli_dispatch/xlsx/tables.rs` and
  `src/capabilities/commands/xlsx/tables.rs`. This was an isomorphic
  de-monolithization slice only: command routing, unsupported-command errors,
  capability order, and MCP command resources are unchanged. Proof: `cargo
  fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D
  warnings`, focused `xlsx_tables` Go-oracle/serve tests, focused capability
  subset/MCP tests, and `cargo test --all-targets` passed with 4 ZIP guard unit
  tests plus 88 Rust contract tests. Office/Open XML proof was not rerun because
  no OOXML output behavior changed.
- XLSX table append XML validation/ref-rewrite helpers split from
  `src/xlsx_table_append.rs` into `src/xlsx_table_append/table_xml.rs`. This is
  a private isomorphic seam ahead of the next table mutation slice:
  append-row/append-record command behavior, JSON shape, validation, and
  readback commands are unchanged. Proof: `cargo fmt --check`, `cargo
  check --all-targets`, and focused `cargo test --test rust_contract_smoke
  xlsx_tables -- --nocapture` passed with 9 XLSX table contract tests.
- Rust direct CLI parity landed for `xlsx tables set-column-format`. The
  implementation resolves an exact table column to its data-body range, excludes
  header/totals rows, then reuses the existing XLSX range set-format path for
  styles, validation, output writing, and range readback. It is not advertised
  in Rust capabilities or serve/MCP because the Go capability inventory also
  omits this Cobra command. Proof in the worker lane: `cargo fmt --check`,
  `cargo check --all-targets`, and focused `cargo test --test
  rust_contract_smoke xlsx_tables_set_column_format -- --nocapture` passed with
  2 Go-oracle contract tests. Integration proof repeated `cargo fmt --check`,
  `cargo check --all-targets`, and the same focused contract tests. A
  Rust-generated formatted workbook at
  `.tmp\xlsx-table-column-format-proof\rust-table-format.xlsx` passed Rust
  `validate --strict`, Microsoft Open XML SDK validation (`Valid: true`,
  `ErrorCount: 0`, schema `Office2019`), and desktop Excel COM open proof
  (`1 passed, 0 failed`).
- Windows edit smoke against `target/debug/ooxml.exe` reached the implemented
  edit surface: 12 scenarios passed strict validation, Microsoft Open XML SDK
  schema validation, and desktop Office COM open proof. The three implemented
  XLSX mutation scenarios (`xlsx-cells-set`, `xlsx-ranges-set`, and
  `xlsx-ranges-set-format`) opened in Excel without repair/failure. The full
  52-scenario smoke remains red for the Rust port because 40 Go-surface edit
  commands are still intentionally unsupported.

The first Rust slice implements and tests the CLI cases from that baseline:

- `--json version`
- `--json capabilities` advertises the Rust-supported `ooxml version` command
  and checks the advertised path against the Go oracle capability inventory
- `--json inspect <pptx|xlsx|docx>` with Go-oracle comparison for PPTX deck
  structure, XLSX workbook summaries including shared strings/styles/charts, and
  DOCX document summaries including body counts, headers/footers, comments, and
  media assets; generated parity cases also cover relocated standard and macro
  XLSX/DOCX main parts, malformed main XML failure paths, and unsupported OOXML
  package detection
- `--json pptx slides show ... --include-text`
- `--json xlsx ranges export ...` with Go-oracle comparison for default JSON
  export, typed export, formula/format matrices, and `--max-cells` guardrails
- `--json xlsx ranges set ...` with Go-oracle comparison for inline JSON
  matrices, CSV/TSV matrix input, `--values-file -` stdin, saved output
  readback, formula cells, null skipping, dry-run templates, generated mutation
  readback commands, formula-overwrite rejection, merged-cell rejection, and
  preservation of untouched shared-string/style/formula-cache cell XML, formula
  recalculation metadata, calc-chain invalidation on formula overwrite/clear,
  and serve/MCP operation routing through the session `op` path
- `--json xlsx ranges set-format ...` with Go-oracle comparison for direct CLI
  number-format mutation, custom styles part creation, formatted blank-cell
  creation, saved output format readback, generated mutation readback commands,
  dry-run non-mutation, and serve/MCP operation routing through the session
  `op` path
- `--json xlsx workbook metadata inspect/update ...` with Go-oracle comparison
  for core/app properties, calc settings defaults, canonical updated-field
  ordering, stringly previous values, dry-run omission of output/readback
  commands, empty-value clearing, explicit `--full-calc-on-load=false` clearing,
  strict validation, generated inspect/validate command replay with quoted
  paths, guard failures, invalid calc modes, and serve operation/inspect routing
  through the session path
- `--json docx text <docx>` with Go-oracle comparison across the committed
  positive DOCX fixture corpus: paragraphs, styles, preserved whitespace,
  hyperlinks, field/instruction text omission, tables, merged tables, headers,
  comments/media/image fixtures, default namespace handling, and unique/duplicate
  `w14:paraId` marker handling
- `--json docx blocks <docx>` with Go-oracle comparison for stable body block
  reports, block filtering, paragraph/table selectors, content hashes, optional
  paragraph run metadata, table cell text, unique/duplicate `w14:paraId` handle
  behavior, namespace-sensitive metadata handling, missing-block errors,
  negative block rejection, malformed main document rejection, and unsupported
  package-type rejection
- `--json docx blocks replace <docx>` and
  `--json docx blocks delete <docx>` with Go-oracle comparison for hash-guarded
  body block replacement/deletion, paragraph style preservation, table deletion,
  destination/readback symmetry for replacement, strict validation, dry-run
  non-mutation, missing/invalid hash errors, hash mismatch errors, block-range
  validation, text/text-file conflict rejection, delete-last rejection,
  unsupported package-type rejection, and serve operation/readback routing
- `--json docx blocks insert-after <docx>` with Go-oracle comparison for
  hash-guarded paragraph insertion before the first block and after table
  blocks, optional paragraph style, strict validation, `docx blocks` readback,
  dry-run non-mutation, hash-shape/hash-mismatch errors, block-range
  validation, text/text-file conflict rejection, unsupported package-type
  rejection, and serve operation/readback routing
- `--json docx paragraphs append <docx>` with Go-oracle comparison for direct
  CLI paragraph append mutation, optional paragraph style, strict validation,
  DOCX text readback, dry-run non-mutation, output-flag validation, text/text-file
  conflict rejection, unsupported package-type rejection, and serve
  operation/readback routing
- `--json docx paragraphs insert <docx>` with Go-oracle comparison for direct
  CLI paragraph insertion at the document start and after table blocks, text-file
  input, strict validation, DOCX text readback, dry-run non-mutation, bad index
  and missing-target errors, output-flag validation, text/text-file conflict
  rejection, unsupported package-type rejection, and serve operation/readback
  routing
- `--json docx paragraphs set <docx>` and
  `--json docx paragraphs clear <docx>` with Go-oracle comparison for direct CLI
  paragraph replacement/clearing, style preservation, first-run property
  preservation through readback, strict validation, text-file input, dry-run
  non-mutation, stable paragraph handle injection/resolution, structural handle
  survival after insertion, stale/ambiguous/wrong-format handle errors,
  table-target rejection, missing-target errors, output-flag validation,
  required/non-empty replacement text validation, and unsupported package-type
  rejection, plus serve operation/readback routing
- `--json docx styles list <docx>` and `--json docx styles show <docx>` with
  Go-oracle comparison for style catalog enumeration, style-type filtering,
  nullable missing-styles-part behavior, style handles, found/not-found style
  show results, and invalid argument errors
- `--json docx styles apply <docx>` with Go-oracle comparison for paragraph,
  run, and table style mutation, style-handle resolution, hash guards, dry-run
  non-mutation, strict validation, paragraph handle stamping, style type
  mismatch rejection, missing-style candidate errors, output-flag validation,
  table-target rejection, unsupported package-type rejection, and serve
  inspect/operation/readback routing
- `--json docx comments list <docx>` with Go-oracle comparison for comment
  enumeration, `--comment-id` filtering, missing-id errors, empty documents
  without a comments part, semantic content hashes, body-block anchors, stable
  selectors, comment handles, and serve inspect routing through the session path
- `--json docx comments add <docx>` with Go-oracle comparison for direct CLI
  comment insertion, comments part/content-type/relationship creation, body
  range marker insertion, deterministic `--date` readback, strict validation,
  `comments list` readback, dry-run non-mutation, required author validation,
  unsupported package-type rejection, and serve operation/readback routing
- `--json docx comments edit <docx>` with Go-oracle comparison for direct CLI
  comment text/date/author mutation, `--expect-hash` guard failures, stable
  comment-handle targeting and stale-handle rejection, strict validation,
  `comments list` readback, dry-run non-mutation, unsupported package-type
  rejection, and serve operation/readback routing
- `--json docx comments remove <docx>` with Go-oracle comparison for direct CLI
  comment deletion, body range/reference marker cleanup, `--expect-hash` guard
  failures, stable comment-handle targeting and stale-handle rejection, no-comment
  target errors, strict validation, `comments list` readback, dry-run
  non-mutation, unsupported package-type rejection, and serve operation/readback
  routing
- `--json docx fields list <docx>` with Go-oracle comparison for simple and
  complex fields, body plus header field ordering, cached result readback,
  leading-instruction `--type` filtering, empty documents, unsupported package
  rejection, document-order mixed fields, switch-bearing field instructions, and
  table-nested fields reported as non-editable
- `--json docx fields insert <docx>` and
  `--json docx fields set-result <docx>` with Go-oracle comparison for body
  simple-field insertion, unknown-code warnings, simple field cached-result
  updates, complex header field cached-result updates, hash mismatch guards,
  selector validation, table-target rejection, strict validation, readback
  commands, and serve `inspect`/`op` routing through the session path
- `--json docx headers list <docx>` and `--json docx footers list <docx>` with
  Go-oracle comparison for section-scoped header/footer references, default
  header/footer refs, pasteable selectors, relationship-id aliases, part aliases,
  content types, empty section properties, and unsupported package-type
  rejection
- `--json docx headers show <docx>` and `--json docx footers show <docx>` with
  Go-oracle comparison for `--type`, `--id`, and `--selector` targeting,
  relationship and part selector aliases, scoped paragraph selectors, paragraph
  text/style readback, paragraph-suffix selectors, and unsupported package-type
  rejection
- `--json docx headers set-text <docx>` and
  `--json docx footers set-text <docx>` with Go-oracle comparison for selector
  and index targeting, paragraph-suffix selectors, previous-text readback,
  first-run property preservation, strict validation, dry-run templates,
  generated validate/show/list commands, new header/footer part creation,
  unreferenced part reuse with section-reference wiring, and serve operation
  routing through the session `op` path
- `--json docx images list <docx>` with Go-oracle comparison for inline image
  enumeration, media relationship resolution, content type, EMU dimensions,
  block indexes, block hashes, selectors, empty documents, media-only fixtures
  without inline image references, and unsupported package-type rejection
- `--json docx images replace <docx>` and
  `--json docx images insert <docx>` with Go-oracle comparison for saved DOCX
  mutation, image payload writes, inline extent updates, relationship and media
  part allocation, strict validation/readback, dry-run non-mutation,
  `--expect-hash` guard failures, missing image/block errors, and direct CLI
  capability advertisement
- `--json docx tables show <docx>` with Go-oracle comparison for whole-document
  and selected-table readback, body block indexes, selectors, content hashes,
  dimensions, merged-cell detection, cell text, detailed table objects, empty
  no-table documents, bad selectors, missing main-document parts, and
  unsupported package-type rejection
- `--json docx tables set-cell <docx>`,
  `--json docx tables clear-cell <docx>` with Go-oracle comparison for
  hash-guarded cell mutation JSON, output/readback command fields, strict
  validation, selected-table readback, previous cell text, dry-run shape, and
  serve operation/inspect routing through the session path
- `--json docx tables insert-row <docx>` and
  `--json docx tables delete-row <docx>` with Go-oracle comparison for
  hash-guarded row mutation JSON, output/readback command fields, strict
  validation, selected-table readback, dry-run shape, row-target errors,
  stale-hash guards, and merged-table rejection; delete-row also covers
  last-row rejection
- `--json docx text <xlsx>` unsupported-type rejection with direct Go-oracle
  comparison for exit code, stderr JSON, and empty stdout
- JSON error envelope for an invalid slide number
- `--json pptx replace text ... --out <pptx>`
- `--json --strict validate <pptx>`
- `--json --strict validate <docx|xlsx>` negative-package diagnostics for
  dangling relationships plus missing DOCX main-document and XLSX worksheet
  parts, with exit-code and stdout JSON parity against the Go oracle
- `pptx render ... --format json` manifest shape, with real-tool execution when
  LibreOffice and Poppler are available and a deterministic test hook for the
  frozen contract
- `--format json verify <pptx> --baseline <pptx>` validation plus semantic text
  diff envelope for the frozen PPTX fixture
- `serve` JSON-RPC open, op, inspect, validate, plan, commit, and abort flow for
  the frozen XLSX cell-edit session, with validate returning real diagnostics
  arrays instead of placeholder nulls
- `serve open`/`commit` handling for advertised `inPlace`, `backup`, and
  `noValidate` options, including commit validation-by-default and no-write
  behavior on validation failure
- `mcp` stdio JSON-RPC initialize, tools/resources discovery, command resource
  readback, and tools/call open, op, inspect, validate, plan, commit, and abort
  flow for the frozen XLSX cell-edit session
- `mcp` `resource://command/{path}` dynamic readback for every command
  advertised by the Rust capability inventory, accepting both full
  `ooxml ...` paths and op-vocabulary shorthand paths
- `mcp` `resource://capabilities` mirrors the Rust CLI capability inventory and
  contract metadata, with the MCP command-resource template included
- `--json capabilities` advertises Rust control surfaces `ooxml capabilities`,
  `ooxml serve`, and `ooxml mcp`, keeping the machine-readable inventory aligned
  with the self-description, JSON-RPC session, and MCP entry points
- `--json capabilities --for <filter>` for the Rust-supported partial command
  surface, including the web-agent-relevant PPTX and XLSX commands
- Rust capability inventory is checked as a strict subset of the Go oracle
  capability inventory, so Rust cannot advertise non-oracle command paths while
  the partial surface grows
- Capability surface ratchet: the current Go oracle advertises 290 command
  paths, Rust advertises 166, and the harness pins the 124-command gap until
  each new Rust command intentionally moves the count
- `--json xlsx sheets list <xlsx>` with direct Go-oracle comparison for the
  minimal workbook fixture
- `--json pptx slides list <pptx>` with direct Go-oracle comparison for
  minimal, notes, table, and dangling-layout PPTX fixtures
- `--json pptx slides selectors <pptx> --slide <n>` for the generated minimal
  slide selector readback path
- `--json pptx shapes show <pptx> --slide <n> --include-text --include-bounds`
  for generated shape readback commands, with Go-oracle comparison on text-shape
  and table/graphicFrame fixtures
- `--json pptx masters list <pptx>` and
  `--json pptx masters show <pptx> --master <n>` with Go-oracle comparison for
  master ordering, stable selectors, linked layouts, theme/default text style
  readback, placeholder summaries, missing masters, unsupported-package cases,
  and serve `inspect` routing through the session path
- `--json pptx layouts list <pptx>` and
  `--json pptx layouts show <pptx> --layout <selector>` with Go-oracle
  comparison for layout ordering, master filtering, number/name selectors,
  placeholder summaries, theme/default text style readback, not-found selectors,
  unsupported-package cases, and serve `inspect` routing through the session
  path
- `--json pptx tables show <pptx> --slide <n>` with Go-oracle comparison for
  table fixture readback, target selectors, `@all-tables`, details mode, empty
  slide results, out-of-range slides, target misses, missing table IDs, and
  unsupported-package cases, plus serve `inspect` routing through the session
  path
- `--json pptx comments list <pptx>` with Go-oracle comparison for deck-wide,
  slide-filtered, comment-id-filtered, generated commented decks, missing
  comments, slide range guards, unsupported-package cases, and serve `inspect`
  routing through the session path
- `--json pptx comments add/edit/remove <pptx>` with Go-oracle comparison for
  saved mutation JSON, generated comments-list readback commands, stable
  comment-handle edit/remove targeting, dry-run output/templates, hash mismatch
  guards, missing-comment and slide-range errors, capability indexing, strict
  validation through emitted commands, and final-comment comments-part cleanup
- `--json pptx slides delete/move/reorder <pptx>` with Go-oracle comparison
  for saved mutation JSON, generated slides-list/readback/validate commands,
  dry-run templates, strict validation of saved PPTX outputs, slides-list
  readback after mutation, notes-slide cleanup on delete, and representative
  range/permutation errors
- `--json pptx extract text <pptx>` with Go-oracle comparison for full-deck,
  slide-filtered, empty-selection, and unsupported-package cases, plus serve
  `inspect` routing through the session path
- `--json pptx extract notes <pptx>` and
  `--json pptx notes show <pptx>` with Go-oracle comparison for full-deck,
  slide-filtered, empty-note, notes-body, out-of-range, and
  unsupported-package cases, plus serve `inspect` routing through the session
  path
- `--json pptx extract images <pptx>` with Go-oracle comparison for image file
  export manifests, duplicate output filename behavior, no-image `null`
  manifests, output artifact byte checks, layout-image flag acceptance, and
  representative invalid-slide errors
- `--json pptx extract xml <pptx>` with Go-oracle comparison for slide, layout,
  and master selectors, raw XML/summary output artifact byte checks, required
  `--out` handling, and representative selector range errors
- `--json xlsx cells extract <xlsx>` with Go-oracle comparison for default
  sparse extraction, dense `--include-empty` ranges, formulas, booleans, inline
  strings, and date-style cell metadata
- `--json xlsx sheets show <xlsx>` with Go-oracle comparison for worksheet
  metadata, used ranges, stable selectors, and generated readback command
  templates
- `--json xlsx sheets add/rename/move/delete <xlsx>` with Go-oracle comparison
  for saved mutation JSON, dry-run output/templates, required-flag and
  representative error parity, validation/list/show readback command execution,
  sheet-name validation, move-target and last-sheet delete guards, capability
  indexing, and strict validation of saved outputs. The add harness normalizes
  the Go oracle's variable new sheetId while asserting the same destination
  shape and package invariants.
- `--json xlsx names list/show <xlsx>` with Go-oracle comparison for
  workbook-scoped and sheet-local defined names, scope filtering, selectors,
  workbook handles, generated `showCommand` execution, `capabilities --for
  name`, and serve inspect routing
- `--json xlsx names add/update/rename/delete <xlsx>` with Go-oracle
  comparison for saved mutation JSON, dry-run output, validation and readback
  commands, stale `--expect-ref` guards, invalid-name/error parity, sheet/range
  ref construction, empty defined-name cleanup, capability indexing, and strict
  validation of saved outputs
- `--json xlsx freeze show/set/clear <xlsx>` with Go-oracle comparison for
  unfrozen/frozen readback, saved mutation JSON, generated validation/readback
  commands, dry-run behavior, invalid row/column bounds, stale
  `--expect-state` guards, unfrozen clear errors, capability indexing, strict
  validation, Open XML SDK schema validation, and desktop Excel COM open proof
- `--json xlsx colwidths show <xlsx>` with Go-oracle comparison for default
  widths, explicit/custom/hidden `<col>` spans, default column-width overrides,
  reversed range normalization, generated set-command templates, and capability
  indexing
- `--json xlsx rowheights show <xlsx>` with Go-oracle comparison for default
  heights, explicit/custom/hidden `<row>` entries, default row-height
  overrides, reversed range normalization, invalid row-range errors, generated
  set-command templates, and capability indexing
- `--json xlsx colwidths set <xlsx>` and
  `--json xlsx rowheights set <xlsx>` with Go-oracle comparison for saved
  mutation JSON, generated validation/readback commands, dry-run no-write
  behavior, out-of-range and stale-expect errors, saved output readback,
  capability indexing, serve operation routing, strict validation, and Go
  readback of committed outputs
- `--json xlsx filters-sorts show <xlsx>`,
  `--json xlsx filters-sorts set-autofilter <xlsx>`,
  `--json xlsx filters-sorts clear-autofilter <xlsx>`, and
  `--json xlsx filters-sorts add-column-filter <xlsx>`,
  `--json xlsx filters-sorts clear-column-filter <xlsx>`,
  `--json xlsx filters-sorts set-sort <xlsx>`, and
  `--json xlsx filters-sorts clear-sort <xlsx>` with Go-oracle comparison for
  worksheet/table autoFilter readback, worksheet sortState readback, saved
  mutation JSON, generated validation/show readback commands, dry-run behavior,
  invalid range errors, table-target default range behavior, column value/custom
  filters, clearing column filters, sort condition refs, stale range/filter/sort
  guards, capability indexing, and serve inspect routing for show
- `--json xlsx tables list <xlsx>` and `--json xlsx tables show <xlsx>` with
  Go-oracle comparison for generated table workbooks, table metadata, columns,
  bridge command templates, `capabilities --for table`, and stable table
  selectors (`tableId`, `id`, `table`, `#`, part, relationship, display/name,
  and bare names)
- `--json xlsx tables export <xlsx>` with Go-oracle comparison for default JSON
  export, typed export, formula matrices, `--data-out`, `--max-cells`, missing
  selectors, paths/sheet names with spaces, `capabilities --for table`, and
  serve inspect routing
- `--json xlsx tables append-rows <xlsx>` with Go-oracle comparison for saved
  mutation JSON, dry-run output, generated validation/range/table readback
  commands, table and autoFilter range expansion, appended range readback,
  invalid column-count errors, strict validation, Open XML SDK schema
  validation, desktop Excel COM open proof, capability indexing, serve operation
  routing, and MCP command-resource discovery
- `--json xlsx tables append-records <xlsx>` with Go-oracle comparison for
  saved mutation JSON, dry-run output/templates, generated validation/range/table
  readback commands, table and autoFilter range expansion, appended range
  readback, required `--expect-range`, missing/extra-field policies,
  blank/duplicate table column rejection, capability indexing, serve operation
  routing, MCP command-resource discovery, strict validation, Open XML SDK
  schema validation, and desktop Excel COM open proof
- `--json xlsx workbook metadata inspect/update <xlsx>` with Go-oracle
  comparison for default inspection, saved mutation output, generated readback
  commands, dry-run, clearing, calc-mode/full-recalc flags, guard/error
  envelopes, `capabilities --for xlsx`, and strict Go-subset inventory ratchet
- `--json vba inspect/extract-bin/attach/remove <xlsx|xlsm>` for the opaque
  package-level VBA path, with Go-oracle comparison for macro package wiring,
  byte-for-byte `vbaProject.bin` extraction, saved output readback, dry-run
  templates, and strict validation of attached/removed packages.
- `--json vba inspect-bin <vbaProject.bin> --family xlsx|pptx`,
  `--json vba list <xlsm|pptm>`, and
  `--json vba extract <xlsm|pptm> --out-dir <dir>` with Go-oracle comparison
  for parseable source-only VBA projects, CFB/MS-OVBA decompression, module
  selectors and hashes, host-family compatibility warnings, missing-macro
  errors, and extracted `.bas` source readback.
- `--json vba create <output.xlsm|output.pptm>` with Go-oracle comparison for
  source normalization, argument errors, fake-helper JSON completion, emitted
  follow-up commands, and PowerShell helper invocation shape without launching
  Office COM in Rust tests.
- `--json vba office-check <xlsm|pptm>` with Go-oracle comparison for the
  deterministic macro-free skipped report and Rust implementation of the local
  LibreOffice/soffice open-check path when an engine is installed. Rust still
  does not promote source-changing `add-module`/`replace-module`/
  `remove-module`.
- `serve` JSON-RPC generic PPTX inspect/op/commit path for
  `pptx slides show` plus `pptx replace text`, matching the Flue workbench's
  generic `apply_ooxml_ops_to_current` smoke route
- `serve` JSON-RPC inspect support for `xlsx cells extract`, so generated XLSX
  readback commands can run through the web/agent session loop
- `serve` JSON-RPC inspect support for `xlsx sheets show`, so `showCommand`
  values generated by `xlsx sheets list` can run through the web/agent session
  loop
- `serve` JSON-RPC inspect support for `xlsx tables list/show`, so generated
  table readback commands can run through the web/agent session loop
- `serve` JSON-RPC op/inspect support for `xlsx workbook metadata
  update/inspect`, so workbook-level metadata edits can run through the same
  web/agent session loop as range and table workflows
- `serve` JSON-RPC inspect support for DOCX text, header/footer, image,
  comment, block, field, style, and table readback commands, so DOCX discovery
  and generated readback commands can run through the same web/agent session
  loop as direct CLI calls
- `serve` JSON-RPC op support for DOCX header/footer, field, paragraph, style,
  block, comment, and table mutations, proving the op-compatible DOCX mutation
  paths used by the web/agent session loop
- Focused Rust-produced Office proof on 2026-06-20 generated representative
  DOCX, XLSX, and PPTX outputs with `target\debug\ooxml.exe`; all three passed
  Rust `validate --strict`, the .NET Open XML SDK validator
  (`DocumentFormat.OpenXml` 3.5.1, Office2019 schema, zero errors), and desktop
  Office COM open checks: Word 16.0 build 16.0.20026, Excel 16.0 build 20026,
  and PowerPoint 16.0 build 20026 opened the files without failure.

Still missing before parity can be claimed:

- real render proof parity beyond the mocked frozen manifest path.
- Full command-surface inventory parity.
- Metamorphic and fuzz harnesses for OOXML package invariants.
- Broad release-grade Office/Open XML SDK/COM proof for the complete promoted
  Rust surface, including Office-authored `vba create`, real macro package
  `vba office-check`, and module source mutation gates.

Dependency note: live GitHub inspection of `https://github.com/Dicklesworthstone`
found useful Rust infrastructure projects, but no direct OOXML/ZIP/XML package
library. The initial Rust subject therefore uses mainstream Rust crates for ZIP,
XML, and JSON handling while keeping Dicklesworthstone projects as the preferred
source for future MCP, async/runtime, TUI, and agent ergonomics patterns.
