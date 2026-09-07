# Handoff to the Windows session (2026-09-05)

State at handoff: `origin/master` = `4405384`; working tree clean on the Linux box.
Last clean-worktree verify on Linux: verify88 at `cb08071`, 1038 tests, fmt and
clippy clean. Hosted CI: fully green matrix at `b249c22`; the run at `1ebd16c`
failed the macOS and Windows compile jobs and the release perf budgets (see below).

## What to run on Legion

1. `git pull` on master.
2. `pwsh -File tools/legion-proof.ps1 -RepoRoot .` (see `docs/proof-windows.md`).
   It builds the release binary and the Open XML SDK validator, produces the
   152-command contract evidence, runs `tools/windows-office-edit-smoke.ps1`
   with Office COM enabled, builds the five canonical recipes and opens/saves
   them through Office, then writes `target/legion-proof/summary.json` and
   `report.md`. A Linux `-SkipOffice` dry run passed end to end at `a5d0a45`.
3. Paste `report.md` into bead `ooxml-epic-e-gu5.1` and close it if every
   Office row passed. A timeout is a failure (suspected repair prompt).
4. Tag `v0.1.0` only after that proof and a green hosted matrix
   (`ooxml-epic-a-xq9.11`; `docs/release.md` and `release.yml` gate assets on CI).

## Open items

- `ooxml-epic-e-gu5.1` (open, unassigned): Rerun and record desktop Office proof on Legion after the fixes
- `ooxml-epic-a-xq9.11` (open, unassigned): Execute the v0.1.0 tag and release with verified assets
- `ooxml-w2-yxg.12` (in_progress, IvoryGate): xlsx ranges set 100k cells regressed past its release budget (2622 ms vs 2500 ms limit)
- `ooxml-w2-yxg.11` (in_progress, AzureOwl): Perf budgets: largest-committed-workbook workload must not pick producer-defective corpus files
- `ooxml-us9` (open, unassigned): LibreOffice corpus DOCX uses invalid font charset characterSet value

## Known hosted-CI failures at `1ebd16c`

- performance-budgets: reproduced locally. The perf workload now discovers
  `testdata/corpus/libreoffice/sales.xlsx` as the largest committed workbook and
  `check` flags that file by design (`ooxml-w2-yxg.11`); `xlsxRangesSet100kCells`
  measured 2622 ms against a 2500 ms limit (`ooxml-w2-yxg.12`, WIP committed at
  `4405384`).
- Rust compile (macos-latest) and (windows-latest): failed in "Run full portable
  Rust test gate"; the logs could not be fetched before the host rebooted (GitHub
  API rate limit). Rerun CI on master and read those two jobs first.

## Operating notes

- `AGENTS.md` carries the wave rules (target dirs off tmpfs, two-step staged
  commits, manifest regeneration with in-crate pins, environment-neutral goldens,
  Windows path scrubbing, never kill peer processes).
- The deprecated Go tree was removed; it survives at tag `go-reference-final`.
- `web/.env` on the Linux box holds the production Flue env copied from cx43
  (gitignored); the credentialed model smoke passed with it. The Flue upgrade
  questions are in `docs/flue-agentic-audit.md`.
