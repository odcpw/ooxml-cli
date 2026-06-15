# Rich Text Fixtures

This directory contains PPTX fixtures with rich text formatting for testing text extraction and formatting preservation.

Created: 2026-03-11T08:19:29.913145

## Fixtures

### rich-formatting/presentation.pptx
- **Description**: Mixed formatting (bold, italic, colors) in title and content slides
- **Slides**: 2
- **Characteristics**:
  - Title slide with mixed formatting in subtitle (bold, italic, colored text)
  - Content slide with bullet points at different levels with various formatting
  - Tests extraction of run-level formatting properties (bold, italic, RGB color)

### rich-alignment/presentation.pptx
- **Description**: Different paragraph alignments and spacing variants
- **Slides**: 2
- **Characteristics**:
  - Slide 1: Left, center, right, and justify aligned paragraphs
  - Slide 2: Different font sizes (12pt to 28pt) and colors
  - Tests extraction of paragraph-level properties (alignment, font size, color)

### rich-bodypr/presentation.pptx
- **Description**: Different text body properties (anchoring, margins, word wrap)
- **Slides**: 2
- **Characteristics**:
  - Various vertical anchor settings (top, middle, bottom)
  - Different margin configurations (left, top, right, bottom)
  - Word wrap variants
  - Tests extraction of bodyPr-level properties (anchor, margins, word wrap)

### rich-numbered-lists/presentation.pptx
- **Description**: Numbered and mixed lists with multi-level formatting
- **Slides**: 2
- **Characteristics**:
  - Numbered list with multiple levels
  - Mixed bullet/number formatting with bold, italic, and colored text
  - Tests extraction of bullet/numbering properties and level hierarchy

## Testing Instructions

Test extract text command:

```bash
# Text output
./ooxml pptx extract text testdata/pptx/rich-formatting/presentation.pptx

# JSON output
./ooxml pptx extract text testdata/pptx/rich-formatting/presentation.pptx --format json

# With --rich flag (when implemented)
./ooxml pptx extract text testdata/pptx/rich-formatting/presentation.pptx --rich --format json

# Verify fixtures open in LibreOffice
libreoffice testdata/pptx/rich-formatting/presentation.pptx
libreoffice testdata/pptx/rich-alignment/presentation.pptx
libreoffice testdata/pptx/rich-bodypr/presentation.pptx
libreoffice testdata/pptx/rich-numbered-lists/presentation.pptx
```

## Property Extraction Coverage

### Run Properties (a:rPr)
- [x] Bold (a:rPr/@b)
- [x] Italic (a:rPr/@i)
- [x] Font size (a:rPr/@sz)
- [x] Font color (a:solidFill/a:srgbClr)
- [ ] Font name (a:rPr/@latin typeface)
- [ ] Underline (a:rPr/@u)
- [ ] Language (a:rPr/@lang)

### Paragraph Properties (a:pPr)
- [x] Alignment (a:pPr/@algn)
- [ ] Level (a:pPr/@lvl)
- [ ] Bullet character (a:buChar/@char)
- [ ] Numbering format (a:buAutoNum/@type)
- [ ] Spacing before/after (a:spcBef/a:spcAft)
- [ ] Line spacing (a:lnSpc)

### Body Properties (a:bodyPr)
- [x] Vertical anchor (a:bodyPr/@anchor)
- [x] Margin left (a:bodyPr/@lIns)
- [x] Margin top (a:bodyPr/@tIns)
- [x] Margin right (a:bodyPr/@rIns)
- [x] Margin bottom (a:bodyPr/@bIns)
- [x] Word wrap (a:bodyPr/@wrap)

## Running Tests

Generate/regenerate fixtures:
```bash
python3 testdata/generate/python/create_rich_text_fixtures.py
```

Run extract text tests:
```bash
go test ./internal/cli -run TestExtractText
go test ./internal/cli -run TestExtractTextRich
```

## Known Formatting Limitations

Some formatting options available in the PPTX format may not be fully captured by python-pptx:
- Some bullet/numbering options are limited
- Complex spacing configurations may vary
- Some color formats (theme colors, gradients) may not be available in python-pptx

See fixtures in LibreOffice Impress to verify actual formatting.
