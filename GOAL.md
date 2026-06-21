# GOAL: First-Class Pure Rust VBA Authoring

This repo is now a Rust product. Go is deprecated/reference-only.

The product goal is cross-platform Office VBA authoring: on Linux, macOS, or
Windows, an agent must be able to create a valid `vbaProject.bin` from `.bas`
and `.cls` source files, attach it to XLSM/PPTM/DOCM packages, and iterate macro
code safely. Desktop Office COM may be used as a Windows proof oracle, but it
must not be an implementation dependency for the core authoring path.

Do not pretend package-level `vbaProject.bin` attach/remove solves this. The
core feature is a pure Rust VBA project authoring device.

## Primary Specs

- MS-OVBA, Office VBA File Format Structure:
  https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-ovba/575462ba-bf67-4190-9fac-c275523c75fc
- MS-CFB, Compound File Binary File Format:
  https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb/53989ce4-7b05-4f8d-829b-d08d6148375b

Use the local/downloaded specs as the detailed record source when writing binary
records. Validators are useful, but Office-open proof is required before any
generated macro package is called done.

## First-Class Workflows

Add a macro to an existing workbook:

```powershell
ooxml vba build-bin --family xlsx --source .\Hello.bas --out .\vbaProject.bin
ooxml vba attach .\workbook.xlsx --bin .\vbaProject.bin --out .\hello.xlsm
ooxml validate --strict .\hello.xlsm
ooxml --json vba list .\hello.xlsm
ooxml --json vba extract .\hello.xlsm --out-dir .\extracted
```

Create a workbook from scratch, then add a macro:

```powershell
ooxml xlsx scaffold .\workbook.xlsx --out .\workbook.xlsx
ooxml vba create --pure .\workbook.xlsx --family xlsx --source .\Hello.bas --out .\hello.xlsm
```

The same pattern should extend to PPTM and DOCM after each host family is proven:

```powershell
ooxml pptx scaffold .\deck.pptx --out .\deck.pptx
ooxml vba create --pure .\deck.pptx --family pptx --source .\Hello.bas --out .\hello.pptm
```

For slice 1, `vba create --pure` may require an existing host package and may
reuse scaffold commands. Host-package scaffolding can improve later; the core
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
where Office will regenerate compiled state. Do not patch Office-authored
compiled cache streams as the main path.

Out of initial scope:

- UserForms
- digital signatures
- password/protection editing
- macro execution by default
- inline function lists or code editing niceties
- shipping maybe-broken macro files when a family/feature is unsupported

## Feature Order

1. XLSM host packaging.
2. Standard `.bas` modules.
3. Class `.cls` modules.
4. `vba create --pure` wrapping build-bin + attach.
5. Add macro to an existing `.xlsx` and to a scaffolded `.xlsx`.
6. `vba rebuild` from extracted source directory.
7. PPTM host packaging and pure authoring.
8. DOCM package support, then pure authoring only after Word-specific proof.
9. Optional explicit macro-run smoke proof.

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

This project should move quickly without churning.

- Do not micro-compile after every tiny edit.
- Work in coherent implementation slices.
- Run focused tests by filter during development.
- Run `cargo fmt --check`, `cargo check --all-targets`, and broader tests at
  integration checkpoints.
- Run Office proof only after generated packages are expected to open.
- Keep dev/test debug info low unless debugging needs it.
- Use the existing fast compile settings: parallel cargo jobs, incremental
  builds, and low debug info for dev/test profiles.

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

Do not call the feature done until Office proves the generated file opens.

## Work Rules

- Work on `master`.
- Commit useful green checkpoints.
- Push coherent checkpoints.
- Use subagents, but assign disjoint slices.
- Respect user/worktree changes; do not revert unrelated work.
- Keep the design boring, small, and honest.
- If a part of MS-OVBA is not implemented, the CLI must refuse clearly instead
  of producing maybe-broken macro files.
