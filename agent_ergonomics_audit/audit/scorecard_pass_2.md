# Pass 2 Agent-Ergonomics Scorecard

Scope: follow-up implementation pass for the pass-1 deferrals plus the next complex flag-contract slice.

Inventory:
- reused pass-1 inventory as the baseline
- added one read-only command surface: `ooxml agent-triage`
- re-scored 4 focused surfaces

## Scores

| Surface | Weighted | Main Before | Main After |
|---|---:|---|---|
| `verb__agent-triage` | 860 | Agents still needed separate capabilities, robot-docs, and doctor calls for first-contact triage. | One read-only JSON command returns quick refs, health, filters, aliases, warnings, and next commands. |
| `alias_registry__discovery` | 850 | Alias vocabulary lived in multiple consumers and could drift. | `agent_aliases` is the single source for capabilities filters, robot docs, and help topic aliases. |
| `verb__xlsx_data-validations_create` | 835 | Agents had to infer valid validation type/source/operator combinations. | Capability JSON exposes per-type required, optional, forbidden, and output flags. |
| `verb__charts_create` | 830 | XLSX/PPTX chart create commands accepted constrained source modes but did not publish them. | Capability JSON exposes range/table and inline/external source modes. |

## Findings

- The codebase is tight in implementation paths; the remaining waste was mostly in discovery contracts and duplicated alias vocabulary.
- `agent-triage` is deliberately additive and read-only, so it improves first-contact ergonomics without changing mutation semantics.
- The missing local `dotnet` install was an environment proof gap, not a product JSON contract failure; the test now accepts the documented advisory exit behavior.

## Verification

- `cargo fmt`
- `cargo build`
- `cargo test --test rust_contract_smoke utility`
- `cargo test --test rust_contract_smoke capabilities`
- `bash agent_ergonomics_audit/audit/regression_tests/R-007__agent_triage_shape.test.sh`
- `bash agent_ergonomics_audit/audit/regression_tests/R-008__shared_alias_registry.test.sh`
- `bash agent_ergonomics_audit/audit/regression_tests/R-009__chart_data_validation_constraints.test.sh`
- `bash agent_ergonomics_audit/audit/regression_tests/R-010__doctor_health_advisory.test.sh`

Residual risk: broader release gates were not rerun; this pass changed discovery/help metadata and contract tests, not package mutation logic.
