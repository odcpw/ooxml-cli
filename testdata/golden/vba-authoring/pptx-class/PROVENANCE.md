# PPTX Class VBA Authoring Golden

Generated from this repo's pure Rust writer:

```powershell
cargo run --quiet --bin ooxml -- --json vba build-bin --family pptx --source testdata/golden/vba-authoring/pptx-class/AgentSlide.bas --source testdata/golden/vba-authoring/pptx-class/Worker.cls --out testdata/golden/vba-authoring/pptx-class/vbaProject.bin
cargo run --quiet --bin ooxml -- --json vba inspect-bin testdata/golden/vba-authoring/pptx-class/vbaProject.bin --family pptx
```

Inputs:

- `AgentSlide.bas`
- `Worker.cls`

Expected binary:

- `vbaProject.bin`
- size: 5120 bytes
- sha256: `417f50943286b0a7e4d01afbc7a659970bc42c586ecd9843122b4bff33ea03ea`

Expected inspect output:

- `inspect-bin.json`

This fixture covers PowerPoint/PPTM authoring with one standard `.bas` module
and one class `.cls` module. PowerPoint Office-open proof was run separately
with `ooxml vba office-check` on a PPTM created from these sources.
