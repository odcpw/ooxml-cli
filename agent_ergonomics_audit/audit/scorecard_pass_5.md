# Pass 5 Agent-Ergonomics Scorecard

Scope: release-grade Linux-local real-file trace coverage for the high-value OOXML surfaces named in the pass-4 handoff.

Inventory:
- reused the existing command-path tests as baseline evidence
- added committed LibreOffice-exported XLSX/PPTX chart fixtures
- added one deterministic release trace golden for XLSX charts, XLSX data validations, XLSX conditional formats, VBA package/source workflows, and PPTX charts
- fixed conformance handling for Office chart style/color-style parts exposed by producer-exported fixtures

## Scores

| Surface | Weighted | Main Before | Main After |
|---|---:|---|---|
| `trace__release_real_file_surfaces` | 930 | High-value surfaces had good focused tests, but no single release trace proved saved output, strict validation, conformance, readback, and semantic content across the requested set. | `release_real_file_traces_cover_high_value_surfaces` now freezes a reviewed semantic summary over chart, data-validation, conditional-format, VBA, and PPTX chart paths. |
| `fixture__producer_exported_charts` | 850 | Chart lanes relied mostly on synthetic/python-generated fixtures. | XLSX and PPTX chart traces now run against committed LibreOffice headless re-export fixtures with provenance. |
| `validator__chart_style_content_types` | 820 | `conformance check` rejected producer-exported `style*.xml` and `colors*.xml` chart parts as chart XML mismatches. | Conformance accepts `application/vnd.ms-office.chartstyle+xml` and `application/vnd.ms-office.chartcolorstyle+xml`, pinned by a focused regression. |
| `macro_preservation__xlsx_mutations` | 820 | Conditional formatting had XLSM preservation coverage, but chart and data-validation release traces did not prove macro wiring survived XLSX-family mutations. | Release trace creates XLSM inputs, mutates chart/data-validation surfaces, validates/conformance-checks output, and asserts `/xl/vbaProject.bin` remains wired. |

## Findings

- The exported chart fixtures exposed a real conformance false positive; fixing it made the producer-exported trace usable instead of loosening the release test.
- VBA proof in this pass is Linux-local deterministic package/source proof only. The golden explicitly records that desktop Microsoft Office COM proof is still a release gate.
- Broad `xlsx_conditional` and `vba` substring filters still include stale Go-oracle comparisons. Rust-native coverage for the relevant lanes is green, but the legacy oracle should be retired or refreshed for Rust-only surfaces.

## Verification

- `UPDATE_GOLDENS=1 cargo test --test rust_contract_smoke release_real_file_traces_cover_high_value_surfaces -- --nocapture`
- `cargo test --test rust_contract_smoke release_real_file_traces_cover_high_value_surfaces -- --nocapture`
- `cargo test --test rust_contract_smoke conformance_accepts_office_chart_style_and_color_style_parts -- --nocapture`
- `cargo test --test rust_contract_smoke conformance_ -- --nocapture`
- `cargo test --test rust_contract_smoke xlsx_charts -- --nocapture`
- `cargo test --test rust_contract_smoke xlsx_data_validations -- --nocapture`
- Rust-native conditional-format focused checks for XLSM preservation, serve add/delete, reorder readback, and icon-set readback
- `cargo test --test rust_contract_smoke pptx_charts -- --nocapture`
- VBA authoring golden suites for XLSM, PPTM, DOCM, and provenance
- `cargo check --all-targets`
- `cargo fmt --check`
- `git diff --check`

Residual risk: desktop Microsoft Office COM proof and stale Go-oracle retirement remain before a release claim.
