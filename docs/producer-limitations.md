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
