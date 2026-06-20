# Rust Port Status

The Go implementation remains the reference on `codex/ooxml-go-reference`.
Rust work lands on `codex/ooxml-rust-port`. The Rust smoke harness builds its
Go oracle from a detached `codex/ooxml-go-reference` worktree by default, or
from `OOXML_GO_ORACLE_DIR`/`OOXML_GO_ORACLE_REF` when deliberately overridden.

The frozen Go contract lives in `testdata/golden/rust-port-contract/baseline.json`.

Latest milestone, 2026-06-20:

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
- The top-level serve op dispatcher now routes XLSX and DOCX commands by
  family prefix, leaving exact command matching and unsupported-command
  fallbacks inside each child dispatcher.
- DOCX CLI dispatch for text, block, style, comment, field, header/footer,
  image, table, and paragraph commands moved from `src/cli_dispatch.rs` into
  `src/cli_dispatch/docx.rs`, leaving the top-level CLI dispatcher responsible
  for core command routing plus PPTX/XLSX families.
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
- DOCX table show/set-cell/clear-cell commands, table summary rendering,
  table-cell XML rewrites, and table readback command generation moved from
  `src/main.rs` into `src/docx_tables.rs`.
- Shared DOCX Word XML constants, paragraph/text fragment readers, body block
  range walkers, paragraph rendering/insertion/replacement, table scaffolding,
  `w14:paraId` stamping, and namespace helpers moved from `src/main.rs` into
  `src/docx_xml.rs`, while existing command modules keep importing through the
  crate facade.
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
  all pass with 4 ZIP guard unit tests plus 78 Rust contract tests. A generated
  metadata-update XLSX passed `ooxml --json validate --strict`, Microsoft Open
  XML SDK validation (`Valid: true`, `ErrorCount: 0`), and desktop Excel COM
  open proof (`1 passed, 0 failed`).
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
- `--json docx tables show <docx>` with Go-oracle comparison for whole-document
  and selected-table readback, body block indexes, selectors, content hashes,
  dimensions, merged-cell detection, cell text, detailed table objects, empty
  no-table documents, bad selectors, missing main-document parts, and
  unsupported package-type rejection
- `--json docx tables set-cell <docx>` and
  `--json docx tables clear-cell <docx>` with Go-oracle comparison for
  hash-guarded cell mutation JSON, output/readback command fields, strict
  validation, selected-table readback, previous cell text, dry-run shape, and
  serve operation/inspect routing through the session path
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
  paths, Rust advertises 65, and the harness pins the 225-command gap until
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
- `--json pptx extract text <pptx>` with Go-oracle comparison for full-deck,
  slide-filtered, empty-selection, and unsupported-package cases, plus serve
  `inspect` routing through the session path
- `--json pptx extract notes <pptx>` and
  `--json pptx notes show <pptx>` with Go-oracle comparison for full-deck,
  slide-filtered, empty-note, notes-body, out-of-range, and
  unsupported-package cases, plus serve `inspect` routing through the session
  path
- `--json xlsx cells extract <xlsx>` with Go-oracle comparison for default
  sparse extraction, dense `--include-empty` ranges, formulas, booleans, inline
  strings, and date-style cell metadata
- `--json xlsx sheets show <xlsx>` with Go-oracle comparison for worksheet
  metadata, used ranges, stable selectors, and generated readback command
  templates
- `--json xlsx names list/show <xlsx>` with Go-oracle comparison for
  workbook-scoped and sheet-local defined names, scope filtering, selectors,
  workbook handles, generated `showCommand` execution, `capabilities --for
  name`, and serve inspect routing
- `--json xlsx tables list <xlsx>` and `--json xlsx tables show <xlsx>` with
  Go-oracle comparison for generated table workbooks, table metadata, columns,
  bridge command templates, `capabilities --for table`, and stable table
  selectors (`tableId`, `id`, `table`, `#`, part, relationship, display/name,
  and bare names)
- `--json xlsx tables export <xlsx>` with Go-oracle comparison for default JSON
  export, typed export, formula matrices, `--data-out`, `--max-cells`, missing
  selectors, paths/sheet names with spaces, `capabilities --for table`, and
  serve inspect routing
- `--json xlsx workbook metadata inspect/update <xlsx>` with Go-oracle
  comparison for default inspection, saved mutation output, generated readback
  commands, dry-run, clearing, calc-mode/full-recalc flags, guard/error
  envelopes, `capabilities --for xlsx`, and strict Go-subset inventory ratchet
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

Still missing before parity can be claimed:

- real render proof parity beyond the mocked frozen manifest path.
- Full command-surface inventory parity.
- Metamorphic and fuzz harnesses for OOXML package invariants.
- Office/Open XML SDK/COM proof gates.

Dependency note: live GitHub inspection of `https://github.com/Dicklesworthstone`
found useful Rust infrastructure projects, but no direct OOXML/ZIP/XML package
library. The initial Rust subject therefore uses mainstream Rust crates for ZIP,
XML, and JSON handling while keeping Dicklesworthstone projects as the preferred
source for future MCP, async/runtime, TUI, and agent ergonomics patterns.
