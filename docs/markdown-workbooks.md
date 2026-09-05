# Markdown workbooks

`xlsx build --from-markdown report.md --out report.xlsx` compiles Markdown into
an XLSX build spec and uses the same atomic, strictly validated batch as JSON
input. Use `--emit-spec report.json` to inspect or edit the intermediate spec.
Either input accepts `-` for stdin. `--spec` and `--from-markdown` are mutually
exclusive. Typed MCP `build_workbook` accepts the same source as a `markdown`
string instead of `spec`, with either an output path or a Serve session.

Each H1 or H2 section containing a pipe table becomes a worksheet named after
the heading. A table before any heading uses `Sheet1`. Heading-only sections
do not create sheets. Sheet names must be unique ignoring case and satisfy
Excel's 31-character limit and forbidden-character rules. Put one table and
at most one `chart` fence in each section; extra tables and charts are errors.
Other Markdown blocks produce warnings instead of becoming worksheet cells.

The first table row supplies column names. Inference considers all nonempty
data cells in a column:

| Column values | Type |
| --- | --- |
| Finite numeric literals | number |
| Numeric literals ending in `%` | percent, stored as a fraction |
| ISO `YYYY-MM-DD` dates | date, stored as Excel serial numbers |
| `true`/`false` or `yes`/`no`, ignoring case | boolean |
| Empty columns or mixed types | text |

A header suffix overrides inference: `(text)`, `(number)`, `(currency)`,
`(percent)` or `(percentage)`, `(date)`, or `(boolean)`. The suffix is removed
from the column name. Explicit hints use the existing XLSX typed-column
validation; incompatible values fail before package publication. For example,
`Code (text)` preserves leading zeroes, `Revenue (currency)` accepts `$1,250`,
and `Approved (boolean)` also accepts `1`/`0`. Blank data cells remain blank.
Column widths derive from content, bounded between 8 and 60 Excel width units.
The header is styled and frozen, and each section gets a banded table with an
autofilter.

A final row whose first cell is `Total` requests a native table totals row.
The converter removes that marker row from the data and asks the table writer
to calculate sums for number and currency columns after the first column.
Other totals cells remain blank. Supplied totals values are recalculated and
reported with a warning. `Total` may appear only once, at the end of the table.

A `chart` fence contains the existing XLSX chart specification as JSON. With
no explicit source, the chart uses the section's table. For a selected range:

````markdown
# Sales

| Region | Units | Revenue (currency) |
| --- | ---: | ---: |
| North | 12 | $1,506.00 |
| South | 9 | $1,260.00 |
| Total | | |

```chart
{"type":"column","title":"Units by region","source":{"path":"self","sheet":"Sales","range":"A1:B3"},"options":{"anchor":"E2"}}
```
````

Front matter accepts the existing `metadata`, `themeSeed`, and `brand` build
spec fields. Unmapped fields produce warnings. See the committed
[`mapping-xlsx.md`](../testdata/markdown/mapping-xlsx.md) fixture for a workbook
with multiple sheets, all inferred types, explicit hints, totals, and a chart.

`xlsx ranges export --format markdown` reads table values back as Markdown.
Formatting and chart configuration are not encoded in that table readback;
use the emitted JSON spec when those details must be retained.
