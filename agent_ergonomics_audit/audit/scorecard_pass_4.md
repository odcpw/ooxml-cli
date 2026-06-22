# Pass 4 Agent-Ergonomics Scorecard

Scope: typed capability metadata builder pass for the primary agent discovery contract.

Inventory:
- reused pass-1 inventory as the baseline
- changed the command/flag construction path behind `capabilities --json`
- pinned the emitted schema shape and a command-local `--strict` regression

## Scores

| Surface | Weighted | Main Before | Main After |
|---|---:|---|---|
| `builder__capability_command_metadata` | 900 | Capability commands and flags were assembled through loose JSON map mutation; `flagConstraints` were attached by direct indexing in five modules. | Command and flag builders serialize typed structs; constrained commands use a named builder path, and the emitted JSON is proven unchanged. |
| `verb__capabilities__strict` | 850 | `ooxml --json capabilities --strict` rejected an advertised global flag when it appeared after the command. | `--strict` is accepted as a command-local no-op for capabilities and pinned in Rust plus R-012. |

## Findings

- The public `capabilities` payload is unchanged after sorting decoded JSON before/after the typed builder refactor.
- `flagConstraints` remains intentionally flexible `Value` payload because its schemas vary by command; only its attachment point is typed/named.
- New tests pin absence-vs-null behavior, command and flag key sets, string-typed global defaults, empty filtered indexes, and surface-specific alias shapes.

## Verification

- `cargo fmt`
- `cargo test --test rust_contract_smoke capabilities`
- `cargo test --test rust_contract_smoke utility`
- sorted decoded capabilities diff against pre-refactor payload
- `bash agent_ergonomics_audit/audit/regression_tests/R-001...R-012`

Residual risk: broader release gates were not rerun; this pass touched discovery metadata construction and focused contract tests only.
