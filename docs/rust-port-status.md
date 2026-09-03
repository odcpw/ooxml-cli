# Current Rust Product Status

Updated 2026-09-04.

`ooxml-cli` is a Rust product for inspecting, editing, validating, and proving
Office Open XML packages. The retired implementation is historical reference
material, not the default runtime or a correctness oracle.

## Current product surface

- The typed manifest publishes 329 command contracts. Capabilities, help,
  completion, Serve, and MCP derive their command identity from that manifest.
- `outline` provides one deterministic, family-aware orientation read.
  `check` composes structural, strict, schema, reference, layout, design, and
  optional render evidence into one finding envelope.
- `apply`, Serve sessions, and MCP use the shared staged mutation seam: edit a
  working copy, validate it, then publish it atomically.
- Published JSON schemas and direct builders cover PPTX, XLSX, and DOCX.
  Their typed MCP adapters are in the current integration wave; use
  `ooxml --json capabilities` as the live authority.
- Supported text extracts can emit readable Markdown with `--format markdown`.
  Markdown-to-PPTX/DOCX build input is still an integration surface and should
  not be treated as released until its build and proof beads are closed.
- MCP retains the generic session tools and adds typed build, edit, outline,
  check, validate, render, find, and replace intents with JSON Schemas.

## Proven on Linux and hosted CI

- Rust format, warnings-as-errors lint, build, unit, focused contract, and
  all-target batch gates are the automated source proof. Each cited batch run
  applies only to the commit union named by that run.
- Generated-package tests use strict validation. Schema-sensitive recipes add
  Microsoft Open XML SDK validation when the validator is available.
- Render tests use LibreOffice or a clearly labelled deterministic test
  renderer. They are visual evidence, not Microsoft Office proof.
- Capability, help, process, and MCP discovery outputs are pinned as reviewed
  goldens; intentional contract changes regenerate those artifacts explicitly.

These checks prove the stated structural, schema, and headless-render levels.
They do not by themselves prove that desktop Word, Excel, or PowerPoint opens
every current output without repair.

## Awaiting Windows Office and release proof

- Earlier Legion runs remain evidence for the exact artifacts and commits they
  tested. Changes from the current build/batch/Markdown/MCP wave require a new
  desktop Office run before inheriting that compatibility claim.
- Macro execution and VBE-sensitive behavior require explicit opt-in Windows
  gates; they are never implied by Linux validation.
- The Cargo version is a release candidate. No `v0.1.0` tag or GitHub Release
  assets exist yet. The release bead must verify native archives and
  `SHA256SUMS` before announcing a formal release.

The archived port chronology, including superseded Go-oracle wording, is kept
unchanged in [history/rust-port-chronology.md](history/rust-port-chronology.md).
For current proof rules, see [testing-strategy.md](testing-strategy.md).
