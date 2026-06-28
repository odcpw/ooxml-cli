# ooxml-cli

[![CI](https://github.com/odcpw/ooxml-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/odcpw/ooxml-cli/actions/workflows/ci.yml)

`ooxml-cli` is a Rust command-line tool for inspecting, editing, validating, and proving Office Open XML files. It works directly on `.pptx`, `.xlsx`, `.docx`, and macro-enabled Office packages, with JSON output designed for agents and scripts.

Rust is the product path. The old Go implementation is kept under `go/` as reference material only.

## What It Handles

| Family | Common work |
|---|---|
| PowerPoint | Slides, layouts, shapes, text, images, tables, charts, notes, rendering |
| Excel | Sheets, ranges, cells, formulas, tables, pivots, names, comments, formatting |
| Word | Blocks, paragraphs, tables, styles, comments, fields, headers, images, replacements |
| VBA | Pure Rust `vbaProject.bin` authoring, attach/remove, list/extract, rebuild, Office proof |
| Package | Inspect, diff, apply ops, strict validation, conformance checks, repair/normalize |

## Install From Source

```powershell
git clone https://github.com/odcpw/ooxml-cli.git
cd ooxml-cli

$env:CARGO_TARGET_DIR = "$env:TEMP\ooxml-target"
cargo build --bin ooxml
$targetDir = (cargo metadata --format-version 1 --no-deps | ConvertFrom-Json).target_directory
& (Join-Path $targetDir "debug\ooxml.exe") version
```

```bash
git clone https://github.com/odcpw/ooxml-cli.git
cd ooxml-cli

export CARGO_TARGET_DIR="${TMPDIR:-/tmp}/ooxml-target"
cargo build --bin ooxml
"$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')/debug/ooxml" version
```

During development, prefer `cargo run --` or a resolved local binary path so an older installed `ooxml` does not shadow the repo build.

## First Commands

```powershell
cargo run -- --help
cargo run -- --json capabilities
cargo run -- --json capabilities --for vba
cargo run -- doctor
```

For agent use, start with live capabilities rather than memorized examples:

```powershell
ooxml --json capabilities --for slide
ooxml --json capabilities --for range
ooxml --json capabilities --for docx
ooxml --json capabilities --for vba
```

## Common Workflows

Inspect and validate a package:

```powershell
ooxml --json inspect .\deck.pptx
ooxml --json validate --strict .\deck.pptx
ooxml --json conformance check .\deck.pptx
```

Find text and turn the replacement into an apply plan:

```powershell
ooxml --json find "Acme Corp" .\deck.pptx
ooxml --json find "Acme Corp" .\deck.pptx --replace "New Co" --to-ops > ops.json
ooxml apply .\deck.pptx --ops .\ops.json --out .\edited.pptx
```

Edit PowerPoint semantically:

```powershell
ooxml --json pptx slides list .\deck.pptx
ooxml --json pptx shapes show .\deck.pptx --slide 1 --include-text --include-bounds
ooxml --json pptx replace text .\deck.pptx --slide 1 --target title --text "New title" --out .\edited.pptx
```

Edit Excel as structured data:

```powershell
ooxml --json xlsx scaffold .\workbook.xlsx --force
ooxml --json xlsx sheets list .\workbook.xlsx
ooxml --json xlsx ranges export .\workbook.xlsx --sheet Sheet1 --range A1:D10 --include-types
ooxml --json xlsx cells set .\workbook.xlsx --sheet Sheet1 --cell A1 --value "Hello" --out .\edited.xlsx
```

Edit Word documents:

```powershell
ooxml --json docx scaffold .\report.docx --text "Draft report" --force
ooxml --json docx blocks .\report.docx
ooxml --json docx replace .\report.docx --find "Draft" --replace "Final" --expect-count 1 --out .\final.docx
```

## VBA And Macro Files

The preferred VBA path is pure Rust authoring. It works without desktop Office on Linux, macOS, and Windows. Desktop Office is used only for optional proof gates, such as opening the file or running the explicit smoke macro.

Create an XLSM from a fresh workbook:

```powershell
ooxml --json xlsx scaffold .\workbook.xlsx --force
ooxml --json vba create .\workbook.xlsx --pure --family xlsx --source .\macros\Module1.bas --out .\workbook.xlsm
ooxml --json validate --strict .\workbook.xlsm
ooxml --json vba list .\workbook.xlsm
```

Build a standalone `vbaProject.bin` and attach it to an existing workbook:

```powershell
ooxml --json vba build-bin --family xlsx --source .\macros\Module1.bas --source .\macros\Worker.cls --out .\vbaProject.bin
ooxml --json vba attach .\workbook.xlsx --bin .\vbaProject.bin --out .\workbook.xlsm
ooxml --json vba extract .\workbook.xlsm --out-dir .\extracted
```

Use the same pure authoring shape for PPTM and DOCM:

```powershell
ooxml --json pptx scaffold .\deck.pptx --title "Macro Deck" --force
ooxml --json vba create .\deck.pptx --pure --family pptx --source .\macros\Module1.bas --out .\deck.pptm

ooxml --json docx scaffold .\document.docx --text "Macro document" --force
ooxml --json vba create .\document.docx --pure --family docx --source .\macros\Module1.bas --out .\document.docm
```

For module-set changes, rebuild from source instead of patching Office-authored binary metadata:

```powershell
ooxml --json vba extract .\workbook.xlsm --out-dir .\macros
ooxml --json vba rebuild .\workbook.xlsm --source-dir .\macros --out .\rebuilt.xlsm
```

## Proof Gates

Fast Rust checks:

```powershell
cargo fmt --check
cargo check --all-targets
cargo test --test rust_contract_smoke <filter> -- --nocapture
```

Broader local checks:

```powershell
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Windows Office proof gates:

```powershell
make check-office-schema
make check-office-com
make check-office-vba-schema
make check-office-vba-com
```

For an explicit generated XLSM macro execution proof:

```powershell
ooxml --json vba run-smoke --timeout-seconds 45 --out-dir .\proof\xlsm-run-smoke
ooxml --json vba run-smoke --smoke-mode Class --timeout-seconds 45 --out-dir .\proof\xlsm-class-run-smoke
```

Validators are necessary, but they do not prove desktop Office opens a file cleanly. Use Office or Open XML SDK proof for compatibility claims.

## Agent Skill

The repo ships a canonical agent skill at:

```text
skills/ooxml/SKILL.md
```

Use it when giving an agent access to `ooxml-cli`. It tells the agent how to discover the live command contract, mutate through semantic commands, run proof commands, and avoid stale examples. The Flue web agent imports this same file, so the CLI and web workbench share one operating guide.

For Codex, copy the bundled skill into your local skills directory:

```powershell
New-Item -ItemType Directory -Force "$env:USERPROFILE\.codex\skills" | Out-Null
Copy-Item -Recurse -Force .\skills\ooxml "$env:USERPROFILE\.codex\skills\ooxml"
```

```bash
mkdir -p ~/.codex/skills
cp -R skills/ooxml ~/.codex/skills/
```

## Limits

- `ooxml-cli` edits OOXML packages directly; it is not a full desktop Office replacement.
- Macro execution is never implicit. Use `vba run-smoke` only when local Excel execution is intended.
- XLSM supports minimal generated blank-designer `.frm` UserForms in the pure authoring path. `.frx` sidecars, binary controls, PPTM/DOCM UserForms, digital signatures, password/protection editing, and arbitrary VBE compile proof are not supported.
- `vba add-module`, `replace-module`, and `remove-module` are guarded paths. Prefer `vba create --pure`, `vba rebuild`, or opaque `vba attach`.
- Go code is not the normal development path.

## Docs

- [skills/ooxml/SKILL.md](skills/ooxml/SKILL.md): canonical agent-facing operating guide
- [docs/vba-macro-support.md](docs/vba-macro-support.md): VBA support and limits
- [docs/testing-strategy.md](docs/testing-strategy.md): proof gates and test strategy
- [docs/windows-office-oracle.md](docs/windows-office-oracle.md): Windows Office open oracle
- [docs/layout-authoring.md](docs/layout-authoring.md): PowerPoint layout authoring
- [docs/placeholder-key-rules.md](docs/placeholder-key-rules.md): stable PowerPoint placeholder keys
- [docs/translation-id-rules.md](docs/translation-id-rules.md): stable translation identifiers

## Design Rules

- Stdout is data; diagnostics go to stderr.
- Use `--json` for agent-facing reads and mutations.
- Mutations require `--out`, `--in-place`, or `--dry-run`.
- Use handles returned by the CLI instead of guessed XML paths.
- Validate every changed package.
- Treat Office open proof as stronger than package validation for compatibility claims.
