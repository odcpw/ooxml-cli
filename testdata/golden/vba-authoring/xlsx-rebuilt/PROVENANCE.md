# XLSX VBA Rebuild Golden

This fixture pins the documented pure-Rust extract/rebuild round trip.

- Base package: deterministic output of `ooxml --json xlsx scaffold workbook.xlsx --force`
- Source fixtures: `../xlsx-class/AgentSmoke.bas` and `../xlsx-class/Worker.cls`
- Workflow: `vba create --pure` to XLSM, `vba extract --out-dir`, then `vba rebuild --source-dir`
- Manifest golden: `vba-project.json`
- Rebuilt package golden: `rebuilt.xlsm`
- Rebuilt package size: 5765 bytes
- Rebuilt package SHA-256: `7e6956874fe44868da627c806b1757c84443e474344177a6ed8a62ea46fd46b7`
- Manifest SHA-256: `a4e71b1217aa8f53e63539bd19408de40196553c2da015bad8de2f88714cca08`
- VBA project SHA-256: `6afab85a97be6608d0bfdf011be599a2c4f1f018447788def5a289d9814f6172`

The 2026-09-03 regeneration adds the scaffold-derived `xl/theme/theme1.xml`
part and the corresponding content-type, workbook relationship, and themed
default style changes. The extracted manifest and `xl/vbaProject.bin` bytes
are unchanged.

The package is a deterministic regression artifact, not desktop Office proof.
The golden test requires strict validation, conformance, identical module
readback, and byte equality before accepting it.
