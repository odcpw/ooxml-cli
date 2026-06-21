# Agent-Ergonomics Playbook

## Applied Now

1. Make `capabilities --for` forgiving for the names agents actually guess.
2. Teach exact corrections for malformed discovery flags.
3. Normalize common help aliases for worksheet rule families.
4. Publish filter vocabulary in `robot-docs guide`.
5. Split ambiguous VBA create modes into pure Rust and legacy Office-COM contracts.
6. Publish conditional-format rule-type flag constraints in capability JSON.

## Next Pass

1. Add one read-only mega-command, likely `ooxml agent-triage`, that returns quick reference, health, top discovery commands, and recommended next invocations in one JSON envelope.
2. Extract a shared alias registry used by capabilities, help, and robot-docs.
3. Extend `flagConstraints` to data-validations create/update/delete and chart creation where flags have mode-specific combinations.
4. Decide whether `capabilities` text mode should remain full JSON or gain a compact human rendering; keep stdout parseable either way.

## Operating Rule

Treat `ooxml --json capabilities` as the canonical contract and `ooxml --json robot-docs guide` as the in-tool handbook. Every future alias or complex flag mode should appear in both, with a Rust contract test and an audit replay script.
