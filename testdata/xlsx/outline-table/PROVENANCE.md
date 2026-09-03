# Outline table fixture provenance

Generated through the validated mutation seam:

```sh
ooxml --json xlsx tables create testdata/xlsx/chart-workbook/workbook.xlsx --sheet Data --range A1:B4 --table Sales --out testdata/xlsx/outline-table/workbook.xlsx
```

SHA-256: `43c926ed9db7047ab0e35147611f76efa0a1b478b7e9925b638326fc41d36bf7`.
The reproduction assertion is in `tests/outline.rs`.
