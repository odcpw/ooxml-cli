# Pass 1 Agent-Ergonomics Scorecard

Scope: focused full pass over the agent discovery, help alias, and high-complexity flag-composition surfaces in `ooxml-cli`.

Inventory:
- 306 command surfaces from `ooxml --json capabilities`
- 1,777 command-local flags plus 3 global flags
- 2,086 total generated inventory rows

## Scores

| Surface | Weighted | Main Before | Main After |
|---|---:|---|---|
| `verb__capabilities` | 865 | Natural filters and no-global-json flow were brittle. | First-try filters work, bad flags teach exact retry, JSON reports normalized filter metadata. |
| `flag__capabilities__for` | 838 | Filter vocabulary was under-discoverable. | Plurals, abbreviations, and modules/macros are accepted and documented. |
| `verb__xlsx_conditional-formats` | 830 | `cf` and singular topic help were easy reasonable guesses that missed. | `xlsx cf --help` and canonical aliases land on group help. |
| `verb__robot-docs_guide` | 840 | Agent guide did not teach the accepted `capabilities --for` vocabulary. | Discovery section includes examples, filters, and aliases. |
| `verb__vba_create` | 827 | Pure Rust and legacy Office-COM modes were presented as one ambiguous surface. | Help and capabilities split modes and conflict rules. |
| `verb__xlsx_conditional-formats_add` | 830 | Agents had to infer valid flag combinations for rule types. | Capability JSON exposes per-`--type` constraints. |

## Findings

- The codebase already had unusually strong agent surfaces: `capabilities`, `robot-docs`, command inventories, and contract smoke tests existed before this pass.
- The waste was not broad dead code; it was discovery friction at the exact points agents use to choose commands and compose flags.
- The most expensive confusion was vocabulary drift: command group names are plural, object kinds are singular, and agents naturally try both.
- The second major gap was mode ambiguity on complex mutating commands. `vba create` and conditional-format authoring needed machine-readable constraints, not only prose.

## Verification

- `cargo fmt`
- `cargo build`
- `cargo check --all-targets`
- `cargo test --test rust_contract_smoke capabilities -- --nocapture`
- `cargo test --test rust_contract_smoke help_alias_topics -- --nocapture`
- `cargo test --test rust_contract_smoke robot_docs_guide -- --nocapture`
- `cargo test --test rust_contract_smoke vba_create_help -- --nocapture`
- `bash agent_ergonomics_audit/audit/regression_tests/R-001__capabilities_filter_aliases.test.sh`
- `bash agent_ergonomics_audit/audit/regression_tests/R-002__capabilities_unknown_flag_teaches.test.sh`
- `bash agent_ergonomics_audit/audit/regression_tests/R-003__help_topic_aliases.test.sh`
- `bash agent_ergonomics_audit/audit/regression_tests/R-004__robot_docs_filter_vocabulary.test.sh`
- `bash agent_ergonomics_audit/audit/regression_tests/R-005__vba_create_mode_help.test.sh`
- `bash agent_ergonomics_audit/audit/regression_tests/R-006__capability_flag_constraints.test.sh`

Known environmental note: the full `utility` filter fails on `doctor_contract_commands_are_machine_readable` because this machine lacks `dotnet`; the changed utility tests passed individually.
