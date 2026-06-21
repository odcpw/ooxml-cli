# Phase 0 Scope Decision

Target: `/home/oliver/Projects/odcpw/ooxml-cli`
Mode: `full`
Current branch: `master`
Audit workspace: `/home/oliver/Projects/odcpw/ooxml-cli/agent_ergonomics_audit`

## User-Confirmed Defaults

- Work in the current checkout and current branch; do not create a new branch.
- Keep the audit workspace in-tree; do not create a sibling workspace.
- Run a full agent-ergonomics pass: audit, apply top recommendations, re-score, add regression tests, and capture handoff artifacts.
- Use quick CASS mining for prior-session friction signals.
- Use peer triangulation if available; proceed solo when a helper is unavailable.

## Scope Guardrails

- Preserve backwards compatibility for existing commands and flags.
- Prefer additive aliases, better discovery, clearer help, and better error guidance over removals.
- Do not remove or rename existing commands or flags without explicit user approval.
- Prioritize the concrete discovery gaps already observed: `capabilities --for` plural/alias filters, conditional-format discoverability, dense/mixed help surfaces, and raw OOXML flag names that need better breadcrumbs.
- Keep changes focused on agent-facing CLI behavior, tests, and audit artifacts; avoid feature work that changes OOXML semantics.

## Toolchain Consent

Rust toolchain is present. If an additional toolchain becomes required, ask before installing.
