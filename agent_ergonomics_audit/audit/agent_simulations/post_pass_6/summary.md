# Pass 6 simulation summary

The isolated fresh agent completed all six canonical outcomes using only the
four allowed public discovery surfaces. It produced five packages; every
package passed strict validation and the Open XML SDK validator. The edited
deck passed design-check with zero errors and rendered to five PNG slides plus
PDF through LibreOffice.

| Task | Success | CLI round trips | Wrong invocations | Useless errors | Hint used |
|---|---:|---:|---:|---:|---|
| Branded deck from spec | yes | 7 | 1 | 0 | useful theme diagnostic, then public build schema |
| Workbook with chart | yes | 4 | 0 | 0 | capabilities build/check entries |
| Report document | yes | 3 | 0 | 0 | capabilities build/check entries |
| Markdown-sourced deck | yes | 3 | 0 | 0 | generated recipe and capabilities |
| Edit and check deck | yes | 5 | 0 | 0 | capabilities semantic replace workflow |
| Design-check and render | yes | 2 | 0 | 0 | capabilities command entries |

The table allocates the 24 task-specific CLI calls to the task they informed;
four shared discovery calls bring the exact session total to 28. Counts include
semantic readback, strict validation, `check --openxml-sdk require`, and
task-specific confirmation. They are evidence counts, not an optimization
score. Wall time was not instrumented and is therefore reported as unknown.

The deterministic regression harness uses a smaller 14-command first-success
sequence: two discovery calls, four builds, five package checks, one semantic
edit, one design check, and one render. Its reviewed golden intentionally
excludes optional exploratory readback while the transcript retains every
independent simulator command and correction.
