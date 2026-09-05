# Brand application by family

The scope is `template apply --brand` on supported packages with an existing
Office theme, and the same brand kit used by scaffolds. `proven` denotes the
bounded behavior below, with executable package readback and strict validation;
it does not mean desktop Office compatibility. SDK and LibreOffice evidence are
recorded separately when those tools are available.

| Brand element | PPTX | DOCX | XLSX |
| --- | --- | --- | --- |
| Theme colors | proven | proven | proven |
| Heading and body fonts | proven | proven | proven |
| Logo placement | proven | proven | proven |
| Chart palette | proven | partial | proven |
| Header/footer marks | proven | proven | proven |
| Page/slide size defaults | proven | proven | proven |
| Table styles | partial | proven | proven |

Evidence: `tests/brand_kit.rs` checks three-family theme signatures, font
readback, dimensions, page margins, footer marks, image relationships,
determinism, and existing-package chart/table application.
`tests/brand_chart_image.rs` additionally exercises theme-driven charts and
optional LibreOffice rendering. The companion brand-parity suite checks this
table against package observations and rebuilds the five canonical recipes.

- Colors and fonts update theme parts. DOCX named heading styles and XLSX cell
  fonts are updated as well. Font installation and fallback selection belong to
  the rendering environment; arbitrary directly formatted text is not restyled.
- PPTX logos are placed on each slide. XLSX logos use a drawing anchor on the
  first worksheet. DOCX top placements use the default header of the last
  section; bottom placements use its default footer. Existing body content and
  footer text survive logo insertion. First/even-page and independently linked
  sections are outside that placement profile.
- Existing chart series receive successive accent colors, cycling after six.
  Source formulas, caches, axes, and series counts are retained. DOCX uses the
  same chart writer, but has no canonical authored-chart fixture, so its chart
  cell remains partial rather than borrowing another family's proof.
- `footerText` supplies the footer mark; the kit has no independent header-text
  field. XLSX marks are print headers/footers and are visible in page output.
- Page defaults cover the kit's paper size, orientation and margins; PPTX uses
  its slide-size setting. Existing content is not reflowed by the CLI.
- XLSX table styles select the named workbook style. DOCX selects an existing
  table style, or defines a named style using the brand accent for borders and
  the first-row shading. An existing non-table style with the same id is
  refused. PPTX table-style selection is not implemented by the brand kit.

`template brand extract` extracts theme colors and heading/body fonts in every
family. Logo paths, footer marks, page settings and table-style defaults are not
recovered by extraction; an extracted kit is a theme kit, not a full round trip
of all application settings. No command-manifest rows change in this work.
