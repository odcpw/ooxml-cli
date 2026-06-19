# Goal: Rust Port With Go Oracle Parity

Build a Rust implementation of `ooxml-cli` while preserving the current Go
implementation as the reference oracle. The final state is:

1. A pushed Go reference branch at the frozen baseline.
2. A pushed Rust branch with proven parity against the Go reference.

Do not claim parity from intention, partial tests, or similar-looking output.
Parity means current evidence proves the Rust subject matches the Go oracle for
the relevant command surface.

Current override: do not run Go test suites for routine Rust port slices. We are
not changing Go. Use targeted Go CLI oracle comparisons for the exact command
path being ported, then Rust-focused verification for the Rust subject.

Goal constraint: routine Go test suites are out of scope for Rust-only slices.
We are not changing Go, so the Go side is exercised by targeted CLI oracle
comparisons only unless a milestone, Go-side edit, contract freeze/update, or
specific oracle doubt makes broader Go verification necessary.

Immediate execution rule: do not run Go test suites after Rust-only work. We are
not changing Go in ordinary port slices; use targeted Go CLI oracle comparisons
instead.

Plain-English rule from Oliver: do not run Go tests each time. We are not
changing Go. For normal Rust-only work, prove behavior with exact Go CLI oracle
commands and Rust verification, not Go test suites.

## Test Cadence Override

This is a Rust port. Normal slices change Rust, not Go. Do **not** run Go tests
after each Rust-only slice. In particular, do **not** run `go test ./...`, Go
package tests, or Go unit tests as reflex checks while only the Rust subject is
changing. Use the Go implementation by running its CLI as the oracle for the
exact command path being ported or audited. Reserve Go tests for milestone
gates, Go-side edits, frozen contract changes, or a concrete reason to distrust
the oracle baseline. If the current slice changes only Rust code, Rust tests,
Rust docs/status files, or the differential harness, the default loop is Rust
verification plus focused Go CLI oracle comparisons, not Go test suites.

## Parallelization Rule

Parallelize evidence gathering aggressively, but serialize writes unless each
writer has a separate worktree. The optimal loop is:

- Use read-only scouts for Go CLI behavior, help text, fixtures, command-family
  inventories, negative cases, and focused parity findings.
- Assign distinct command-family slices so agents do not duplicate discovery or
  produce overlapping recommendations.
- Keep exactly one writer per checkout. If multiple writers are needed, create
  separate worktrees and merge only after each slice has passed its local proof.
- Keep `GOAL.md`, the capability inventory, and the parity harness as the shared
  coordination surface; do not rely on chat state as the durable tracker.
- A slice is mergeable only when it has targeted Go CLI oracle evidence, focused
  Rust tests, formatting, linting where relevant, and no unexplained parity
  mismatch.

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
- Do not run Go tests after every Rust-only slice. We are not changing Go in
  normal Rust port slices, so Go test suites do not belong in the routine
  feedback loop.
- In regular Rust port work, do not run `go test ./...`, package-level Go
  tests, or Go unit tests. Run the Go CLI only as the comparison oracle for the
  exact command path under port.
- Do not run Go tests "just in case" after Rust-only edits. The absence of Go
  changes is itself the reason to skip Go test suites.
- Reserve Go tests for milestone gates, Go-side edits, frozen contract changes,
  or a concrete reason to distrust the oracle baseline.
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
   differential parity gates, and relevant web smoke checks. Do not rerun Go
   tests for each Rust-only slice; run them only for milestones or when Go-side
   code/contracts changed.
10. Repeat until the Rust branch is at proven full parity and pushed.

## Short Prompt

Use this when character budget is tight:

```text
Read and follow GOAL.md. Use the named `$` skills there. Continue the Rust port
until Go is preserved as the oracle branch and the Rust branch reaches proven
full parity through the frozen contract and Go-vs-Rust conformance harness. Do
not run Go tests for each Rust-only slice; Go is not being changed. Use targeted
Go CLI oracle checks unless a milestone gate or Go/contract change requires
broader Go verification.
```
