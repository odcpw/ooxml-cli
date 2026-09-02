# XLSX VBA Rebuild Golden

This fixture pins the documented pure-Rust extract/rebuild round trip.

- Base package: deterministic output of `ooxml --json xlsx scaffold workbook.xlsx --force`
- Source fixtures: `../xlsx-class/AgentSmoke.bas` and `../xlsx-class/Worker.cls`
- Workflow: `vba create --pure` to XLSM, `vba extract --out-dir`, then `vba rebuild --source-dir`
- Manifest golden: `vba-project.json`
- Rebuilt package golden: `rebuilt.xlsm`
- Rebuilt package size: 4972 bytes
- Rebuilt package SHA-256: `a3101c5238f329dcfb6dde3dbdc385b8cdbce52418654e7f59c46d8f2a4d5114`
- Manifest SHA-256: `a4e71b1217aa8f53e63539bd19408de40196553c2da015bad8de2f88714cca08`

The package is a deterministic regression artifact, not desktop Office proof.
The golden test requires strict validation, conformance, identical module
readback, and byte equality before accepting it.
