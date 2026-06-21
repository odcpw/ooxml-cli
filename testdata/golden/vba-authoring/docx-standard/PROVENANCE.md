# DOCX Standard VBA Authoring Golden

Generated from this repo's pure Rust writer:

```powershell
cargo run --quiet --bin ooxml -- --json vba build-bin --family docx --source testdata/golden/vba-authoring/docx-standard/AgentDoc.bas --out testdata/golden/vba-authoring/docx-standard/vbaProject.bin
cargo run --quiet --bin ooxml -- --json vba inspect-bin testdata/golden/vba-authoring/docx-standard/vbaProject.bin --family docx
```

Inputs:

- `AgentDoc.bas`

Expected binary:

- `vbaProject.bin`
- size: 5120 bytes
- sha256: `d372fcdb4a7e43352242b92c67f348a630a75247087f689357537476f15502a3`

This fixture covers standard `.bas` module authoring for the Word/DOCM host
family plus the synthesized Word `ThisDocument` host document module required
for Word-open proof. It intentionally does not cover user-supplied Word `.cls`,
UserForms, signatures, or protected projects.
