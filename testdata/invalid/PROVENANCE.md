# Invalid fixture provenance

`missing-chart-source.xlsx` is derived deterministically from
`testdata/xlsx/chart-workbook/workbook.xlsx`. Only `xl/charts/chart1.xml` is
changed: every `Data!` series reference becomes `MissingChart!`.

Regenerate and byte-compare it with:

```sh
UPDATE_FIXTURES=1 cargo test --test check committed_missing_chart_source_fixture_is_reproducible
```

Committed SHA-256:

```text
7366c399ba2ddc5193b262a3f91ec6528decdf46e319f16f41b0c6fc4f6cbef9  missing-chart-source.xlsx
```

The fixture is intentionally semantically invalid while remaining a readable
XLSX package. It exists to prove that `check` emits one actionable
`XLSX_CHART_SOURCE_INVALID` finding.
