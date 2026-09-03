# Outline defined-name fixture provenance

Generated through the validated mutation seam:

```sh
ooxml --json xlsx names add testdata/xlsx/chart-workbook/workbook.xlsx --name DataRange --ref 'Data!$A$1:$B$4' --out testdata/xlsx/outline-names/workbook.xlsx
```

SHA-256: `88becc483d791e840d16e68bb8821fef0d923849c1b0a856b7383caf0ca94677`.
The reproduction assertion is in `tests/outline.rs`.
