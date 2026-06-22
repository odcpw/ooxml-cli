# LibreOffice Chart Presentation Fixture

Generated on Linux with local LibreOffice headless export:

```bash
soffice --headless --convert-to pptx \
  --outdir testdata/pptx/libreoffice-chart-simple \
  testdata/pptx/chart-simple/presentation.pptx
```

Source fixture: `testdata/pptx/chart-simple/presentation.pptx`

Validation at creation:

```bash
ooxml --json validate --strict testdata/pptx/libreoffice-chart-simple/presentation.pptx
ooxml --json pptx charts list testdata/pptx/libreoffice-chart-simple/presentation.pptx
```

This fixture is committed to catch producer-exported chart package behavior.
It is LibreOffice export evidence, not Microsoft Office COM proof.
