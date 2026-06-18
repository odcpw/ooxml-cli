# Rust Port Status

The Go implementation remains the reference on `codex/ooxml-go-reference`.
Rust work lands on `codex/ooxml-rust-port`.

The frozen Go contract lives in `testdata/golden/rust-port-contract/baseline.json`.
The first Rust slice implements and tests the CLI cases from that baseline:

- `--json version`
- `--json inspect <pptx>`
- `--json pptx slides show ... --include-text`
- `--json xlsx ranges export ... --include-types`
- `--json docx text <docx>`
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

Still missing before parity can be claimed:

- real render proof parity beyond the mocked frozen manifest path.
- MCP discovery and session parity.
- Full command-surface inventory parity.
- Metamorphic and fuzz harnesses for OOXML package invariants.
- Office/Open XML SDK/COM proof gates.

Dependency note: live GitHub inspection of `https://github.com/Dicklesworthstone`
found useful Rust infrastructure projects, but no direct OOXML/ZIP/XML package
library. The initial Rust subject therefore uses mainstream Rust crates for ZIP,
XML, and JSON handling while keeping Dicklesworthstone projects as the preferred
source for future MCP, async/runtime, TUI, and agent ergonomics patterns.
