# Rust Port Status

The Go implementation remains the reference on `codex/ooxml-go-reference`.
Rust work lands on `codex/ooxml-rust-port`. The Rust smoke harness builds its
Go oracle from a detached `codex/ooxml-go-reference` worktree by default, or
from `OOXML_GO_ORACLE_DIR`/`OOXML_GO_ORACLE_REF` when deliberately overridden.

The frozen Go contract lives in `testdata/golden/rust-port-contract/baseline.json`.
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
  conflict rejection, and unsupported package-type rejection
- `--json docx paragraphs insert <docx>` with Go-oracle comparison for direct
  CLI paragraph insertion at the document start and after table blocks, text-file
  input, strict validation, DOCX text readback, dry-run non-mutation, bad index
  and missing-target errors, output-flag validation, text/text-file conflict
  rejection, and unsupported package-type rejection
- `--json docx paragraphs set <docx>` and
  `--json docx paragraphs clear <docx>` with Go-oracle comparison for direct CLI
  paragraph replacement/clearing, style preservation, first-run property
  preservation through readback, strict validation, text-file input, dry-run
  non-mutation, stable paragraph handle injection/resolution, structural handle
  survival after insertion, stale/ambiguous/wrong-format handle errors,
  table-target rejection, missing-target errors, output-flag validation,
  required/non-empty replacement text validation, and unsupported package-type
  rejection
- `--json docx styles list <docx>` and `--json docx styles show <docx>` with
  Go-oracle comparison for style catalog enumeration, style-type filtering,
  nullable missing-styles-part behavior, style handles, found/not-found style
  show results, and invalid argument errors
- `--json docx styles apply <docx>` with Go-oracle comparison for paragraph,
  run, and table style mutation, style-handle resolution, hash guards, dry-run
  non-mutation, strict validation, paragraph handle stamping, style type
  mismatch rejection, missing-style candidate errors, output-flag validation,
  table-target rejection, and unsupported package-type rejection
- `--json docx comments list <docx>` with Go-oracle comparison for comment
  enumeration, `--comment-id` filtering, missing-id errors, empty documents
  without a comments part, semantic content hashes, body-block anchors, stable
  selectors, and comment handles
- `--json docx comments add <docx>` with Go-oracle comparison for direct CLI
  comment insertion, comments part/content-type/relationship creation, body
  range marker insertion, deterministic `--date` readback, strict validation,
  `comments list` readback, dry-run non-mutation, required author validation,
  and unsupported package-type rejection
- `--json docx comments edit <docx>` with Go-oracle comparison for direct CLI
  comment text/date/author mutation, `--expect-hash` guard failures, stable
  comment-handle targeting and stale-handle rejection, strict validation,
  `comments list` readback, dry-run non-mutation, and unsupported package-type
  rejection
- `--json docx comments remove <docx>` with Go-oracle comparison for direct CLI
  comment deletion, body range/reference marker cleanup, `--expect-hash` guard
  failures, stable comment-handle targeting and stale-handle rejection, no-comment
  target errors, strict validation, `comments list` readback, dry-run
  non-mutation, and unsupported package-type rejection
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
- `--json capabilities --for <filter>` for the Rust-supported partial command
  surface, including the web-agent-relevant PPTX and XLSX commands
- Rust capability inventory is checked as a strict subset of the Go oracle
  capability inventory, so Rust cannot advertise non-oracle command paths while
  the partial surface grows
- Capability surface ratchet: the current Go oracle advertises 290 command
  paths, Rust advertises 51, and the harness pins the 239-command gap until
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
- `--json xlsx cells extract <xlsx>` with Go-oracle comparison for default
  sparse extraction, dense `--include-empty` ranges, formulas, booleans, inline
  strings, and date-style cell metadata
- `--json xlsx sheets show <xlsx>` with Go-oracle comparison for worksheet
  metadata, used ranges, stable selectors, and generated readback command
  templates
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
- `serve` JSON-RPC op support for `docx headers set-text`, proving the
  op-compatible DOCX header/footer mutation path used by the web/agent session
  loop

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
