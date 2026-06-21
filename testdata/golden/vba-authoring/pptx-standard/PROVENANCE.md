# PPTX VBA Authoring Golden

This fixture is a pragmatic regression slice for pure Rust VBA authoring.

- Source fixtures: `AgentSlide.bas`
- Workflow: `ooxml vba build-bin --family pptx --source AgentSlide.bas --out vbaProject.bin`
- Expected generated binary: `vbaProject.bin`
- Binary size: 4096 bytes
- Binary sha256: `8752348bae9b3fd624c476431d706ddf03a95ddbdb24e47465ebf98a8a389d0f`
- Expected inspect output: `inspect-bin.json`
- Host package validation fixture: generated with `ooxml pptx scaffold`

The golden is intentionally small and deterministic. It covers the PPTX/PPTM standard-module workflow without Office COM.
