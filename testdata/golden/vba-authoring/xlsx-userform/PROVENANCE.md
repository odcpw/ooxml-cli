# XLSX UserForm VBA Authoring Golden

This fixture is a pragmatic regression slice for pure Rust VBA UserForm authoring.

- Source fixtures: `AgentSmoke.bas`, `Dialog.frm`
- Workflow: `ooxml vba build-bin --family xlsx --source AgentSmoke.bas --source Dialog.frm --out vbaProject.bin`
- Normalized project model: `VBAProject`, Windows-1252, synthesized `ThisWorkbook` and `Sheet1`, standard module `AgentSmoke`, and UserForm `Dialog` with caption `Golden Dialog`
- Expected generated binary: `vbaProject.bin`
- Binary size: 7680 bytes
- Binary sha256: `2f2e8d21d1615bf57d7df66a257b6c8f1091109794138905c81ac4f7d5fce3c0`
- Expected inspect output: `inspect-bin.json`
- Host package validation fixture: `testdata/xlsx/minimal-workbook/workbook.xlsx`

The golden is intentionally small and deterministic. It covers UserForm storage
streams, package attach, strict validation, conformance, source list/extract,
and extract-to-rebuild caption preservation. Generated MSForms UserForms remain
package/list/extract support only and are not runtime-loadable.
