# Release Real-File Trace Golden Provenance

This golden records the compact, reviewed summary emitted by the portable Rust
contract test for the highest-value OOXML release traces:

- XLSX charts
- XLSX data validations
- XLSX conditional formats
- pure Rust VBA package/source workflows for XLSX, PPTX, and DOCX hosts
- PPTX charts

The chart lanes use committed LibreOffice headless re-export fixtures:

- `testdata/xlsx/libreoffice-chart-workbook/workbook.xlsx`
- `testdata/pptx/libreoffice-chart-simple/presentation.pptx`

Regenerate only after reviewing the diff:

```bash
UPDATE_GOLDENS=1 cargo test --test rust_contract_smoke release_real_file_traces_cover_high_value_surfaces -- --nocapture
```

The test writes real temporary package outputs, runs strict validation and
conformance checks, executes readback commands, asserts XLSM macro preservation
for chart and data-validation mutations, and compares a deterministic semantic
summary against `release-real-file-traces.json`. The PPTX render lane uses
`OOXML_RUST_MOCK_RENDER=1` to prove the render command path in normal Linux CI
without requiring local LibreOffice/poppler binaries; it is not desktop Office
COM proof.

The assertion parses the expected file as JSON and compares semantic values, so
checkout-level LF/CRLF differences in the golden serialization do not cause
Windows-only failures. Exact `vbaProject.bin` SHA-256 values remain pinned here,
consistent with the dedicated VBA authoring golden suites.
