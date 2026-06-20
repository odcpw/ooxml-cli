# Goal: First-Class Rust Port of ooxml-cli

Build a Rust implementation of `ooxml-cli` that reaches proven parity with the
current Go implementation while becoming easier for agents and humans to extend.
The Go CLI is the oracle. The Rust CLI is the subject. We do not claim parity
from intention, similar-looking output, or partial smoke tests. We claim parity
only when the proof loop says the Rust subject behaves like the Go oracle for the
declared command surface and produces Office files that real Office can open.

This file is the operating charter for the Rust port. A fresh agent should be
able to read this file, inspect the repo, and continue without needing the chat
history.

## Current Ground Truth

- Main repo path: `C:\Users\olidc\OneDrive\Desktop\Projects\ooxml-cli`.
- Active Rust branch: `codex/ooxml-rust-port`.
- Go reference branch: `codex/ooxml-go-reference`.
- Current `origin/master` includes hardening commit
  `acf3961 Fix two CFB unbounded-memory DoS bugs; harden OPC loader; add ingest fuzz harnesses`.
- Current verification on 2026-06-20 showed `origin/master` at `acf3961`, with
  `origin/master` and `acf3961` both ancestors of `codex/ooxml-rust-port`.
  No master hardening merge/rebase is pending unless `origin/master` advances.
- Rust toolchain is installed on this Windows box:
  `rustc 1.96.0`, `cargo 1.96.0`, MSVC Build Tools with `link.exe`.
- Go is installed: `go1.26.4 windows/amd64`.
- .NET SDK is installed and the Open XML SDK validator builds.
- GitHub CLI is authenticated as `odcpw`.
- Current Rust crate:
  - `Cargo.toml` package `ooxml-rs-port`, binary `ooxml`.
  - `src/main.rs` is now a slim entrypoint/facade after the first
    de-monolithization waves. Current growth pressure is in command-family
    dispatch, capability metadata, OOXML mutation modules, and large contract
    test shards.
  - `tests/rust_contract_smoke.rs` contains the Go-vs-Rust contract harness.
- Last setup checks:
  - `cargo check --all-targets` passed.
  - `cargo fmt --check` passed.
  - `cargo clippy --all-targets -- -D warnings` passed.
  - `cargo test --all-targets` passed with 4 unit tests and 203 Rust contract
    tests after the template leaf integration.
  - The frozen Go contract, serve-flow, and PPTX mutation/validation slices are
    green on Windows.
  - Current capability ratchet: Go advertises 290 command paths, Rust
    advertises 288, leaving 2 unported paths after the template leaf
    integration: `ooxml conformance check` and `ooxml pptx diff`.
  - Open XML SDK validation and desktop PowerPoint COM open proof passed for the
    generated `template apply` and `pptx template compile` decks.

## Definition of Done

The Rust port is done when all of the following are true:

1. The Rust branch contains the current `master` hardening and remains current
   with future Go safety/security fixes.
2. Every Rust-advertised command is a strict subset of the Go oracle command
   inventory until intentionally promoted.
3. Every promoted Rust command matches the Go oracle for exit code, stdout JSON,
   stderr JSON/text, error envelopes, emitted readback commands, mutation output,
   validation behavior, serve behavior, and MCP behavior where applicable.
4. The full supported command surface is either ported, intentionally excluded
   with written rationale, or tracked as open debt in the status document.
5. Rust proof gates are green:
   - `cargo check`
   - `cargo fmt --check`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo test`
   - `cargo test --test rust_contract_smoke`
6. Open XML proof gates are green for mutated/generated files:
   - strict repo validation
   - Open XML SDK schema validation
7. Windows Office proof gates are green for relevant output files:
   - Word opens DOCX/DOCM outputs without repair prompts.
   - Excel opens XLSX/XLSM outputs without repair prompts.
   - PowerPoint opens PPTX/PPTM outputs without repair prompts.
   - Macro-enabled surfaces receive VBA-specific Office proof when touched.
8. The Rust implementation is no longer a single-file accumulation zone. The
   monolith is split into proven, behavior-preserving modules with clear seams,
   no command drift, and no casual reshuffling.
9. The agent-facing CLI remains excellent: stable JSON, stdout as data, stderr
   as diagnostics, useful errors, deterministic handles, pasteable readback
   commands, discoverable capabilities, and no surprise interactivity.
10. The branch is committed and pushed at stable milestones with a concise status
    update in `docs/rust-port-status.md`.

## Non-Negotiables

- Go is the oracle. Rust is the subject.
- Do not run broad Go test suites after every Rust-only edit. Use targeted Go CLI
  oracle comparisons for the command paths under port. Run broader Go tests only
  for milestone gates, Go-side edits, frozen contract changes, or concrete
  oracle doubt.
- Do not claim parity unless the current evidence proves it.
- Do not add new Rust feature surface on top of a broken proof loop.
- Do not keep piling logic into `src/main.rs` or newly extracted accumulation
  zones. New work must either use existing focused modules or happen as part of
  a safe split plan.
- Do not split the Rust monolith aesthetically. Splits must be isomorphic:
  behavior identical, public command/API behavior identical, performance not
  meaningfully worse, compile behavior neutral or better.
- Do not rely on `br`, `bv`, `ntm`, or Agent Flywheel tooling in this Windows
  workflow unless they are actually installed and verified. For now use Codex
  subagents and normal git worktrees.
- Treat desktop Office as a real oracle for "can users open this file?" when a
  slice creates or mutates Office documents.
- Prefer small, evidence-backed slices over sweeping rewrites.

## Required Skills

Use installed skills explicitly with `$` names when doing work under this goal:

- `$reality-check-for-project`: periodically compare implemented code against
  this goal, the README, and status docs.
- `$codebase-archaeology`: map the Go implementation, Rust subject, command
  dispatch, fixtures, and proof harness before changing unfamiliar areas.
- `$testing-golden-artifacts`: maintain frozen Go contract artifacts and keep
  path/date/session scrubbing deterministic on Windows.
- `$testing-conformance-harnesses`: keep Go-vs-Rust subject/oracle/comparator
  tests as the main correctness loop.
- `$running-the-gauntlet-on-your-rust-port`: use the three-pillar lens:
  conformance, surface parity, and performance. Do not run the full gauntlet
  unless explicitly chosen; use the discipline continuously.
- `$de-monolithize-your-codebase-isomorphically`: split remaining Rust and test
  accumulation points only through proven seams and proof gates.
- `$simplify-and-refactor-code-isomorphically`: simplify after behavior is
  locked by tests; no speculative rewrites.
- `$multi-pass-bug-hunting`: run audit, fix, rescan loops on the Rust subject and
  the harness.
- `$agent-ergonomics-and-intuitiveness-maximization-for-cli-tools`: keep the CLI
  excellent for agents and non-interactive automation.
- `$testing-metamorphic`: add OOXML invariants such as validate-after-edit,
  inspect-after-commit, unzip/rezip stability, relationship integrity,
  content-type integrity, idempotent operations, and round-trip preservation.
- `$testing-fuzzing`: fuzz malformed ZIP/OPC/XML edges, relationships, content
  types, CFB/VBA, corrupted slides/sheets/docs, and known ingest boundaries.
- `$multi-model-triangulation`: use Grok as a second-opinion reviewer for
  architecture, de-monolithization plans, and risky parity gaps. I cannot call
  Grok directly; generate copy-paste prompts and synthesize responses returned
  by the user.
- `$agent-fungibility-philosophy`: use interchangeable agents working from clear
  slices; avoid fragile specialist bottlenecks.

## Parallel Execution Model

We want maximum safe parallelization. This machine has enough RAM, so read-only
scouting and independent implementation slices should run concurrently.

Use Codex subagents for parallel work. Use Grok via `$multi-model-triangulation`
for independent review prompts when the decision is hard to reverse.

Parallelism rules:

1. Split work into independent lanes: harness repair, master hardening tracking,
   de-monolithization planning, DOCX surface, XLSX surface, PPTX surface,
   serve/MCP surface, Office proof gates, fuzz/metamorphic gates.
2. Read-only scouting can run aggressively in parallel.
3. Parallel writers must use separate git worktrees or disjoint files with an
   explicit reservation note in the handoff.
4. Shared proof resources are serialized:
   - full Rust test suite
   - Office COM automation
   - Open XML SDK validator runs on shared generated outputs
   - branch integration and pushes
5. Each slice must leave a clear handoff: files touched, tests run, remaining
   risks, and whether Office proof is still needed.
6. One integration lane merges slices, resolves conflicts, reruns proof gates,
   and pushes stable milestones.
7. No subagent may declare full parity from its slice alone.

Suggested parallel lanes:

- Lane A: Windows proof loop and golden scrubber reliability.
- Lane B: track `master` hardening; merge/rebase only when `origin/master` is no
  longer an ancestor of the Rust branch, then resolve conflicts safely.
- Lane C: de-monolithization census and seam plan for current Rust/test
  accumulation points.
- Lane D: Rust clippy and hygiene fixes that do not change behavior.
- Lane E: command-surface inventory gap analysis against Go capabilities.
- Lane F: Office/Open XML proof gate smoke commands and documentation.
- Lane G: Grok review prompts for architecture and de-monolithization choices.

## Proof Ladder

Use this ladder for every slice. Later gates can be skipped only when clearly
irrelevant and the reason is written in the handoff.

1. Local compile and formatting:
   - `cargo check`
   - `cargo fmt --check`
2. Lint and test build:
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo test --no-run`
3. Targeted Rust tests:
   - one test or command-family subset first
   - then `cargo test --test rust_contract_smoke` when the slice affects parity
4. Targeted Go oracle comparison:
   - run the Go CLI for the exact command path being ported or audited
   - compare exit code, stdout, stderr, JSON shape, and generated file behavior
5. Open XML proof:
   - strict repo validation
   - Open XML SDK validator for generated/mutated Office files
6. Desktop Office proof on Windows:
   - Word/Excel/PowerPoint COM/open proof for risky or milestone document edits
   - VBA/macro proof for XLSM/PPTM/DOCM surfaces
7. Broad milestone gate:
   - full Rust tests
   - relevant Office smoke
   - status doc update
   - commit and push

## Windows Office Proof Policy

This project manipulates Office documents. Schema validity is necessary but not
sufficient. The files must open in the respective Office apps.

Use the repository tools where possible:

- `tools/windows-office-edit-smoke.ps1`
- `tools/windows-office-vba-smoke.ps1`
- `tools/windows-office-oracle.ps1`
- `tools/openxml-validator/`

Office proof cadence:

- Fast inner loop: Rust tests plus repo strict validation.
- Medium gate: Open XML SDK validator, skipping Office COM when the change is
  purely internal and no new output files are created.
- Milestone gate: desktop Office open proof for DOCX/XLSX/PPTX outputs.
- Macro gate: Office-authored and Office-opened VBA proof for XLSM/PPTM/DOCM
  when macro-enabled surfaces are touched.
- Release gate: both Open XML SDK and Office COM proof, plus VBA smoke if macro
  support is in scope.

Office COM automation is a shared resource. Do not run multiple Office COM smoke
suites concurrently.

## De-Monolithization Direction

The Rust implementation was allowed to pile into `src/main.rs` during
bootstrapping. The entrypoint is now slimmed down, but the same discipline
applies to newly extracted accumulation points.

Use `$de-monolithize-your-codebase-isomorphically` before expanding major new
feature surface. The goal is not to "make files prettier"; the goal is to make
the Rust port safe for many agents to work on in parallel.

Likely module seams to investigate, not blindly impose:

- process entry, argument parsing, output/error envelope
- capability inventory
- OPC/ZIP package loading and saving
- XML helpers
- DOCX read/mutate operations
- XLSX read/mutate operations
- PPTX read/mutate operations
- serve JSON-RPC session engine
- MCP protocol surface
- validation and diagnostics
- command emission/readback helpers
- test fixture helpers and contract scrubbers

Rules for splitting:

1. First map symbols, call clusters, shared state, and command ownership.
2. Establish baseline gates before moving code.
3. Extract one seam at a time.
4. Keep facade/re-export behavior stable where needed.
5. Do not mix code movement with behavior changes.
6. Preserve blame where practical with mechanical moves.
7. Run the proof ladder after each meaningful split.
8. If a seam is not proven, leave it alone and record why.

## Immediate Phase Plan

### Phase 0: Freeze the Setup Baseline

- Record the Windows setup facts in handoff/status.
- Use the VS developer environment when invoking Cargo from this Codex process:
  call `VsDevCmd.bat` and prepend `%USERPROFILE%\.cargo\bin` to PATH.
- Confirm `dotnet build tools/openxml-validator/openxml-validator.csproj` stays
  green.
- Do not start feature work until Phase 1 is clean.

### Phase 1: Make the Proof Loop Trustworthy

Fix the proof loop before expanding Rust surface:

- Fix Windows path scrubbing in the golden contract harness.
- Normalize `diagnostics` empty-array behavior where the contract expects it, or
  update the contract intentionally if the Go oracle changed.
- Fix existing Clippy failures without behavior changes.
- Ensure:
  - `cargo check`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test --no-run`
  - targeted golden contract checks

### Phase 2: Track Current Go Hardening

- Verify current `origin/master` is an ancestor of `codex/ooxml-rust-port`.
- Merge or rebase only if new `origin/master` hardening is not yet present.
- Preserve Go as oracle and avoid broad Go churn.
- Resolve conflicts with priority on security and ingest hardening.
- Re-run relevant Rust and Go-oracle checks.

### Phase 3: De-Monolithize Safely

- Run a monolith census of Rust files and tests.
- Produce a seam plan for the current Rust and contract-test accumulation
  points.
- Generate Grok review prompt for the seam plan if the split strategy is not
  obvious.
- Extract the first proven seams with no behavior change.
- Keep tests green after each extraction.

### Phase 4: Command Surface Expansion

- Compare Go and Rust capability inventories.
- Pick the next high-value command family based on agent utility and Office risk.
- Port by command path, not vague module mirroring.
- For each command path, add Go-vs-Rust parity cases and status-doc updates.

### Phase 5: Office and Metamorphic Hardening

- Add Office proof for risky document mutations.
- Add metamorphic tests for OOXML invariants.
- Add fuzz targets for malformed packages and ingestion boundaries.
- Keep fuzz and Office results as gates for release, not vague confidence.

### Phase 6: Milestone Commit and Push

- Update `docs/rust-port-status.md`.
- Commit only coherent milestones.
- Push to `origin/codex/ooxml-rust-port`.
- Leave a concise handoff: what is proven, what remains, and which command slice
  should be next.

## How to Use Grok

Use `$multi-model-triangulation` for copy-paste prompts to Grok when:

- choosing the de-monolithization architecture,
- evaluating whether a parity gap is real or harness noise,
- reviewing a risky Office/VBA mutation strategy,
- deciding between command-surface priorities.

The prompt should include:

- the relevant files and line references,
- the Go oracle behavior,
- the Rust subject behavior,
- the proposed change,
- exact proof gates,
- a request to be critical and identify hidden risks.

Paste Grok's response back into the current Codex thread. Codex synthesizes the
recommendation and implements only the parts that survive evidence.

## Handoff Format for Subagents

Every subagent or parallel lane should report:

```text
Lane:
Files read:
Files changed:
Command paths affected:
Proof run:
Office/Open XML proof:
Known risks:
Next suggested action:
```

If a lane changes code, it must also report whether it touched:

- command output shape,
- file mutation behavior,
- validation behavior,
- serve/MCP behavior,
- Office-openability risk,
- the capability inventory.

## Do Not Forget

- The user wants a first-class `ooxml-cli`, not endless harness churn.
- Harness work is justified only when it makes the port safer, faster to prove,
  or easier for agents to use.
- The Rust port must become modular before it grows much further.
- Windows Office is available; use it as proof for real document openability.
- Parallelism is desired, but correctness gates decide what lands.
- Commit and push stable milestones when the evidence is clean.
