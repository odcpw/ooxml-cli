# AGENTS.md

Operating law for every agent working in this repository, human-directed or
swarm. The product is `ooxml-cli`, a Rust CLI that inspects, edits,
validates, and proves Office Open XML packages for agents and scripts. Read
`README.md`, then `docs/bridge-plan-2026-09.md` for the current program, then
`skills/ooxml/SKILL.md` for the CLI operating loop.

## Toolchain on this machine

- `cargo` resolves through mise (`mise.toml` pins Rust 1.98). Use a private
  target dir: `CARGO_TARGET_DIR` is set per pane by the orchestrator; if it is
  not set, use `export CARGO_TARGET_DIR=$HOME/.cache/ooxml-cli-target/$USER-$$`.
  Set `CARGO_PROFILE_DEV_DEBUG=0`.
- Open XML SDK validator: `~/dotnet/dotnet tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll <file>`
  (the runtime-only `dotnet` on PATH cannot build or run it; `doctor` is wrong
  about this until bead `ooxml-epic-a-xq9.7` lands).
- LibreOffice: `/usr/bin/soffice` for `pptx render` and PDF conversion.
- Beads: `br` and `bv` in `~/.local/bin`. Never run bare `bv` (TUI).
- Agent Mail: `am` CLI; project key is this repo's absolute path.

## Proof ladder (from docs/testing-strategy.md)

1. `cargo fmt --check`, `cargo check --all-targets`, focused tests.
2. `cargo clippy --all-targets -- -D warnings` and `cargo test --all-targets`
   (orchestrator batch verify; agents do not run these during a wave).
3. `ooxml validate --strict <file>` on every produced package.
4. Open XML SDK validator for schema proof.
5. LibreOffice render or open for visual evidence.
6. Desktop Office COM proof on Windows for compatibility claims.

Validators are necessary, not sufficient. Never claim Office compatibility
from validators alone.

## Work rules

- Work through beads. `br ready --json` is the only ready authority.
  Claim with `br update <id> --status in_progress --actor <name>`; one
  in_progress bead per agent. Every `br` mutation carries `--actor`.
- Reserve files before editing:
  `am file_reservations reserve /home/oliver/Projects/odcpw/ooxml-cli <name> <paths...> --exclusive --ttl 7200 --reason <bead-id>`.
  Do not edit paths another agent holds. Release when done.
- Keep the design boring, small, and honest. If a feature is unsupported,
  refuse clearly instead of producing a maybe-broken file.
- Mutations to package files go through the mutation seam
  (`docs/mutation-validation-seam.md`): stage, strict validate, publish.
- Stdout is data; stderr is diagnostics. JSON contracts and exit codes are
  stable. Output bytes are deterministic.
- Update capabilities goldens only with `UPDATE_GOLDENS=1` and a reviewed
  diff; say so in the commit message.
- No destructive git: no history rewrites, no force-push, no `git reset --hard`
  on shared branches, no deleting peer work. Do not commit `target/`,
  `.beads/*.db*`, `.beads/issues.jsonl` (the orchestrator flushes beads), or
  scratch files.
- Do not run Office COM, `vba run-smoke`, or the Windows PowerShell gates from
  Linux; note them as follow-ups instead.

## Swarm operations (code-first, batch-verify)

Phase 1 (agents, parallel): claim, write real code plus real tests in the
same bead, run at most `cargo check --all-targets` and focused tests, commit
immediately with the bead id in the message, move the bead to
`batch_pending` with a comment listing commits, tests, and the mapping from
each acceptance item to a test, then take the next assigned bead.

Phase 2 (orchestrator, once per wave): commit-flush, one
`cargo fmt --check` plus `cargo clippy --all-targets -- -D warnings` plus
`cargo test --all-targets` over the union of changes, compile errors first,
failures clustered by file and returned to the same assignee as `rework`
with the exact assertion, rerun until green, then `br gate report` and
`br close` citing the run. Only the orchestrator closes beads.

## Honest credit

Process artifacts are not progress. Refusals are not delivery. Commits are
not a KPI. A close without cited evidence is a debt. Named patterns that are
refused on sight: gate self-weakening (touching tests or validators inside a
feature commit to make them pass), proof-class inflation (calling a validator
pass "Office proof"), golden regeneration reflex, commit-stream pumping,
tautological tests, easy-bead cherry-picking, self-close, scope splitting to
close the original, spec editing instead of implementation, hard-coded demo
paths. Every claimed metric states its denominator.

## Coordination

- Register once: `am agents register --project /home/oliver/Projects/odcpw/ooxml-cli --program codex-cli --model gpt-5.6-sol --name <Name>`.
- Use the bead id as the mail thread subject prefix. Check your inbox at the
  start of each bead and when blocked. Reply to the orchestrator promptly;
  do not wait for replies to start real work.
- When blocked: `br update <id> --status blocked --actor <name>` with a
  comment naming the blocker, then mail the orchestrator.
