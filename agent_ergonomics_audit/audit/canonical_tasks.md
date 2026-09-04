# Pass 6 canonical authoring tasks

These tasks are outcome statements for a fresh agent. The agent may read only
`README.md`, `skills/ooxml/SKILL.md`, `ooxml --help`, and
`ooxml --json capabilities` before choosing commands. It may not inspect source,
tests, prior audit artifacts, or a prewritten command sequence.

## Task 01: branded-deck-from-spec

**Statement.** Given a valid PPTX build specification that references a brand
kit, build a five-slide branded review deck and prove that the published package
is structurally, strictly, and Open XML SDK valid.

**Tags.** mutating, build-spec, pptx, brand, proof

**Expected outcome.** A new `.pptx` exists; the build reports validation and
zero check errors; strict and SDK proof both pass.

**Documented in.** The generated `build-from-spec` recipe and the `pptx build`
entry in capabilities.

**Post-pass target.** One build invocation plus one proof invocation.

## Task 02: workbook-with-chart

**Statement.** Given a valid XLSX build specification containing typed data, a
table, formatting, and a chart, build the workbook and prove the published
package.

**Tags.** mutating, build-spec, xlsx, chart, proof

**Expected outcome.** A new `.xlsx` exists; the build reports validation and
zero check errors; strict and SDK proof both pass.

**Documented in.** The generated `workbook-report` and `build-from-spec`
recipes and the `xlsx build` entry in capabilities.

**Post-pass target.** One build invocation plus one proof invocation.

## Task 03: report-document

**Statement.** Given a valid DOCX build specification containing headings,
rich paragraphs, lists, a table, an image, headers, and footers, build the
report and prove the published package.

**Tags.** mutating, build-spec, docx, rich-content, proof

**Expected outcome.** A new `.docx` exists; the build reports validation and
zero check errors; strict and SDK proof both pass.

**Documented in.** The generated `document-report` and `build-from-spec`
recipes and the `docx build` entry in capabilities.

**Post-pass target.** One build invocation plus one proof invocation.

## Task 04: markdown-sourced-deck

**Statement.** Given Markdown in the supported presentation profile, build a
deck from it and prove the published package without first authoring JSON.

**Tags.** mutating, markdown, pptx, conversion, proof

**Expected outcome.** A new `.pptx` exists; the build reports its Markdown
source, validation, and zero check errors; strict and SDK proof both pass.

**Documented in.** The generated `build-from-markdown` recipe and the
`pptx build --from-markdown` flags in capabilities.

**Post-pass target.** One build invocation plus one proof invocation.

## Task 05: edit-and-check-deck

**Statement.** Replace the title text on the branded deck using the natural
PPTX replace vocabulary, publish to a new path, then run the one-call package
check with strict and SDK proof enabled.

**Tags.** mutating, pptx, edit, readback, proof

**Expected outcome.** A new edited `.pptx` exists, the replacement reports a
real applied mutation, and the check reports zero errors with strict and SDK
proof passed.

**Documented in.** The capabilities `pptx inspect then edit` workflow and the
`pptx replace text` and `check` command entries.

**Post-pass target.** One edit invocation plus one proof invocation.

## Task 06: design-check-and-render-deck

**Statement.** Run the objective design review on the edited deck and render
the deck to page images using the locally advertised renderer.

**Tags.** read-only, pptx, design-check, render, visual-proof

**Expected outcome.** Design check exits successfully with zero errors; render
exits successfully and produces one image for every slide.

**Documented in.** The `design-check` and `render` command entries in
capabilities and their examples in the generated agent documentation.

**Post-pass target.** One design-check invocation plus one render invocation.
