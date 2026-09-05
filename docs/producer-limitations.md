# Producer schema limitations

`check` reports these known schema defects even when the Open XML SDK is not
installed. They are independent of structural validation. A strict structural
pass does not make an SDK-invalid input schema-clean. Findings identify the
part and element and link to an SDK verification command; that command verifies,
it does not repair.

## XLSX chart styles exported by LibreOffice

The original `testdata/corpus/libreoffice/sales.xlsx` has two chart-style defects:

- `XLSX_CHART_STYLE_COLOR_NAMESPACE`: `fontRef` contains `schemeClr` in the
  chart-style namespace instead of the DrawingML namespace.
- `XLSX_CHART_STYLE_MARKER_LAYOUT_CHILD`: `dataPointMarkerLayout` is a leaf
  element but contains style-reference children.

Ordinary cell edits preserve this unrelated part. Automatic repair is unsupported:
removing marker-layout children discards formatting, and a lossless translation
has not been established. Re-export with a producer that emits schema-valid chart
styles, then run the finding's `conformance check --openxml-sdk` command. Inspect
rendering and obtain desktop Office evidence separately if compatibility matters.
The original corpus export remains unchanged as a regression fixture.

## DOCX table and numbering justification exported by LibreOffice

`DOCX_JUSTIFICATION_PRODUCER_VALUE` identifies `start` or `end` used as table
(`tblPr`/`tblPrEx` → `jc`) or numbering-level (`lvl` → `lvlJc`) justification.
Those values are invalid for these contexts in the SDK Office2019 schema.
Paragraph justification permits these logical directions and is not flagged.
Element and attribute namespaces are resolved rather than assuming a `w` prefix.

The original `testdata/corpus/libreoffice/report.docx` contains one invalid table
justification and 63 invalid numbering-level justifications. Automatic replacement
with left/right is unsupported because it requires the intended writing direction.
Set the intended alignment in the source application and re-export, then run
`conformance check --openxml-sdk`. Ordinary paragraph edits preserve these values
and unrelated package parts. The same input also has seven independent font
charset schema errors, tracked in `ooxml-us9`; resolving justification alone does
not establish that the package is schema-clean.
