# GOAL: Rust OOXML Product And First-Class VBA Authoring

This repo is now a Rust product. Go is deprecated/reference-only.

The Rust implementation owns `master` and the release line. `v0.1.0` is the first formal cross-platform binary release; release candidates must pass the complete Rust `cargo test --all-targets` gate plus the Office-specific proof appropriate to the changed surfaces.

The product goal is cross-platform Office VBA authoring: on Linux, macOS, or
Windows, an agent must be able to create a valid `vbaProject.bin` from `.bas`
and `.cls` source files, attach it to XLSM/PPTM/DOCM packages, and iterate macro
code safely. Desktop Office COM is allowed only as a Windows proof oracle. It
must not be an implementation dependency for the core authoring path.

Do not pretend package-level `vbaProject.bin` attach/remove solves this. The
core feature is a deterministic pure Rust VBA project authoring device.

## Primary Specs

- MS-OVBA, Office VBA File Format Structure:
  https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-ovba/575462ba-bf67-4190-9fac-c275523c75fc
- MS-CFB, Compound File Binary File Format:
  https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb/53989ce4-7b05-4f8d-829b-d08d6148375b

Use local/downloaded copies of those specs as the detailed binary-record source
when implementing writers. Validators are useful, but Office-open proof is
required before any generated macro package is called done.

## First-Class User Workflows

Add a macro to an existing `.xlsx`:

```powershell
ooxml vba build-bin --family xlsx --source .\Hello.bas --out .\vbaProject.bin
ooxml vba attach .\workbook.xlsx --bin .\vbaProject.bin --out .\hello.xlsm
ooxml validate --strict .\hello.xlsm
ooxml --json vba list .\hello.xlsm
ooxml --json vba extract .\hello.xlsm --out-dir .\extracted
```

Create an `.xlsx` from scratch, then add a macro:

```powershell
ooxml xlsx scaffold .\workbook.xlsx --out .\workbook.xlsx
ooxml vba create --pure .\workbook.xlsx --family xlsx --source .\Hello.bas --out .\hello.xlsm
```

The same pattern should extend to PPTM and DOCM only after each host family is
proven:

```powershell
ooxml pptx scaffold .\deck.pptx --out .\deck.pptx
ooxml vba create --pure .\deck.pptx --family pptx --source .\Hello.bas --out .\hello.pptm
ooxml docx scaffold .\document.docx --text "Macro document"
ooxml vba create --pure .\document.docx --family docx --source .\Hello.bas --out .\hello.docm
```

For slice 1, `vba create --pure` may require an existing host package and may
reuse scaffold commands. Host-package scaffolding can improve later. The core
contract is pure Rust `vbaProject.bin` generation plus safe attach.

## Target Command Surface

Prefer these names unless implementation research proves a better
agent-facing shape:

- `ooxml vba build-bin --family xlsx --source Module1.bas --source Worker.cls --out vbaProject.bin`
- `ooxml vba create --pure workbook.xlsx --family xlsx --source Module1.bas --out workbook.xlsm`
- `ooxml vba rebuild existing.xlsm --source-dir macros --out edited.xlsm`

Keep the existing package/source commands:

- `vba attach`
- `vba remove`
- `vba inspect`
- `vba inspect-bin`
- `vba list`
- `vba extract`
- `vba extract-bin`

Make pure authoring the first-class path in help, README, capabilities, and
robot docs once each family is proven.

Agent ergonomics:

- JSON output under `--json`.
- Mutating commands return follow-up commands for `inspect`, `list`, `extract`,
  `validate`, `conformance`, and Office proof when relevant.
- Unsupported host families, module kinds, signatures, forms, or protected
  projects must fail clearly.
- No pure-authoring command may imply that Office COM is required.
- Overwrites must follow existing `--force` / `--out` discipline.
- Deterministic input should produce deterministic output bytes where practical.

## Implementation Shape

Keep VBA authoring separate from package attach/remove. Split by boring,
observable boundaries instead of growing a blob:

- project model
- source-module parsing
- CFB writer
- MS-OVBA record writers
- compression/decompression
- package attach integration
- CLI output/ergonomics
- conformance and Office oracle harnesses

Model a VBA project explicitly:

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
only if Office can regenerate compiled state and run simple macros. Do not patch
Office-authored compiled cache streams as the main path.

Out of initial scope:

- runtime-loadable generated UserForms; XLSM source-only `.frm` package/list/extract support is intentionally limited and does not imply runtime support
- `.frx` payloads, MSForms designer type-info generation, or binary-backed controls
- digital signatures
- password/protection editing
- macro execution by default
- inline function lists or code editing niceties
- shipping maybe-broken macro files when a family/feature is unsupported

## Current Status

Already integrated on `master`:

- pure Rust XLSM authoring path
- pure Rust XLSM standard `.bas` macro execution proof in Excel
- pure Rust XLSM class `.cls` macro execution proof in Excel
- `ooxml --json vba run-smoke` is now the explicit opt-in CLI wrapper for
  standard and class XLSM macro execution proof through desktop Excel
- XLSM `build-bin -> attach` proof covers existing and scaffolded workbooks
  with validation, conformance, list, and extract readback
- pure Rust PPTM authoring path
- pure Rust PPTM PowerPoint open proof through `ooxml vba office-check`
- pure Rust PPTM class `.cls` PowerPoint open proof through `ooxml vba office-check`
- package-level DOCM attach/extract/remove/inspect/list support
- pure Rust DOCM standard `.bas` authoring path
- pure Rust DOCM class `.cls` authoring path
- pure Rust DOCM standard and class Word open proof through `ooxml vba office-check`
- `vba rebuild` from source directories
- durable Windows smoke proof for pure-generated XLSM/PPTM/DOCM standard and
  class packages
- Office-open proof for generated XLSM/PPTM/DOCM and package-level DOCM
- `validate --strict` and `conformance check` reject broken VBA package wiring:
  missing/duplicate `vbaProject` relationships, wrong `vbaProject.bin` content
  types, non-macro main parts with VBA payloads, and orphan VBA project parts
- XLSM can package, list, and extract minimal `.frm` source while refusing `.frx` and binary-control claims; generated UserForms are not runtime-loadable
- `xlsx forms entry` provides the supported interactive Excel form workflow through worksheet cells and non-ActiveX Form Controls
- the audited release hardening covers XML entity preservation, OPC relationships/content types, secondary DOCX diff parts, real PPTX selector-based replacement, JSON-RPC serve/MCP behavior, Windows-1252, MS-OVBA chunk bounds, and CFB cycle rejection

Known remaining gaps:

- Keep strengthening the explicit `build-bin -> attach existing/scaffolded host`
  workflows, because that is the agent-facing contract for adding macros to
  user workbooks.
- Golden/provenance coverage for generated `vbaProject.bin` outputs is still
  intentionally small; keep adding focused goldens when authoring behavior
  widens.
- PPTM pure authoring has standard and class-module package/open proof, but
  not executable macro smoke depth comparable to the XLSM Excel run harness.
- DOCM pure authoring supports user standard `.bas` modules and class `.cls`
  modules with a synthesized `ThisDocument` host module; user-supplied
  `ThisDocument` replacement remains out of scope until separately proven.

## Feature Order

1. XLSM host packaging.
2. Standard `.bas` modules.
3. Class `.cls` modules.
4. `vba create --pure` wrapping build-bin + attach.
5. Add macro to an existing `.xlsx` and to a scaffolded `.xlsx`.
6. `vba rebuild` from extracted source directories.
7. PPTM host packaging and pure authoring.
8. DOCM package support, then pure standard/class-module authoring after Word-specific proof.
9. Explicit opt-in macro-run smoke proof.

For add/remove/replace of modules, rebuild a fresh `vbaProject.bin` from the
parsed/extracted source model. Do not rely on unsafe mutation of Office-authored
binary metadata as the main design.

## Testing And Proof

Linux/macOS tests must not require Office. Windows Office tests are proof gates,
not implementation dependencies.

Required local tests:

- unit tests for every binary record writer
- CFB writer unit tests
- compression/decompression roundtrip tests
- golden tests for a minimal generated `vbaProject.bin`
- determinism test: same model/source produces the same bytes
- roundtrip test: `build-bin -> inspect-bin -> list/extract`
- package test: `build-bin -> attach to scaffold workbook -> validate`
- conformance test: `ooxml --json conformance check` on generated packages
- Open XML SDK schema validation where available

Required Windows proof before calling a family done:

- generated XLSM opens in Excel without repair
- generated PPTM opens in PowerPoint without repair
- generated DOCM opens in Word without repair
- explicit opt-in smoke can run a harmless macro, such as writing
  `Hello from ooxml` into a known cell

Validators are necessary but not sufficient. The prior `definedNames` workbook
ordering bug proved that weak validators can pass files Office rejects.

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

Move quickly without churning.

- Do not micro-compile after every tiny edit.
- Work in coherent implementation slices.
- Run focused tests by filter during development.
- Run `cargo fmt --check`, `cargo check --all-targets`, and broader tests at
  integration checkpoints.
- Treat `make check-ci` as the complete local Rust gate; it runs all unit and integration targets.
- Run Office proof only after generated packages are expected to open or execute.
- Keep dev/test debug info low unless debugging needs it.
- Use Cargo parallelism, incremental builds, low debug info, and an unsynced
  local target directory.
- Avoid OneDrive-synced build artifacts for normal Cargo work.

Use parallel subagents for independent slices. Good lanes:

- one implementation worker per host-family or command-family gap
- one test/golden/conformance worker
- one read-only archaeology/bug-hunt worker when design is uncertain
- one integration lane, owned by the main agent

Serialize edits that touch the same files, commits, pushes, Office COM proof,
and any full-test gate.

## Required Skills To Apply Intelligently

Use the appropriate local skills as needed, especially:

- `$codebase-archaeology`
- `$testing-conformance-harnesses`
- `$testing-golden-artifacts`
- `$multi-pass-bug-hunting`
- `$de-monolithize-your-codebase-isomorphically` if VBA code starts growing
  into a blob
- `$agent-ergonomics-and-intuitiveness-maximization-for-cli-tools` for the CLI
  command surface

Do not run a massive generic Rust-port process. The useful direction now is a
pragmatic, proven, safe Rust implementation.

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
without repair and the harmless macro smoke must be possible under an explicit
opt-in flag.

Do not call the feature done until Office proves the generated file opens. Do
not call executable macro authoring done until Office proves a generated macro
can run under the explicit smoke harness.

## Work Rules

- Keep `master` as the Rust product branch; prepare risky integrations in a dedicated worktree and merge only a green candidate.
- Commit useful green checkpoints.
- Push coherent checkpoints.
- Use subagents, but assign disjoint slices and do not duplicate work.
- Respect user/worktree changes; do not revert unrelated work.
- Keep the design boring, small, and honest.
- If a part of MS-OVBA is not implemented, the CLI must refuse clearly instead
  of producing maybe-broken macro files.
