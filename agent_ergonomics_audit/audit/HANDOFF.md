# Pass 4 Handoff

## What We Did

- mode: focused typed-builder follow-up
- recommendations applied this pass: 1 / 1
- branch: `master`
- audit workspace: in-tree at `agent_ergonomics_audit/`
- source commit: `65b63093f2a02c32c7b410bbfb3cb65e491e387b`

## Uplift Summary

- `CapabilityCommand` and `CapabilityFlag` typed serializers now build command metadata instead of hand-inserting JSON map fields.
- The five `flagConstraints` commands use `capability_command_with_flag_constraints` instead of direct `command["flagConstraints"]` mutation.
- `ooxml --json capabilities --strict` is accepted and pinned, matching the advertised global-flag contract.
- Schema-shape tests now guard absence-vs-null behavior, local flag keys, alias shapes, global default types, and empty filtered indexes.

## Verification

- `cargo fmt`
- `cargo test --test rust_contract_smoke capabilities`
- `cargo test --test rust_contract_smoke utility`
- sorted decoded capabilities diff against pre-refactor payload
- `bash agent_ergonomics_audit/audit/regression_tests/R-001...R-012`

## Remaining Work

- Run broader release gates before a release tag.
- Keep `flagConstraints` as flexible JSON until the shapes settle; only then consider typed per-constraint enums.

## Land-The-Plane Status

- [ ] target's current branch pushed
- [x] no new branch was created
- [x] workspace folder will be committed alongside code
- [ ] beads created for queued work
- [x] manifest updated with `current_pass=4`
- [x] pass-4 regression checks present
