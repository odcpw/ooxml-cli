# VBA Macro Support

`ooxml-cli` supports practical VBA package workflows for PowerPoint and Excel macro-enabled files. The implementation is deliberately conservative: treat Office-authored `vbaProject.bin` payloads as compatibility truth, mutate package wiring safely, and only rewrite source streams where the supported behavior is proven.

Authoritative specs:

- MS-CFB: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb/53989ce4-7b05-4f8d-829b-d08d6148375b
- MS-OVBA: https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-ovba/b39ac32f-0ce1-4533-9297-2ff3ff62c9ec
- VBA relationship/content type references:
  - https://learn.microsoft.com/en-us/openspecs/office_standards/ms-offmacro/3a702445-ddfa-4eba-9f4c-a2d02bbb452a
  - https://learn.microsoft.com/en-us/openspecs/office_standards/ms-offmacro2/6205a8c4-f957-47ed-a64b-fae5ea96c5a0

## Supported Families

| Family | VBA part | Main part | Macro extension |
| --- | --- | --- | --- |
| XLSX/XLSM | `/xl/vbaProject.bin` | `/xl/workbook.xml` | `.xlsm` |
| PPTX/PPTM | `/ppt/vbaProject.bin` | `/ppt/presentation.xml` | `.pptm` |

DOCX/DOCM package-level support remains deferred.

Shared constants:

- VBA part content type: `application/vnd.ms-office.vbaProject`
- VBA relationship type: `http://schemas.microsoft.com/office/2006/relationships/vbaProject`
- XLSM main content type: `application/vnd.ms-excel.sheet.macroEnabled.main+xml`
- PPTM main content type: `application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml`

## Command Surface

```bash
ooxml --json vba inspect <file>
ooxml --json vba create output.xlsm|output.pptm --family xlsx|pptx --source Module1.bas --source Worker.cls [--extract-bin vbaProject.bin] [--enable-vba-object-model-access] [--force]
ooxml --json vba extract-bin <file> --out vbaProject.bin
ooxml --json vba inspect-bin vbaProject.bin --family xlsx|pptx
ooxml --json vba attach <file.xlsx|file.pptx> --bin vbaProject.bin --out output.xlsm|output.pptm
ooxml --json vba remove <file.xlsm|file.pptm> --out output.xlsx|output.pptx
ooxml --json vba list <file.xlsm|file.pptm>
ooxml --json vba extract <file.xlsm|file.pptm> --out-dir macros/
ooxml --json vba replace-module <file.xlsm|file.pptm> --module Module1 --source Module1.bas --expect-sha256 <sha256> --allow-experimental-vba-source-rewrite --out output.xlsm|output.pptm
```

Implemented behavior:

- Detect package macro state and VBA consistency.
- Create fresh Office-authored `.xlsm` / `.pptm` files from `.bas` / `.cls` source modules on Windows desktop Office.
- Extract `vbaProject.bin` byte-for-byte.
- Inspect standalone seeds before attachment with host-family compatibility warnings.
- Attach/remove `vbaProject.bin` while updating relationships and content types.
- Parse CFB/MS-OVBA enough to list/export parseable `.bas` and `.cls` modules.
- Replace an existing parseable module source stream with a source SHA-256 guard.
- Preserve exact no-op replacement bytes.
- Refuse signed packages for attach/remove/source-changing rewrites.
- Refuse Office-shaped module-set add/remove before writing output.

## Office-Authored Creation Path

When you need a new `.xlsm` or `.pptm` from `.bas` / `.cls` source files, use `ooxml vba create`. It drives desktop Office to author the VBA project, then returns `ooxml` readback/validation commands:

```powershell
ooxml --json vba create .\out\seed.xlsm `
  --family xlsx `
  --source .\macros\Module1.bas `
  --source .\macros\Worker.cls `
  --extract-bin .\out\vbaProject.bin `
  --enable-vba-object-model-access `
  --force
```

Then attach the seed to another package if needed:

```powershell
ooxml --json vba inspect-bin .\out\vbaProject.bin --family xlsx
ooxml --json vba attach .\testdata\xlsx\minimal-workbook\workbook.xlsx --bin .\out\vbaProject.bin --out .\out\workbook.xlsm
ooxml validate --strict .\out\workbook.xlsm
ooxml --json vba list .\out\workbook.xlsm
```

Use `--family pptx` and `.pptm` output for PowerPoint.

This is the supported way to change the module set for Office-facing files until general `_VBA_PROJECT` regeneration is implemented.

`tools/windows-office-vba-create.ps1` remains the backend/fallback for direct troubleshooting. Agents should prefer `ooxml vba create` because it validates inputs, discovers the helper from the checkout, and emits normalized follow-up commands.

## Why Add/Remove Is Guarded

Real Office-shaped projects include version-dependent `VBA/_VBA_PROJECT` metadata that tracks the module set. The current source writer can update `dir`, `PROJECT`, module streams, and compiled-cache cleanup, but it does not regenerate `_VBA_PROJECT`.

Therefore:

- `vba add-module` and `vba remove-module` are only safe for synthetic/source-only test projects.
- On real Office-shaped `.xlsm`/`.pptm` inputs, those commands fail before writing output, even with `--allow-experimental-vba-source-rewrite`.
- The user-facing path for module-set changes is: run `vba create` with the desired source files, or create/obtain an Office-authored seed and then `vba attach`.

## Proof Gates

Fast schema-level VBA gate:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass `
  -File .\tools\windows-office-vba-smoke.ps1 `
  -RepoRoot . `
  -RequireOpenXmlSdk `
  -SkipOffice `
  -EnableVbaObjectModelAccess
```

Full desktop Office open proof:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass `
  -File .\tools\windows-office-vba-smoke.ps1 `
  -RepoRoot . `
  -RequireOpenXmlSdk `
  -EnableVbaObjectModelAccess `
  -OfficeOracleTimeoutSeconds 120
```

Make targets:

```bash
make check-office-vba-schema
make check-office-vba-com
make office-vba-smoke-fast
make office-vba-smoke
```

The VBA smoke gate:

- creates fresh Office-native `.xlsm` and `.pptm` seeds from `.bas` / `.cls` files;
- extracts and reattaches their `vbaProject.bin` payloads;
- replaces an existing `.bas` module;
- validates with `ooxml validate --strict`;
- validates with Microsoft Open XML SDK when available;
- asserts real Office-shaped add/remove are refused;
- opens macro-enabled outputs through Excel and PowerPoint COM in the full lane.

Proof level `microsoft-office-com-open` means desktop Office opened the package without repair/failure. It does not execute macros, compile the VBA project, or prove macro security.

## Validation Diagnostics

`ooxml validate` reports package-level VBA consistency for supported families, including:

- multiple or missing VBA relationships
- orphaned or missing project parts
- wrong VBA relationship type/source/target
- wrong content type
- macro-enabled main part without a usable project
- non-macro main part with VBA artifacts
- empty project payload
- unexpected outgoing relationships from `vbaProject.bin`
- known package or VBA signature artifacts
- host-family risks such as Excel document modules in PPTM

## Out Of Scope

- General `_VBA_PROJECT` regeneration.
- Macro execution.
- VBE compile proof.
- Procedure/function-level editing helpers.
- Signatures/resigning.
- Forms and `.frx` import/export.
- Password/protection editing.
- Access/ACCDB VBA.
