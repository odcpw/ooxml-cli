# Pass 3 Agent-Ergonomics Scorecard

Scope: focused codebase-tightening pass for the capabilities object-kind discovery contract.

Inventory:
- reused pass-1 inventory as the baseline
- changed one discovery contract surface: `objectKindsIndex`
- re-scored 1 focused surface

## Scores

| Surface | Weighted | Main Before | Main After |
|---|---:|---|---|
| `field__capabilities__objectKindsIndex` | 890 | `objectKindsIndex` was a hand-maintained parallel map that had stale and missing entries relative to command `targetObjectKinds`. | `objectKindsIndex` is derived from `commands[].targetObjectKinds` for all and filtered capabilities output, with exact invariant tests. |

## Findings

- The codebase is tight in behavior paths, but the manual object-kind table was real code waste: a second source of truth beside command metadata.
- The derived index intentionally follows command rows after `--for` filtering, so the emitted index describes the same command set agents are inspecting.
- The new contract test catches unknown target kinds, stale index entries, and missing index entries.

## Verification

- `cargo fmt`
- `cargo test --test rust_contract_smoke capabilities`
- `target/debug/ooxml --json capabilities` derived-index runtime diff check
- `bash agent_ergonomics_audit/audit/regression_tests/R-011__object_kinds_index_generated.test.sh`

Residual risk: broader release gates were not rerun; this pass touched only capability JSON generation and capability contract tests.
