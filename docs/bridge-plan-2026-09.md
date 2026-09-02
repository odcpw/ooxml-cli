# Bridge Plan: ooxml-cli as the agent-first Office authoring tool

Reality check date: 2026-09-03
Scope: OOXML only (PPTX/PPTM, XLSX/XLSM, DOCX/DOCM, VBA). No new file families.
Status: plan space. Beads are generated from this document; the beads are the
authority once created. This document is revised in place, never forked.

## 1. Vision restated

An agent such as Claude, Codex, or Gemini, working from a shell or over MCP,
must be able to produce an excellent, good-looking, valid Office file from
nothing, and to edit an existing one, with:

1. first-guess commands that work or redirect precisely;
2. one call for one intent, including "build this whole deck";
3. output that opens in desktop Office without repair, every time;
4. built-in design quality so a from-scratch file is not ugly by default;
5. proof at every step: validation, schema, render, layout QA, Office open.

The README already promises 1, 3, and 5. Evidence gathered on 2026-09-03 shows
2 and 4 are missing, 3 is broken for pivots, and 1 fails for most first guesses.

## 2. Evidence summary (what the code actually does today)

Verified on Linux with Rust 1.98, LibreOffice, and the Open XML SDK validator:

- 564 tests pass; fmt clean; clippy fails on stable 1.98 (3 lints).
- CI red since commit e6c8600 (2026-07-18): 3 of 64 Windows smoke scenarios
  fail. Root cause: `xlsx pivots create` writes a `<pivotTableParts>` child
  into the worksheet XML. The Open XML SDK rejects it, Excel will prompt for
  repair, and the repo's own `validate --strict` and `conformance check` pass
  it. CI uploads no artifacts, so the failing scenario names never reached
  the log.
- Documented `vba extract` then `vba rebuild --source-dir` fails with
  "duplicate VBA module name Sheet1".
- `doctor` reports the Open XML SDK validator as available when only the
  dotnet runtime exists.
- From-scratch PPTX: scaffold ships one layout ("Title Slide"). There is no
  content layout, so `new-slide-from-layout "Title and Content"` fails. Cloning
  the title slide and adding text produces a title that overlaps the chart;
  `validate-layout` reports zero collisions because inherited placeholder
  bounds are not resolved. Multi-line `--set-text` produces one paragraph
  with an embedded newline, not bullets.
- From-scratch DOCX: scaffold ships no `styles.xml`, `numbering.xml`,
  `settings.xml`, or theme. `paragraphs append --style Heading1` silently
  writes a dangling style reference that validation, conformance, and the SDK
  all accept; Word renders it as Normal. `docx blocks` readback omits style.
- From-scratch XLSX: the pieces exist (`ranges set`, `tables create`,
  `set-style`, `set-format`, `colwidths`, `freeze`, `charts create`) and the
  result validates clean, but every guessed flag name was wrong
  (`--cell`, `--col`, `--values`, `--bold`) and each error was only
  `unknown flag: --x` with no valid-flag list and no suggestion.
- Cross-family flag drift for the same concept: `--after`, `--insert-after`,
  `--block`, `--index`, `--after-block`; `--file` vs `--image`; `--values`,
  `--values-file`, `--values-json`, `--data`, `--cells`.
- Every mutation is one process, one new file, one validation. `apply --ops`
  batches, but only 70 of 152 mutating commands are `opCompatible`.
- Mutation readback is inconsistent: `chartPart`, `primarySelector`,
  `paragraphCount` come back null on several commands. The artifact proof
  matrix reports 152 mutating commands and 93 with no proof row at all.
- MCP exposes 7 generic tools (open, op, inspect, validate, plan, commit,
  abort); no typed authoring tools with schemas.
- Scaffold output is byte-deterministic (good). No `SOURCE_DATE_EPOCH`
  handling exists anywhere, which is fine while no timestamps are written.
- Repo hygiene: no tag or release; README says v0.1.0 is released; help
  says "Rust port of ooxml-cli"; the status doc is 2,188 lines of chronology;
  211,000 lines of deprecated Go remain in-tree with 17 Makefile targets.
- Beads: 2 open, 50 closed. None of the above is tracked.

## 3. Gap register

Legend: P0 release blocker, P1 core vision, P2 excellence, P3 hygiene.
Every gap lists the intended bead shape. Test beads are mandatory companions.

### Track A: Release blockers (P0)

#### A1 Pivot worksheet XML is invalid
Current: `src/xlsx_pivots.rs` around line 1246 emits
`<pivotTableParts count="1"><pivotTablePart r:id=.../></pivotTableParts>` into
the worksheet root and parses it back on read.
Target: worksheets never contain that element. Pivot tables are discovered
through worksheet relationships of type `.../relationships/pivotTable` plus
the workbook `pivotCaches` list. Existing files that already contain the bogus
element are repaired on the next mutation and flagged by validation.
Success:
- Open XML SDK reports 0 errors for the three CI pivot scenarios.
- `xlsx pivots list/show` still find pivots in files created by Excel and by
  the old writer.
- `validate --strict` flags a worksheet that contains `pivotTableParts`.
- Windows CI job green; Office COM open proof rerun on Legion.
Complexity: M.

#### A2 Clippy red on current stable
Current: three `manual_slice_fill` style lints in `src/vba/cfb.rs` (lines
269 and 1389) and `src/vba/codec.rs` (line 294). `make check-ci` includes
clippy with warnings denied, so the Ubuntu job fails on the next push.
Target: clippy clean on stable; a documented policy for lint drift: CI runs
stable, release pins 1.96, and a weekly scheduled CI run on beta gives early
warning.
Complexity: S.

#### A3 Extract then rebuild round trip fails
Current: `vba extract` writes host document modules (ThisWorkbook, Sheet1,
ThisDocument) as `.cls` files. `vba rebuild` reads every `.cls` as a class
module and then synthesizes host modules again, so the model rejects the
duplicate name.
Target: extract writes a `vba-project.json` manifest into the output dir
(project name, code page, module list with kind and `hostSynthesized`).
Rebuild consumes the manifest when present. Without a manifest, rebuild
classifies document modules by `VB_PredeclaredId = True` plus
`VB_Exposed = True` attributes and the family's known host names, and never
synthesizes a host module that the source set already provides. User-supplied
host module source is used verbatim for XLSM and PPTM; for DOCM it remains
refused with a clear message until Word proof exists.
Success: the README round trip passes for xlsm, pptm, and docm in a contract
test; rebuilt packages validate strict and list the same modules.
Complexity: M.

#### A4 CI hides failures
Current: the Windows smoke job prints only "61 passed, 3 failed". Summary
JSON and the proof matrix stay on the runner.
Target: the smoke script prints each failed scenario with stage and detail to
stdout; CI uploads `summary.json`, `artifact-proof-matrix.{json,md}`, and the
failed output packages as workflow artifacts; a step summary lists failures.
Complexity: S.

#### A5 Doctor false positive for the SDK validator
Current: `src/doctor.rs` treats any `dotnet` binary as proof. The runtime
without an SDK cannot build or run the validator.
Target: doctor runs `dotnet --list-sdks`, requires an 8.x SDK, reports the
exact remediation command, and reports the built DLL path when present.
Complexity: S.

#### A6 Linux schema gate
Current: Open XML SDK validation runs only on the Windows CI job and only over
the smoke scenarios. Linux developers and agents get no schema proof.
Target: `conformance check --openxml-sdk` invokes the validator when an SDK
is present and reports its findings inside the conformance JSON; the Ubuntu CI
job installs the .NET SDK and runs the validator over every artifact produced
by the release trace goldens and the from-scratch recipes (Track D).
Complexity: M.

#### A7 Validator blind spot for main-part child order
Current: `validate --strict` checks workbook child order only. Worksheet,
slide, slide layout, master, and document body child order are unchecked, so
an invalid element in the wrong place passes.
Target: embed the ISO 29500 child sequence for `CT_Worksheet`,
`CT_Workbook`, `CT_Slide`/`CT_SlideLayout`/`CT_SlideMaster` common shape
tree, `CT_Document` body and `sectPr`, and the chart space root. Strict
validation reports unknown or out-of-order children with the offending part
and XPath. Unit tests use fixtures derived from the pivot bug.
Complexity: M.

#### A8 Release execution
Current: no tag, no release, README claims a release line.
Target: after A1 through A7 are green and the Legion Office proof is rerun,
create the annotated tag `v0.1.0`, verify the four assets and `SHA256SUMS`,
install from the release, run the acceptance sequence against the installed
binary, and update README, the status doc, and the release-readiness doc to
state facts. A `CHANGELOG.md` is generated from git history.
Complexity: S (after prerequisites).

### Track B: First guess works (P1)

#### B1 Errors teach the exact fix
Current: `src/cli_args.rs` has three sites that emit `unknown flag: --x`.
Unknown command tokens say "run `ooxml help`".
Target: every invalid-args error carries a structured envelope:
`code`, `message`, `hint`, `didYouMean` (Levenshtein distance 1 to 2 against
the leaf's `localFlags` from the manifest, and against alias registry
entries), `validFlags`, `helpCommand`, and `correctedCommand` (the full
invocation with the substitution applied). Unknown command tokens suggest the
nearest command paths from the manifest. Missing required flags list the
required set and an example. The text mode prints the same information on
stderr. Exit codes stay as documented.
Success: a corpus of at least 200 wrong invocations generated from the
manifest (every leaf, every flag, typo classes: transposition, drop, wrong
family, plural or singular) produces `didYouMean` or `validFlags` in 100 percent
of cases; contract test pins the envelope schema.
Complexity: M.

#### B2 One vocabulary across families
Current: same concept, different flags per family (see evidence).
Target: a shared alias registry (`src/agent_aliases.rs` grows into the single
owner) that maps accepted aliases to canonical flags per leaf, surfaced in
`capabilities` as `localFlags[].aliases`, in help, and in `robot-docs guide`.
Canonical set, with aliases accepted everywhere the concept exists:
- position: `--after <n>` canonical; `--insert-after`, `--after-block`,
  `--block` accepted for insert commands.
- target index: `--index` canonical; `--block` accepted for docx styles.
- data input: `--values` (inline JSON), `--values-file`, `--data-format`
  canonical; `--values-json`, `--data`, `--cells`, `--cells-file` accepted.
- media: `--image` canonical for pictures; `--file` accepted.
- ranges: `--range` canonical; `--col`, `--cols`, `--columns` accepted on
  column commands; `--cell` accepted where a single cell is meant.
- freeze: `--at <cell>` accepted and translated to rows and cols.
Aliases never remove existing flags. Deprecation is not required.
Success: contract test asserts every alias resolves; capabilities golden
updated; intent corpus from B1 shows the alias cases succeed.
Complexity: M.

#### B3 Verb naming that matches intent
Current: `pptx text set` styles runs and has no `--text`; text replacement is
`pptx replace text`. `xlsx freeze set` wants `--rows`/`--cols` while Excel
users think "freeze at A2".
Target: `pptx text set` accepts `--text` (and `--paragraphs-file`) and sets
content and style in one call; `pptx replace text` remains. `xlsx freeze set
--at A2` works. Any mismatch between a legible intent and a wrong verb gets a
`didYouMean` from B1 that names the right verb.
Complexity: S.

#### B4 Guards that help instead of block
Current: `docx blocks insert-after` requires `--expect-hash` even when the
agent just created the document. `docx paragraphs insert` has no guard at
all. The two paths are inconsistent.
Target: hash guards stay available on every block-addressed docx mutation and
are required only when the caller passes `--require-guard` or when the
package changed since the readback (detected by comparing a content hash
returned by `docx blocks` with the current block); otherwise a missing guard
produces a warning field in the JSON. All mutation readbacks return the new
block hashes so the next call can be guarded without a second read.
Complexity: M.

#### B5 Consistent mutation envelope
Current: destination fields differ per command and are often null.
Target: every mutating command returns `destination` with `file`, `partUri`,
`primarySelector`, `handle`, and the created or changed object summary;
plus `readbackCommand`, `validateCommand`, `conformanceCommand`, and
`renderCommand` where a renderer exists. A single contract test runs every
one of the 152 mutating commands through a fixture and asserts the envelope
schema (this closes the 93 "no proof row" gaps at the structural and readback
tiers).
Complexity: L.

#### B7 One read to orient: `ooxml outline`
Current: an agent orients with `inspect` plus two or three family reads.
Target: `ooxml outline <file>` returns one compact, family-aware tree in a
single call: pptx (slides, layout, title, shape selectors and handles, text
preview, charts, tables, images, notes flag), xlsx (sheets, used range,
tables, names, charts, pivots, data validations, freeze), docx (blocks with
kind, style, text preview, hash, tables, images, headers and footers, fields).
Depth and text preview length are flags; output is deterministic and pinned.
This is the read half of the build specs in C2: the outline of a built file is
the same shape as the spec, so an agent can read, modify, and rebuild.
Complexity: M.

#### B8 One proof call: `ooxml check`
Current: proof is five separate commands (validate, conformance, SDK, layout
QA, render), each with its own JSON.
Target: `ooxml check <file>` runs strict validation, conformance, the Open
XML SDK validator when available, layout QA and design lint for pptx, style
integrity for docx, formula and reference integrity for xlsx, and optionally
`--render`. It returns one envelope with a `proofLevel` (structural, strict,
schema, visual) and a `findings` list where every finding has a severity and
a `fixCommand`. Every mutation readback names `ooxml check` as its
`checkCommand`. This is the mega-command that makes the agent loop
"edit, check, fix" a two-step loop.
Complexity: M.

#### B6 Recipes inside the tool
Current: `capabilities.workflows` has two entries with zero steps;
`robot-docs guide` is prose.
Target: `capabilities --workflows` and `robot-docs recipe <name>` return
runnable, ordered command sequences for: deck from scratch, deck from
template, workbook report, document report, macro workbook, find and replace
across a package, translate a deck. Each step carries the exact command, the
expected readback fields, and the proof command. `agent-triage` links to
them. The SKILL.md and README embed the same recipes, generated from the
binary by a test so they cannot drift.
Complexity: M.

### Track C: One call, one intent (P1)

#### C1 Every mutation is batchable
Current: 70 of 152 mutating commands are `opCompatible`.
Target: all 152 are dispatchable through `apply --ops`, serve `op`, and MCP
`op`. Ops may reference results of earlier ops in the same batch with
`$ref` (for example the slide number returned by a `new-slide` op). The
batch validates once at the end, stages atomically through the existing
mutation seam, and returns per-op readback. `--dry-run` returns the resolved
plan with all `$ref` substitutions. The `serve_dispatches_every_op_compatible`
guard test extends to the full set.
Complexity: L.

#### C2 Declarative build specs
Current: an agent must issue ten to thirty processes to author a small deck.
Target: `pptx build`, `xlsx build`, `docx build` take a JSON spec
(`--spec file.json` or stdin) and produce a complete package in one process:
- pptx: theme or template, slides with layout, title, subtitle, bullets with
  levels, notes, images, tables, charts, text boxes, speaker notes, section
  headers, footers and slide numbers.
- xlsx: sheets, header rows, data (JSON or CSV file refs), typed columns,
  tables with styles, number formats, conditional formats, freeze panes,
  column widths or autofit, charts, defined names, data validations,
  workbook metadata.
- docx: title, headings, paragraphs, bullet and numbered lists, tables with
  header rows and styles, images with captions, page breaks, headers and
  footers, table of contents field, sections with orientation and margins,
  core properties.
The spec schema is published by `capabilities --schema pptx-build` (JSON
Schema, versioned) and by MCP. Build composes the existing mutation engine
through C1 so there is one XML writer per feature. Readback is the full
document outline with selectors and handles for follow-up edits.
Complexity: XL (split into per-family beads plus a shared spec core).

#### C3 Markdown in, Office out
Current: none.
Target: `docx build --from-markdown report.md` and
`pptx build --from-markdown deck.md` convert CommonMark plus a small
front-matter block into the C2 spec, then build. Slides split on `---` or
level-1 headings; bullets, numbered lists, tables, images, code blocks,
emphasis, and links map to native constructs; front matter selects theme,
template, footer, and page setup. Markdown is what agents write natively, so
this is the shortest path from intent to file. Round trip `docx text` and
`pptx extract text` output stays readable as the source.
Complexity: L.

#### C4 Typed MCP tools
Current: 7 generic tools; the agent must know command words and flags.
Target: MCP `tools/list` exposes one typed tool per build spec
(`build_presentation`, `build_workbook`, `build_document`) with the JSON
Schema from C2, plus `render_preview`, `design_check`, `validate_package`,
and `edit_package` (batch ops from C1). Generic tools remain. Tool schemas are
generated from the manifest and pinned by a golden.
Complexity: M.

#### C5 Read as Markdown
Current: `docx text` and `pptx extract text` return JSON of raw text.
Target: `--format markdown` on `docx text`, `pptx extract text`, and
`xlsx ranges export` produces CommonMark that preserves headings, lists,
tables, emphasis, links, image references, and slide separators, using the
same mapping as C3 in reverse. Round trip tests: markdown to docx to markdown
is stable for the supported subset. Agents can then read an Office file the
same way they write one.
Complexity: M.

### Track D: Beautiful by default (P1 to P2)

#### D1 Real PPTX scaffold
Current: one layout, one master, minimal theme, 4:3 default check needed.
Target: `pptx scaffold` produces a 16:9 deck with a complete master, the
eleven standard layouts (Title Slide, Title and Content, Section Header, Two
Content, Comparison, Title Only, Blank, Content with Caption, Picture with
Caption, Title and Vertical Text, Vertical Title and Text), placeholder
geometry on a consistent grid, master text styles with five bullet levels
and sizes, theme fonts with fallbacks, and a named color scheme.
`--theme <name>` selects one of at least four built-in schemes (neutral,
corporate blue, warm, dark) defined once as data; `--template deck.pptx`
imports masters and layouts from an existing deck through the existing
`masters import` path. `--size 16:9|4:3|A4`. All output byte-deterministic.
Typography follows a modular scale (title 40 pt, section 32, heading 28,
body 20, 18, 16, 14 for levels 1 to 5) with line spacing derived from the
font metrics in D10, so text never touches placeholder edges at the default
sizes. `--theme-seed <hex>` derives the six accent colors and the dark and
light pairs in OKLCH space from one brand color, with every text and
background pairing guaranteed at least WCAG AA contrast; the derivation is a
pure function with unit tests and the palette is written into the theme so
desktop Office shows the same colors.
Success: `new-slide-from-layout "Title and Content"` works on a scaffold;
render shows no overlaps; SDK clean; LibreOffice and Office open clean.
Complexity: L.

#### D2 Bullets and rich paragraphs
Current: multi-line text becomes one paragraph.
Target: `--text` with newlines produces paragraphs; `--bullets` and
`--paragraphs-file` (JSON array of `{text, level, bullet, bold, ...}`) produce
leveled bullet lists in placeholders and text boxes; `pptx text set` and
`new-slide-from-layout --set-text` share the paragraph builder; readback
returns paragraphs with levels.
Complexity: M.

#### D3 Layout QA that sees inherited geometry
Current: placeholders without explicit `xfrm` have null bounds, so
`validate-layout` misses the most common overlap.
Target: resolve placeholder bounds through layout then master by placeholder
type and index (the resolver already exists for shapes show; reuse it).
Report collisions, overflow, off-slide, and safe-margin violations with
severity and a suggested fix command (`shapes set-bounds`, font size). Add
`--fix auto` for the safe subset (shrink font within bounds, nudge inside
margins). `validate-layout` is included in every pptx mutation's
`layoutCheckCommand`.
Complexity: M.

#### D4 Semantic placement and units
Current: all placement is raw EMUs, which agents get wrong.
Target: every `--x --y --cx --cy` accepts units (`in`, `cm`, `pt`, `px`,
`%` of slide) and a `--slot` vocabulary computed from the slide size and
layout body area: `body`, `left-half`, `right-half`, `top-half`,
`bottom-half`, `grid:RxC:i`, `caption`, `full-bleed`. Readback reports both
EMUs and inches. Same unit parsing for docx image sizes and xlsx column
widths.
`pptx slides compose --slide n --items <json>` places several items (text,
image, chart, table) in one call using a flex-like layout: `--arrangement
row|column|grid:RxC`, `--gutter`, `--padding`, and per-item `grow` weights.
The engine computes bounds from the layout's body area and returns them, so
the agent never types an EMU for a multi-item slide.
Complexity: M.

#### D5 DOCX scaffold with a real style set
Current: no styles part; dangling style references are silent.
Target: `docx scaffold` writes `styles.xml` (Normal, Title, Subtitle,
Heading 1 to 4, List Bullet, List Number, Quote, Caption, Table Grid, Table
Light), `numbering.xml` with bullet and decimal definitions, `settings.xml`,
`fontTable.xml`, a theme, and core properties; `--template file.docx` inherits
styles and page setup from an existing document. Style references are
validated: applying an unknown style fails with the list of available styles
unless `--create-style` supplies a built-in definition. `docx blocks` and
`docx text` readback include `styleId` and list level.
Complexity: L.

#### D6 DOCX structure commands
Current: no lists, page breaks, sections, or table styles.
Target: `docx paragraphs append --list bullet|number --level n`,
`docx breaks insert --page`, `docx sections set --orientation --margins`,
`docx tables create --style --header-row --widths`, `docx tables set-style`,
`docx fields insert --toc` (TOC field with `updateFields` on open),
`docx images insert --caption`. All batchable and part of the docx build spec.
Complexity: L.

#### D7 XLSX polish
Current: pieces exist but no defaults for a good-looking report.
Target: `xlsx colwidths autofit` (content-based estimate using character
widths), `xlsx ranges set-style --preset header|total|band|input`,
`xlsx tables create --header-style`, `xlsx sheets set-tab-color`, print setup
(`xlsx sheets set-print --fit-to-width --landscape --repeat-header`), chart
styling parity with pptx charts, and `xlsx scaffold --theme`. Total rows and
banded rows are one flag away.
Complexity: M.

#### D8 Design lint
Current: none.
Target: `ooxml design-check <file>` for all three families reports objective
issues with fix commands: text contrast below WCAG AA (computed in OKLCH from
theme colors), font size below 12 pt on slides, more than seven bullets or
two levels on a slide, inconsistent fonts, empty placeholders, images
stretched beyond native resolution, tables wider than the page, missing alt
text, missing slide titles, column widths clipping numbers. Findings are
JSON with severity and a `fixCommand`. Included in the build readback.
Complexity: L.

#### D9 Preview for every family
Current: only `pptx render`.
Target: `ooxml render <file>` for xlsx and docx through LibreOffice (PDF plus
PNG per page), the same JSON shape as pptx render, and `diff --render` for
all families. Renders are used by the recipe tests and by `design-check
--visual` for overflow confirmation.
Complexity: S.

#### D10 Text metrics for real overflow estimates
Current: overflow is estimated from a fixed line height.
Target: embed average-width tables for the built-in theme fonts (Aptos,
Calibri, Segoe UI, Arial, Liberation equivalents) and compute wrapped line
counts per paragraph, respecting autofit settings and bullet indents. Used
by D3 and D8. Calibrated against LibreOffice renders in tests.
Complexity: M.

#### D11 Brand kit across families
Current: `template tokens/profile/apply` exist for pptx and xlsx themes.
Target: a `brand.json` (colors, fonts, logo path, footer text, slide number
policy, page setup) accepted by every scaffold and build command through
`--brand`, and by `template apply --brand` for existing files of all three
families. The docx theme and styles, the xlsx theme and table styles, and the
pptx theme and masters are all derived from the same kit, so a report, a deck,
and a workbook produced in one session look like one family. `brand.json`
has a published schema and a `template brand extract <file>` command that
derives a kit from an existing Office file.
Complexity: M.

#### D12 Chart defaults that look right
Current: charts are created with library defaults.
Target: chart creation for both families applies a house style by default:
theme accent palette in order, no 3D, no gridline clutter, axis number
formats inferred from the source cells (currency, percent, integer), sensible
category axis label rotation, legend placement by series count, optional
data labels, and a title that defaults to the source header. `--style
minimal|default|dense` and `charts copy-style` cover the rest. Rendered
comparison fixtures pin the visual outcome.
Complexity: M.

#### D13 Image pipeline
Current: images are embedded as given.
Target: on insert, images are checked for format, EXIF orientation is
honored, oversized images are downsampled to a `--max-dpi` (default 220) with
the original kept when `--keep-original`, alt text is settable and required
by design-check, and `--fit` respects the slot from D4. Same pipeline for
pptx, docx, and xlsx drawings.
Complexity: M.

### Track E: Proof discipline (P1 to P2)

#### E1 From-scratch recipes as e2e tests
Target: three Linux e2e tests build the "Q3 review" deck, the sales
workbook, and the quarterly report document from specs, then assert strict
validation, conformance, Open XML SDK clean, layout QA with zero issues,
design-check with zero errors, LibreOffice render succeeds, and a frozen
semantic summary golden. Logs are detailed and structured.
Complexity: M.

#### E2 Every mutating command has a proof row
Target: the artifact proof matrix reaches zero structural, readback,
validate, and conformance gaps on Linux; the Windows smoke matrix covers every
mutating command; `-FailOnGap` becomes a CI gate.
Complexity: L.

#### E3 Determinism and env conventions
Target: a test builds each family twice and asserts identical bytes; any
future timestamp writer honors `SOURCE_DATE_EPOCH`; `NO_COLOR`, `CI`, and
non-TTY discipline are verified by a test for the few text-mode commands.
Complexity: S.

#### E5 Performance budgets
Target: build specs and `ranges set` stream large data: 200,000 cells in
under 5 seconds and under 300 MB resident on the CI runner; a 60-slide deck
build under 3 seconds; `outline` on a 50 MB workbook under 2 seconds. Budget
tests run in CI with timing logged and fail on regression beyond 25 percent.
Complexity: M.

#### E6 Hardening the new input surfaces
Target: the spec parser, markdown converter, brand kit loader, and ops
`$ref` resolver get fuzz seeds and adversarial tests: path traversal in file
references, zip bombs and decompression ceilings on input packages, oversized
images, deeply nested markdown, hostile style ids, and reference cycles in
`$ref`. All failures are clean errors with exit codes, never panics.
Complexity: M.

#### E7 Property tests: the builder never emits invalid XML
Target: a property-based test generates random valid specs for each family,
builds them, and asserts strict validation, conformance, and (when the SDK is
present) schema validity. Shrinking reports the smallest failing spec. This
is the strongest guard against the class of bug behind A1.
Complexity: M.

#### E4 Office proof rerun and record
Target: after A1, rerun the Legion edit and VBA lanes plus the recipe outputs,
record results in the release-readiness doc, and only then tag.
Complexity: S (Windows machine required).

### Track F: Repo hygiene for agents (P3)

#### F1 Retire the Go tree
Target: tag `go-reference-final` at the current commit, remove `go/` and the
17 Go Makefile targets from master, and point docs at the tag. This needs the
user's explicit go-ahead because it deletes history from the working tree.
Complexity: S.

#### F2 Truthful docs and AGENTS.md
Target: root help summary says what the tool is; README states release
status factually; the status doc keeps a short current section and moves the
chronology to `docs/history/`; an `AGENTS.md` at the root summarises the work
rules from GOAL.md, the proof ladder, the toolchain, and points at
`skills/ooxml/SKILL.md`.
Complexity: S.

#### F3 Existing open beads
`ooxml-y7v` (web model smoke) stays open and is unblocked only by
credentials. `ooxml-dedup-opc-a1-helpers-y3k` stays open and is a
prerequisite for A1 (relationship XML helpers) and A7.

## 4. Dependency graph

```
A2 clippy ─┐
A4 CI artifacts ─┤
A5 doctor ───────┼─→ A6 Linux SDK gate ─→ E1 recipes ─→ E2 proof rows ─→ A8 release
y3k helpers ─→ A1 pivot ─→ A7 child-order validator ─┘         ↑
A3 rebuild ─────────────────────────────────────────────────────┘

B1 errors teach ─→ B2 aliases ─→ B3 verbs
B5 envelope ─→ C1 batch everything ─→ C2 build specs ─→ C3 markdown ─→ C4 MCP typed
B4 guards ─┘          B7 outline ─────────┘    │           C5 read as markdown
                                               └─→ E7 property tests
D1 pptx scaffold ─→ D2 bullets ─→ D3 layout QA ─→ D8 design lint ─→ B8 check
D4 units, slots, compose ─┘         ↑            ↑
D5 docx scaffold ─→ D6 docx structure            │
D7 xlsx polish ──────────────────────┘           │
D9 render all ─→ D10 text metrics ───────────────┘
D11 brand kit depends on D1, D5, D7; D12 chart defaults on D1; D13 images on D4
B6 recipes depends on C2, D1, D5, D7, B8
E3 determinism, E5 perf, E6 hardening follow C2
F1, F2 independent; F1 needs user approval
```

## 5. Ordering

1. Track A first, in parallel lanes: A2, A4, A5 (small) with A1 and A3
   (medium). A7 follows A1. A6 follows A5. A8 last.
2. B1 and B5 next; they unlock the intent corpus and the envelope contract
   that every later command must satisfy.
3. D1 and D5 next; without real scaffolds no from-scratch recipe can be good.
4. C1 then C2, with D2, D4, D6, D7 landing as build-spec features.
5. D3, D9, D10, D8 for quality proof; C3 and C4 for the agent surfaces.
6. E1 and E2 gate the tag; F1 and F2 when convenient.

## 6. Verification plan

- Every implementation bead has a companion test bead with unit tests,
  contract tests through the CLI boundary, and structured logging.
- `make check-ci` green on stable; Ubuntu CI runs the Open XML SDK validator.
- The three recipe e2e tests pass with zero layout and design findings.
- Windows smoke 64 of 64 plus new scenarios; Office COM proof recorded.
- `bv --robot-insights` reports no cycles; `br dep cycles` empty.
