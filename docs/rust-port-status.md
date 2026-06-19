# Rust Port Status

The Go implementation remains the reference on `codex/ooxml-go-reference`.
Rust work lands on `codex/ooxml-rust-port`.

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
  preservation of untouched shared-string/style/formula-cache cell XML; this is
  direct CLI subset support, with full formula recalculation/calc-chain parity
  and serve `op` routing still open in `DISC-001`
- `--json xlsx ranges set-format ...` with Go-oracle comparison for direct CLI
  number-format mutation, custom styles part creation, formatted blank-cell
  creation, saved output format readback, generated mutation readback commands,
  and dry-run non-mutation; serve/MCP operation routing is still open in
  `DISC-002`
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
- `--json docx styles list <docx>` and `--json docx styles show <docx>` with
  Go-oracle comparison for style catalog enumeration, style-type filtering,
  nullable missing-styles-part behavior, style handles, found/not-found style
  show results, and invalid argument errors
- `--json docx comments list <docx>` with Go-oracle comparison for comment
  enumeration, `--comment-id` filtering, missing-id errors, empty documents
  without a comments part, semantic content hashes, body-block anchors, stable
  selectors, and comment handles
- `--json docx fields list <docx>` with Go-oracle comparison for simple and
  complex fields, body plus header field ordering, cached result readback,
  leading-instruction `--type` filtering, empty documents, unsupported package
  rejection, document-order mixed fields, switch-bearing field instructions, and
  table-nested fields reported as non-editable
- `--json docx tables show <docx>` with Go-oracle comparison for whole-document
  and selected-table readback, body block indexes, selectors, content hashes,
  dimensions, merged-cell detection, cell text, detailed table objects, empty
  no-table documents, bad selectors, missing main-document parts, and
  unsupported package-type rejection
- `--json docx text <xlsx>` unsupported-type rejection with direct Go-oracle
  comparison for exit code, stderr JSON, and empty stdout
- JSON error envelope for an invalid slide number
- `--json pptx replace text ... --out <pptx>`
- `--json --strict validate <pptx>`
- `pptx render ... --format json` manifest shape, with real-tool execution when
  LibreOffice and Poppler are available and a deterministic test hook for the
  frozen contract
- `--format json verify <pptx> --baseline <pptx>` validation plus semantic text
  diff envelope for the frozen PPTX fixture
- `serve` JSON-RPC open, op, inspect, validate, plan, commit, and abort flow for
  the frozen XLSX cell-edit session
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
  paths, Rust advertises 27, and the harness pins the 263-command gap until
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
