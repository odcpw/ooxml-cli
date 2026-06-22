# LibreOffice Chart Workbook Fixture

Generated on Linux with local LibreOffice headless export:

```bash
soffice --headless --convert-to xlsx \
  --outdir testdata/xlsx/libreoffice-chart-workbook \
  testdata/xlsx/chart-workbook/workbook.xlsx
```

Source fixture: `testdata/xlsx/chart-workbook/workbook.xlsx`

Validation at creation:

```bash
ooxml --json validate --strict testdata/xlsx/libreoffice-chart-workbook/workbook.xlsx
ooxml --json xlsx charts list testdata/xlsx/libreoffice-chart-workbook/workbook.xlsx
```

This fixture is committed to catch producer-exported chart package behavior.
It is LibreOffice export evidence, not Microsoft Office COM proof.
