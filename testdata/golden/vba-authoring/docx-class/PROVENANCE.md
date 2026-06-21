# DOCX Class VBA Authoring Golden

Generated from this repo's pure Rust writer:

```powershell
cargo run --quiet --bin ooxml -- --json vba build-bin --family docx --source testdata/golden/vba-authoring/docx-class/AgentDoc.bas --source testdata/golden/vba-authoring/docx-class/Worker.cls --out testdata/golden/vba-authoring/docx-class/vbaProject.bin
cargo run --quiet --bin ooxml -- --json vba inspect-bin testdata/golden/vba-authoring/docx-class/vbaProject.bin --family docx
```

Inputs:

- `AgentDoc.bas`
- `Worker.cls`

Expected binary:

- `vbaProject.bin`
- size: 6144 bytes
- sha256: `9a0d1e425908a52909d472e794640dec13fd27d56f8b6588a3609d0420070aec`

Expected inspect output:

- `inspect-bin.json`

This fixture covers Word/DOCM authoring with a synthesized Word
`ThisDocument` host document module, one standard `.bas` module, and one user
class `.cls` module. Word Office-open proof was run separately with
`ooxml vba office-check` on a DOCM created from these sources.
