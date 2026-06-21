# XLSX Standard VBA Authoring Golden

This fixture is a pragmatic regression slice for pure Rust VBA authoring.

- Source fixtures: `AgentSmoke.bas`
- Workflow: `ooxml vba build-bin --family xlsx --source AgentSmoke.bas --out vbaProject.bin`
- Expected generated binary: `vbaProject.bin`
- Binary size: 4096 bytes
- Binary sha256: `21479229375710ab564da290ba3e32f430a70ec1bbeaac9b4998a18037faf19c`
- Expected inspect output: `inspect-bin.json`
- Host package validation fixture: `testdata/xlsx/minimal-workbook/workbook.xlsx`

The golden is intentionally small and deterministic. It covers the XLSX/XLSM
standard-module workflow without a class module, including `build-bin`, package
attach, strict validation, conformance, source list, and source extraction.
