# Goal: Rust Port With Go Oracle Parity

Build a Rust implementation of `ooxml-cli` while preserving the current Go
implementation as the reference oracle. The final state is:

1. A pushed Go reference branch at the frozen baseline.
2. A pushed Rust branch with proven parity against the Go reference.

Do not claim parity from intention, partial tests, or similar-looking output.
Parity means current evidence proves the Rust subject matches the Go oracle for
the relevant command surface.

## Test Cadence Override

This is a Rust port. Normal slices change Rust, not Go. Do **not** run the full
Go test suite after each Rust-only slice. In particular, do **not** run
`go test ./...` as a reflex check. Use targeted Go CLI oracle comparisons only
for the exact command path being ported or audited. Reserve full Go tests for
milestone gates, Go-side edits, frozen contract changes, or a concrete reason
to distrust the oracle baseline.

## Required Skills

Use these skills explicitly:

- `$codebase-archaeology`: map the Go CLI architecture, command dispatch,
  package structure, serve mode, MCP mode, web hooks, tests, fixtures, and
  frozen contract artifacts.
- `$ooxml`: keep behavior grounded in semantic OOXML workflows, stable handles,
  validation, render/readback evidence, and the existing `ooxml` CLI contract.
- `$testing-golden-artifacts`: use the frozen Go baseline in
  `testdata/golden/rust-port-contract/` as the initial observable contract.
- `$testing-conformance-harnesses`: build and maintain the Go-vs-Rust
  differential harness.
- `$running-the-gauntlet-on-your-rust-port`: use subject/oracle/comparator
  discipline, surface parity inventory, negative ledgers, and repeated hardening
  rounds once the Rust binary exists.
- `$testing-metamorphic`: add OOXML invariants such as validate-after-edit,
  render-after-edit, unzip/rezip stability, relationship integrity, content-type
  integrity, and idempotent operations.
- `$testing-fuzzing`: fuzz malformed Office files, ZIP/package boundaries, XML
  edges, relationships, content types, and corrupted slides/sheets/docs.
- `$profiling-software-performance`: profile only after correctness parity for a
  surface is established.
- `$extreme-software-optimization`: optimize only from measured hotspots, one
  behavior-preserving lever at a time.
- `$multi-pass-bug-hunting`: run fresh-eyes audit/fix/rescan passes against both
  the Rust implementation and the parity harness.
- `$agent-ergonomics-and-intuitiveness-maximization-for-cli-tools`: keep the
  Rust CLI agent-friendly: stable JSON, stdout-as-data, stderr-as-diagnostics,
  useful errors, documented exit codes, discoverable help, `capabilities --json`,
  and no surprise interactivity.

## Dependency Rule

For Rust libraries and reusable infrastructure, first inspect and prioritize
usable repos from `https://github.com/Dicklesworthstone`. If no suitable repo
exists for a needed domain, such as ZIP, XML, OOXML parsing/writing, CLI parsing,
JSON, MCP, async/runtime, testing, fuzzing, or conformance, use mainstream Rust
crates and document why.

## Non-Negotiables

- Go is the oracle. Rust is the subject.
- Do not run the full Go test suite after every Rust-only slice. Use targeted
  Go-vs-Rust parity checks for the command being ported. We are not changing Go
  in normal Rust port slices, so reserve full Go tests for milestone gates or
  when Go code, frozen contracts, or shared oracle assumptions change.
- In regular Rust port work, do not run `go test ./...`; run the Go CLI only as
  the comparison oracle for the exact command path under port.
- Port by command surface, not vague module mirroring.
- Every implemented Rust surface must be compared against Go for stdout, stderr,
  exit code, JSON shape, mutation result, validation result, and any relevant
  serve/MCP/web behavior.
- Every mismatch is fixed or documented in
  `testdata/golden/rust-port-contract/DISCREPANCIES.md` with impact, affected
  tests, review date, and status.
- Correctness comes before performance.
- Commit and push stable milestones.
- Keep this goal active until full parity is actually proven.

## Phases

1. Preserve and push the Go reference branch at the frozen baseline.
2. Map the Go architecture and command surfaces.
3. Create the Rust branch and Rust crate/binary.
4. Build the Go-vs-Rust differential harness.
5. Port the frozen CLI baseline first: `version`, `inspect`,
   `pptx slides show`, `xlsx ranges export`, `docx text`, invalid JSON error,
   `pptx replace text`, and `validate`.
6. Add parity for `pptx render`, `verify --baseline`, `serve` JSON-RPC, MCP
   discovery/session flows, and the web `OOXML_BIN` smoke path.
7. Expand coverage from `capabilities --json` until every Go command is present,
   intentionally excluded, or tracked as open.
8. Add metamorphic tests and fuzzing.
9. Run fresh-eyes review, `cargo fmt`, `cargo clippy`, `cargo test`,
   differential parity gates, and relevant web smoke checks. Do not rerun the
   full Go suite for each Rust-only slice; run it only for milestones or when
   Go-side code/contracts changed.
10. Repeat until the Rust branch is at proven full parity and pushed.

## Short Prompt

Use this when character budget is tight:

```text
Read and follow GOAL.md. Use the named `$` skills there. Continue the Rust port
until Go is preserved as the oracle branch and the Rust branch reaches proven
full parity through the frozen contract and Go-vs-Rust conformance harness. Do
not run the full Go test suite for each Rust-only slice; Go is not being changed.
Use targeted Go-vs-Rust checks unless a milestone gate or Go/contract change
requires broader Go verification.
```
