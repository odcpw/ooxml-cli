# XLSX VBA Authoring Golden

This fixture is a pragmatic regression slice for pure Rust VBA authoring.

- Source fixtures: `AgentSmoke.bas`, `Worker.cls`
- Workflow: `ooxml vba build-bin --family xlsx --source AgentSmoke.bas --source Worker.cls --out vbaProject.bin`
- Expected generated binary: `vbaProject.bin`
- Binary size: 6656 bytes
- Binary sha256: `6afab85a97be6608d0bfdf011be599a2c4f1f018447788def5a289d9814f6172`
- Expected inspect output: `inspect-bin.json`
- Host package validation fixture: `testdata/xlsx/minimal-workbook/workbook.xlsx`

The golden is intentionally small and deterministic. It covers the XLSX class workflow, including synthesized Excel host modules (`ThisWorkbook`, `Sheet1`), a standard module, and a class module.
