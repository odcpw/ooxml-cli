# VBA Macro Support

`ooxml-cli` supports practical VBA package workflows for Excel, PowerPoint, and Word macro-enabled files. The implementation is deliberately conservative: XLSM, PPTM, and DOCM can be authored from `.bas` / `.cls` source through the pure Rust `vba create --pure` path, with host document modules synthesized where needed. XLSM can package, list, and extract minimal `.frm` UserForm source, but generated forms are not runtime-loadable yet. Package wiring is mutated safely, and source streams are only rewritten where the supported behavior is proven.

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
| DOCX/DOCM | `/word/vbaProject.bin` | `/word/document.xml` | `.docm` |

Shared constants:

- VBA part content type: `application/vnd.ms-office.vbaProject`
- VBA relationship type: `http://schemas.microsoft.com/office/2006/relationships/vbaProject`
- XLSM main content type: `application/vnd.ms-excel.sheet.macroEnabled.main+xml`
- PPTM main content type: `application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml`

## Command Surface

```bash
ooxml --json vba inspect <file>
ooxml --json vba build-bin --family xlsx|pptx|docx --source Module1.bas --source Worker.cls --out vbaProject.bin
ooxml --json vba create workbook.xlsx --pure --family xlsx --source Module1.bas --source Worker.cls --out workbook.xlsm
ooxml --json vba create deck.pptx --pure --family pptx --source Module1.bas --out deck.pptm
ooxml --json vba create document.docx --pure --family docx --source Module1.bas --source Worker.cls --out document.docm
ooxml --json vba rebuild workbook.xlsm|deck.pptm|document.docm --source-dir macros --out rebuilt.xlsm|rebuilt.pptm|rebuilt.docm
ooxml --json vba create output.xlsm|output.pptm --family xlsx|pptx --source Module1.bas --source Worker.cls [--extract-bin vbaProject.bin] [--enable-vba-object-model-access] [--force]
ooxml --json vba extract-bin <file> --out vbaProject.bin
ooxml --json vba inspect-bin vbaProject.bin --family xlsx|pptx|docx
ooxml --json vba attach <file.xlsx|file.pptx|file.docx> --bin vbaProject.bin --out output.xlsm|output.pptm|output.docm
ooxml --json vba remove <file.xlsm|file.pptm|file.docm> --out output.xlsx|output.pptx|output.docx
ooxml --json vba list <file.xlsm|file.pptm|file.docm>
ooxml --json vba extract <file.xlsm|file.pptm|file.docm> --out-dir macros/
ooxml --json vba replace-module <file.xlsm|file.pptm|file.docm> --module Module1 --source Module1.bas --expect-sha256 <sha256> --allow-experimental-vba-source-rewrite --out output.xlsm|output.pptm|output.docm
ooxml --json xlsx forms entry --out entry-form.xlsm --field Name --field Email --field Notes
```

Implemented behavior:

- Detect package macro state and VBA consistency.
- Build source-only/cache-free XLSM/PPTM/DOCM `vbaProject.bin` files in pure Rust.
- XLSM/PPTM/DOCM pure authoring accepts `.bas` and `.cls` source modules.
- Create simple Excel entry forms with worksheet input cells, a VML-backed non-ActiveX Form Control button, and a generated VBA submit macro.
- XLSM pure authoring accepts `.frm` UserForm source for package/list/extract workflows only; Office runtime load is not supported yet.
- `.frx` sidecars, valid MSForms designer type-info generation, and binary-backed form controls are refused instead of guessed.
- DOCM pure authoring synthesizes Word's `ThisDocument` host module when needed.
- Attach pure-generated VBA projects to existing or freshly scaffolded `.xlsx` / `.pptx` / `.docx` packages with `vba create --pure`.
- Rebuild an existing `.xlsm` / `.pptm` / `.docm` package from a directory of supported source files with `vba rebuild --source-dir`.
- Run an explicit local Excel macro execution smoke for a generated XLSM with `ooxml --json vba run-smoke`.
- Create fresh Office-authored `.xlsm` / `.pptm` files from `.bas` / `.cls` source modules on Windows desktop Office as a legacy/fallback path.
- Extract `vbaProject.bin` byte-for-byte.
- Inspect standalone seeds before attachment with host-family compatibility warnings.
- Attach/remove `vbaProject.bin` while updating relationships and content types.
- Parse CFB/MS-OVBA enough to list/export parseable `.bas` and `.cls` modules.
- Replace an existing parseable module source stream with a source SHA-256 guard.
- Preserve exact no-op replacement bytes.
- Refuse signed packages for attach/remove/source-changing rewrites.
- Refuse Office-shaped module-set add/remove before writing output.

## Worksheet Form Controls

For a simple Excel form that users can fill in directly on a worksheet, use
`xlsx forms entry` instead of VBA UserForm authoring:

```powershell
ooxml --json xlsx forms entry `
  --out .\out\entry-form.xlsm `
  --field Name `
  --field Email `
  --field Notes `
  --button "Submit Entry"
ooxml --json validate --strict .\out\entry-form.xlsm
ooxml --json vba list .\out\entry-form.xlsm
```

The command creates a fresh `.xlsm` with a form sheet, an entries sheet, a
non-ActiveX Form Control button stored in VML, and a generated `SubmitEntry`
macro. Desktop Excel proof on Windows showed the workbook opens without repair,
the button runs the assigned macro after the file is trusted, input cells are
cleared, and the submitted row is appended to the entries sheet.

## Pure XLSM/PPTM/DOCM Creation Path

When you need an `.xlsm`, `.pptm`, or `.docm` from VBA source files, use `ooxml vba create --pure`. It builds the VBA project binary in Rust, attaches it to the input package, and returns `ooxml` readback/validation commands:

```powershell
ooxml --json xlsx scaffold .\workbook.xlsx --force
ooxml --json vba create .\workbook.xlsx `
  --pure `
  --family xlsx `
  --source .\macros\Module1.bas `
  --source .\macros\Worker.cls `
  --out .\out\workbook.xlsm
ooxml validate --strict .\out\workbook.xlsm
ooxml --json vba list .\out\workbook.xlsm
```

For a minimal XLSM UserForm, pass a `.frm` source alongside any standard modules:

```powershell
ooxml --json vba create .\workbook.xlsx `
  --pure `
  --family xlsx `
  --source .\macros\AgentSmoke.bas `
  --source .\macros\Dialog.frm `
  --out .\out\userform.xlsm
ooxml --json validate --strict .\out\userform.xlsm
ooxml --json vba list .\out\userform.xlsm
ooxml --json vba extract .\out\userform.xlsm --out-dir .\out\macros
```

The current `.frm` path writes PROJECT `Package`/`BaseClass` entries, `dir` metadata, module source, and minimal root designer storage streams. Computer Use testing with Excel shows these generated forms open as package content but fail runtime instantiation with an ActiveX Designer type-information mismatch. Treat this as package/list/extract support, not working interactive UserForms. `.frx` sidecars, embedded controls, valid MSForms designer stream generation, and PPTM/DOCM form packaging are not supported yet.

For PowerPoint:

```powershell
ooxml --json pptx scaffold .\deck.pptx --title "Macro Deck" --force
ooxml --json vba create .\deck.pptx `
  --pure `
  --family pptx `
  --source .\macros\Module1.bas `
  --out .\out\deck.pptm
ooxml validate --strict .\out\deck.pptm
ooxml --json vba list .\out\deck.pptm
```

For Word, pass standard `.bas` modules and optional class `.cls` modules. `ooxml-cli` synthesizes the `ThisDocument` host module:

```powershell
ooxml --json docx scaffold .\document.docx --text "Macro document"
ooxml --json vba create .\document.docx `
  --pure `
  --family docx `
  --source .\macros\Module1.bas `
  --source .\macros\Worker.cls `
  --out .\out\document.docm
ooxml validate --strict .\out\document.docm
ooxml --json vba office-check .\out\document.docm --out-dir .\proof\pure-docm-office-check
```

Use `vba build-bin` when you specifically want a standalone `vbaProject.bin` artifact:

```powershell
ooxml --json vba build-bin --family xlsx --source .\macros\Module1.bas --out .\out\vbaProject.bin
ooxml --json vba attach .\workbook.xlsx --bin .\out\vbaProject.bin --out .\out\workbook.xlsm
ooxml --json vba build-bin --family pptx --source .\macros\Module1.bas --out .\out\ppt-vbaProject.bin
ooxml --json vba attach .\deck.pptx --bin .\out\ppt-vbaProject.bin --out .\out\deck.pptm
ooxml --json vba build-bin --family docx --source .\macros\Module1.bas --source .\macros\Worker.cls --out .\out\doc-vbaProject.bin
ooxml --json vba attach .\document.docx --bin .\out\doc-vbaProject.bin --out .\out\document.docm
```

Use `vba rebuild` when you have an extracted source directory and want to replace
the package's user module set with a fresh pure Rust VBA project:

```powershell
ooxml --json vba rebuild .\workbook.xlsm --source-dir .\macros --out .\rebuilt.xlsm
ooxml --json validate --strict .\rebuilt.xlsm
ooxml --json vba list .\rebuilt.xlsm
```

## Office-Authored Creation Path

When you specifically need an Office-authored XLSM/PPTM seed, use legacy `ooxml vba create` without `--pure`. It drives desktop Office to author the VBA project, then returns `ooxml` readback/validation commands:

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

This remains available for troubleshooting and comparison, but pure Rust authoring is the preferred path.

`tools/windows-office-vba-create.ps1` remains the backend/fallback for direct troubleshooting. Agents should prefer `ooxml vba create` because it validates inputs, discovers the helper from the checkout, and emits normalized follow-up commands.

## Why Add/Remove Is Guarded

Real Office-shaped projects include version-dependent `VBA/_VBA_PROJECT` metadata that tracks the module set. The current source writer can update `dir`, `PROJECT`, module streams, and compiled-cache cleanup, but it does not regenerate `_VBA_PROJECT`.

Therefore:

- `vba add-module` and `vba remove-module` are only safe for synthetic/source-only test projects.
- On real Office-shaped `.xlsm`/`.pptm`/`.docm` inputs, those commands fail before writing output, even with `--allow-experimental-vba-source-rewrite`.
- The user-facing path for macro module-set changes is `vba rebuild --source-dir` or `vba create --pure` with the desired source files.

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

Pure generated PPTM PowerPoint-open proof:

```powershell
ooxml --json pptx scaffold .\deck.pptx --title "Macro Deck" --force
ooxml --json vba create .\deck.pptx --pure --family pptx --source .\macros\Module1.bas --out .\deck.pptm
ooxml --json vba office-check .\deck.pptm --out-dir .\proof\pure-pptm-office-check
```

For PPTM class-module proof, pass both the standard entrypoint and class source:

```powershell
ooxml --json vba create .\deck.pptx --pure --family pptx --source .\macros\Module1.bas --source .\macros\Worker.cls --out .\deck.pptm
ooxml --json vba office-check .\deck.pptm --out-dir .\proof\pure-pptm-class-office-check
```

Pure generated DOCM Word-open proof:

```powershell
ooxml --json docx scaffold .\document.docx --text "Macro document"
ooxml --json vba create .\document.docx --pure --family docx --source .\macros\Module1.bas --source .\macros\Worker.cls --out .\document.docm
ooxml --json vba office-check .\document.docm --out-dir .\proof\pure-docm-office-check
```

Explicit generated-XLSM macro execution proof:

```powershell
ooxml --json vba run-smoke `
  --timeout-seconds 45 `
  --out-dir .\proof\xlsm-run-smoke
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
- opens macro-enabled outputs, including pure-generated XLSM/PPTM/DOCM standard and class scenarios, through desktop Office COM in the full lane.
- separately executes a harmless generated XLSM macro in the explicit `vba run-smoke` lane.

Proof level `microsoft-office-com-open` means desktop Office opened the package without repair/failure. The explicit `vba run-smoke` lane also proves one generated XLSM macro can execute under opt-in local Excel automation; it does not prove arbitrary macro security or broad VBE compile coverage.

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
- General macro execution automation in the CLI.
- VBE compile proof.
- User-supplied `ThisDocument` document-module replacement beyond the synthesized host module.
- Procedure/function-level editing helpers.
- Signatures/resigning.
- Runtime-loadable generated UserForms.
- Valid MSForms designer type-info stream generation.
- `.frx` import/export and binary-backed form controls.
- PPTM/DOCM UserForm packaging.
- Password/protection editing.
- Access/ACCDB VBA.
