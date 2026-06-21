# GOAL: Pure Rust VBA Authoring

This repo is a Rust product. Go is deprecated reference material only.

The product goal is cross-platform VBA authoring: on Linux, macOS, or Windows,
an agent must be able to create a valid `vbaProject.bin` from `.bas` / `.cls`
source files, attach it to Office Open XML macro-enabled packages, and iterate
macro code safely without desktop Office.

Do not treat package-level `vbaProject.bin` attach/remove as enough. The core
authoring path must be pure Rust. Office COM may remain a proof oracle on
Windows, but it must not be an implementation dependency.

## Primary Specs

- MS-OVBA, Office VBA File Format Structure:
  https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-ovba/575462ba-bf67-4190-9fac-c275523c75fc
- MS-CFB, Compound File Binary File Format:
  https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb/53989ce4-7b05-4f8d-829b-d08d6148375b

The current published MS-OVBA page identifies the spec as the Office VBA file
format for Office 97 and later and describes the storage that contains embedded
macros and custom forms. The current MS-CFB page describes the compound-file
container as a file-system-like structure for application-specific streams.
Use the downloaded PDFs/DOCX specs as the detailed record source when writing
binary records.

## User Workflows

The first-class workflow is adding a macro to an existing workbook:

```powershell
ooxml vba build-bin --family xlsx --source .\Hello.bas --out .\vbaProject.bin
ooxml vba attach .\workbook.xlsx --bin .\vbaProject.bin --out .\hello.xlsm
ooxml validate --strict .\hello.xlsm
ooxml --json vba list .\hello.xlsm
ooxml --json vba extract .\hello.xlsm --out-dir .\extracted
```

The second workflow is creating a workbook from scratch and adding a macro:

```powershell
ooxml xlsx scaffold .\workbook.xlsx --out .\workbook.xlsx
ooxml vba create --pure .\workbook.xlsx --family xlsx --source .\Hello.bas --out .\hello.xlsm
```

For slice 1, `vba create --pure` may require an existing host package and may
reuse the existing scaffold command. The important part is that pure Rust builds
the VBA project binary and attaches it. Host scaffolding can be improved after
the binary writer is proven.

## Target Command Surface

Prefer these names unless implementation research proves a better agent-facing
shape:

- `ooxml vba build-bin --family xlsx --source Module1.bas --source Worker.cls --out vbaProject.bin`
- `ooxml vba create --pure workbook.xlsx --family xlsx --source Module1.bas --out workbook.xlsm`
- `ooxml vba rebuild existing.xlsm --source-dir macros --out edited.xlsm`

Keep existing commands:

- `vba attach`
- `vba remove`
- `vba inspect`
- `vba inspect-bin`
- `vba list`
- `vba extract`
- `vba extract-bin`

But make pure authoring the first-class path in help, README, capabilities, and
robot-docs once it is proven.

Agent-ergonomic requirements:

- JSON output by default under `--json`.
- Every mutating result returns follow-up `inspect`, `list`, `extract`,
  `validate`, `conformance`, and Office-proof commands where relevant.
- Errors must refuse unsupported module kinds or host families clearly.
- No command may imply Office COM is required for pure authoring.
- Dangerous overwrite paths require existing `--force` / `--out` discipline.
- Deterministic input should produce deterministic output bytes where practical.

## Implementation Shape

Create a clean Rust VBA authoring module separate from package attach/remove.
Do not let this grow into one blob. If a file starts becoming a monolith, split
by proven boundaries: model, CFB writer, MS-OVBA records, compression, CLI
output, and package attach integration.

Model the project explicitly:

- host family: `xlsx` first, then `pptx`, then `docx`
- project name, default `VBAProject`
- code page, default Windows-1252 unless source requires otherwise
- references
- modules
- module kind: standard `.bas` first, class `.cls` second
- source text, normalized intentionally

Write the CFB container and required VBA project streams/storages:

- `PROJECT`
- `PROJECTwm`
- `VBA/dir`
- `VBA/_VBA_PROJECT`
- one stream per module

Implement MS-OVBA compression for `dir` and module source where required. Reuse
the current parser/codec only after proving it writes the required format.

Handle `MODULEOFFSET` intentionally. Prefer source-only/cache-free authoring
where Office will regenerate compiled state. Do not patch Office-authored
compiled cache streams as the main path.

Out of initial scope:

- UserForms
- digital signatures
- password/protection editing
- macro execution by default
- pretending validators prove Office compatibility

## Feature Order

1. XLSM host packaging.
2. Standard `.bas` modules.
3. Class `.cls` modules.
4. `vba create --pure` wrapping build-bin + attach.
5. `vba rebuild` from extracted source directory.
6. PPTM host packaging.
7. DOCM host packaging.
8. Optional explicit macro-run smoke proof.

For add/remove/replace of modules, rebuild a fresh `vbaProject.bin` from the
parsed/extracted source model. Do not rely on unsafe mutation of Office-authored
binary metadata as the primary design.

## Testing And Proof

Linux/macOS tests must not require Office. Windows Office tests are proof gates,
not implementation dependencies.

Required local tests:

- unit tests for binary record writers
- CFB writer unit tests
- compression/decompression roundtrip tests
- golden tests for a minimal generated `vbaProject.bin`
- determinism test: same model/source -> same bytes
- roundtrip test: `build-bin -> inspect-bin -> list/extract`
- package test: `build-bin -> attach to scaffold workbook -> validate`
- conformance test: `ooxml --json conformance check` on generated packages
- Open XML SDK schema validation where available

Required Windows proof before calling this done:

- generated XLSM opens in Excel without repair
- explicit opt-in smoke can run a harmless macro, such as writing
  `Hello from ooxml` into a known cell
- later, generated PPTM opens in PowerPoint without repair
- later, generated DOCM opens in Word without repair

Validators are necessary but not sufficient. Office-open proof is required for
the claim that generated macro files work.

## Conformance And Golden Discipline

Maintain a small conformance matrix for implemented MS-OVBA/MS-CFB writer
requirements. Track known unimplemented spec areas explicitly rather than
shipping unknown gaps.

Golden artifacts must include provenance:

- generated by this Rust version / command
- source module inputs
- normalized project model
- expected inspection/list/extract output

Use `UPDATE_GOLDENS=1` only with diff review. Do not commit transient `.actual`
files.

## Efficient Engineering Loop

Do not micro-compile after every tiny edit. Work in coherent slices:

1. Read and design the slice.
2. Edit a small set of owned files.
3. Run the narrowest useful unit/contract tests.
4. Run `cargo fmt --check` and `cargo check --all-targets` at integration
   boundaries.
5. Run full/focused Office proof only after the generated package is expected
   to open.

Use parallel subagents for independent read/design/bug-hunt slices. Serialize
integration, commits, and Office COM proof gates.

Rust compile speed policy:

- Keep dev/test debug info low unless debugging needs it.
- Use focused tests by filter during implementation.
- Avoid clippy/full test suite until an integration checkpoint.
- Prefer one full verification per green milestone.

## Active Acceptance Criteria

On a Linux box without Office, this must work:

```bash
ooxml xlsx scaffold workbook.xlsx --out workbook.xlsx
ooxml vba build-bin --family xlsx --source Hello.bas --out vbaProject.bin
ooxml vba attach workbook.xlsx --bin vbaProject.bin --out hello.xlsm
ooxml validate --strict hello.xlsm
ooxml --json vba list hello.xlsm
ooxml --json vba extract hello.xlsm --out-dir extracted
```

On Windows with Office installed, the same `hello.xlsm` must open in Excel
without repair. The macro-run smoke is explicit opt-in.

Do not call the feature done until Office proves the generated file opens.

## Work Rules

- Work on `master`.
- Commit useful green checkpoints.
- Push when a checkpoint is coherent.
- Use subagents, but assign disjoint slices.
- Use de-monolithization discipline only if the VBA authoring code starts
  growing into large, coupled files.
- Keep the design boring, small, and honest.
- If a part of MS-OVBA is not implemented, the CLI must refuse clearly instead
  of producing maybe-broken macro files.
