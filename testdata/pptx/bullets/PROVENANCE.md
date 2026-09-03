# PPTX bullet hierarchy render fixture

Generated from `testdata/pptx/multi-layout/presentation.pptx` with the Rust CLI:

```text
ooxml --json pptx new-slide-from-layout \
  testdata/pptx/multi-layout/presentation.pptx \
  --layout "Title and Content" \
  --set-text "title=Bullet hierarchy" \
  --set-text "body=- First level item one\n- First level item two\n- First level item three\n\t- Second level item one\n\t* Second level item two" \
  --out testdata/pptx/bullets/presentation.pptx
```

SHA-256: `53b64c2a198dcc07a01176abf34bbb9976aada4154957fbe0fc143e7f7cd720f`

Proof on 2026-09-03:

- `ooxml --json --strict validate`: valid.
- Open XML SDK validator: 0 errors.
- LibreOffice rendered slide 5 to PNG; visual inspection showed three first-level bullet characters and two indented second-level dash characters. This is render evidence, not a desktop Microsoft Office compatibility claim.
