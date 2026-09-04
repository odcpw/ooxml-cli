# Pass 6 Agent-Ergonomics Scorecard

Scope: fresh-context execution of six canonical OOXML authoring and proof
outcomes using only README, the shipped OOXML skill, root help, and
machine-readable capabilities.

## Outcome

| Measure | Result | Denominator |
|---|---:|---:|
| Canonical tasks completed | 6 | 6 |
| Produced packages strict-valid | 5 | 5 |
| Produced packages Open XML SDK clean | 5 | 5 |
| CLI invocations that exited nonzero | 1 | 28 |
| Useless CLI errors | 0 | 1 CLI error |
| Rendered slides | 5 | 5 |

The lone nonzero invocation put a brand-kit path in the `theme` field while
preparing a supplied specification. The diagnostic named the rejected value
and enumerated valid built-in themes; the public schema then exposed `brand` as
the correct property. It is recorded as a useful error, not hidden from the
score.

## Regression guard

`tests/fresh_agent_simulation.rs` independently reads the four public surfaces,
derives the documented first-success commands, compares them with
`testdata/golden/fresh-agent-simulation/command-sequence.json`, executes all 14
steps, and proves each generated package with strict validation plus the SDK
validator. The render assertion requires all five slide images when
LibreOffice is available and can be made mandatory in proof lanes.

This means documentation drift is executable: removing or renaming a documented
command or capability flag fails before package mutation, while runtime or
validator regressions fail at their actual step.

## Evidence

- `agent_simulations/post_pass_6/fresh-agent-session.md`: exact independent
  command transcript, correction, output summaries, and proof classification.
- `agent_simulations/post_pass_6/summary.md`: per-task success, round trips,
  wrong invocations, hints, and useless-error counts.
- `canonical_tasks.md`: stable six-task outcome definitions.
- `tests/fresh_agent_simulation.rs`: executable public-surface and real-package
  proof.
- `testdata/golden/fresh-agent-simulation/command-sequence.json`: reviewed
  first-success command sequence.

No desktop Microsoft Office compatibility claim is made. The proof classes in
this pass are strict package validation, Open XML SDK schema validation, and
local LibreOffice rendering.
