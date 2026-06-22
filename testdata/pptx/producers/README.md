# Producer Variance Fixtures

This directory contains PPTX fixtures created by different producers to test parser variance handling.

Created: 2026-03-11T08:19:30.264010

## Fixtures

### powerpoint/presentation.pptx
- **Producer**: simulated PowerPoint output (created with python-pptx 0.6.23)
- **Structure**: Standard 2-slide presentation (Title Slide + Title and Content layouts)
- **Slides**: 2
- **Characteristics**:
  - Uses standard placeholder shapes
  - Standard namespace declarations
  - All optional XML elements present
  - Mimics typical PowerPoint export format

### libreoffice/presentation.pptx
- **Producer**: simulated LibreOffice Impress output (created with python-pptx 0.6.23, modified)
- **Structure**: Standard 2-slide presentation (Title Slide + Title and Content layouts)
- **Slides**: 2
- **Characteristics**:
  - Uses standard placeholder shapes
  - May have sparse XML structure (optional elements removed)
  - Missing p:nvPr elements on shapes
  - May have minimal namespace declarations
  - Mimics LibreOffice's export format

### google-slides/presentation.pptx
- **Producer**: simulated Google Slides output (created with python-pptx 0.6.23)
- **Structure**: 2-slide presentation with blank layouts using text boxes
- **Slides**: 2
- **Characteristics**:
  - Uses text boxes instead of placeholders (no p:ph elements)
  - Blank layouts to simulate Google Slides' approach
  - Should trigger shape:<id> key fallback in parser
  - Tests parser's ability to handle missing placeholder metadata

### python-pptx/presentation.pptx
- **Producer**: python-pptx 0.6.23
- **Source**: Symlink to testdata/pptx/minimal-title/presentation.pptx
- **Structure**: Single-slide presentation (Title Slide layout)
- **Slides**: 1
- **Characteristics**:
  - Standard python-pptx output
  - Basic placeholder structure
  - Reference fixture for python-pptx library output

## Testing Instructions

Test all M2 commands against these fixtures:

```bash
ooxml --json pptx masters list testdata/pptx/producers/<producer>/presentation.pptx
ooxml --json pptx masters show testdata/pptx/producers/<producer>/presentation.pptx --master <selector>
ooxml --json pptx layouts list testdata/pptx/producers/<producer>/presentation.pptx
ooxml --json pptx layouts show testdata/pptx/producers/<producer>/presentation.pptx --layout <selector>
ooxml --json pptx slides list testdata/pptx/producers/<producer>/presentation.pptx
ooxml --json pptx slides show testdata/pptx/producers/<producer>/presentation.pptx --slide <n>
```

Each fixture has a corresponding golden JSON file for regression testing.

## Known Parser Variance Patterns

1. **Placeholder vs Text Box**: Google Slides fixture uses text boxes (no p:ph element). Parser should fall back to shape:<id> keys.
2. **Optional XML Elements**: LibreOffice fixture has sparse XML (missing p:nvPr). Parser should treat as absent, not error.
3. **Namespace Declarations**: Different producers use different namespace declarations. Parser should handle any valid namespace prefix.
4. **Attribute Ordering**: Different producers may order XML attributes differently. Parser should ignore order.
5. **Unknown Elements**: PowerPoint may use mc:AlternateContent wrappers. Parser should preserve unknown elements.

## Running Tests

```bash
cargo test --test rust_contract_smoke pptx_
```
