# Test Fixture Generation Pipeline

This directory contains Python scripts that generate PPTX test fixtures used by ooxml-cli test suites.

## Overview

All fixtures are generated using python-pptx and committed to `testdata/pptx/` to ensure reproducible tests. The corrupted fixtures are created by python-pptx with post-processing to deliberately break specific aspects of the PPTX structure.

## Setup

```bash
pip install -r requirements.txt
```

## Fixture Generators

### Normal Fixtures

These are valid PPTX files suitable for testing core inspection and manipulation features (M0–M2+).

- **minimal-title.py** — Minimal valid PPTX with a single title slide. Tests basic structure parsing.
  - Output: `../pptx/minimal-title/presentation.pptx`
  - Layout: 1 slide (Title Slide layout)

- **title-content.py** — Title slide + content layout slide. Tests multi-slide decks and content placeholders.
  - Output: `../pptx/title-content/presentation.pptx`
  - Layouts: Title Slide + Title and Content

- **picture-placeholder.py** — Slide with embedded picture. Tests media relationship handling.
  - Output: `../pptx/picture-placeholder/presentation.pptx`
  - Content: Embedded test image

- **table-slide.py** — Slide with a 3x3 table. Tests table parsing and cell access.
  - Output: `../pptx/table-slide/presentation.pptx`
  - Content: 3x3 table with sample data

- **chart_simple.py** — Two-slide deck with normal embedded-workbook column charts.
  - Output: `../pptx/chart-simple/presentation.pptx`
  - Content: Two clustered column charts with stored caches and embedded XLSX workbook parts

- **notes-slide.py** — Slide with speaker notes. Tests notes relationship handling.
  - Output: `../pptx/notes-slide/presentation.pptx`
  - Content: Speaker notes on a content slide

- **multi-layout.py** — Deck using at least 3 distinct layout names (Title Slide, Title and Content, Section Header, Blank). Tests layout enumeration.
  - Output: `../pptx/multi-layout/presentation.pptx`
  - Constraint: Must contain at least two distinct layout names
  - Layouts: Title Slide, Title and Content, Section Header, Blank

- **notes-handout.py** — Slides with both speaker notes and handout content. Tests notes + handout relationship handling.
  - Output: `../pptx/notes-handout/presentation.pptx`
  - Content: Multiple slides with speaker notes

### Supplemental Fixture Generators

These scripts generate groups of fixtures or supporting docs across multiple fixture directories.

- **create_geometry_fixtures.py** — Generates image geometry fixtures with known rotation, flip, and crop values.
  - Output: `../pptx/geometry/*/presentation.pptx`
- **create_rich_text_fixtures.py** — Generates rich text fixtures plus `testdata/pptx/README_RICH_TEXT.md`.
  - Output: `../pptx/rich-*/presentation.pptx`, `../pptx/README_RICH_TEXT.md`
- **create_producer_fixtures.py** — Generates/refreshes producer-variance fixtures and their README.
  - Output: `../pptx/producers/*`, `../pptx/producers/README.md`

### Slide Assembly Fixtures (M13)

These fixtures test slide assembly operations (delete, move, import, merge).

- **slide-assembly-multi.py** — Multi-slide deck with varied layouts for delete/move/reorder testing.
  - Output: `../pptx/slide-assembly-multi/presentation.pptx`
  - Slides: 5 slides with different layouts
  - Layouts: Title Slide, Title and Content (2x), Section Header, Blank
  - Purpose: Exercise multiple layouts and provide targets for assembly operations

- **slide-assembly-import-source.py** — Source deck with different theme and layout set for import/merge testing.
  - Output: `../pptx/slide-assembly-import-source/presentation.pptx`
  - Slides: 4 slides with custom theme colors and styling
  - Layouts: Title Slide, Title and Content, Two Content, Blank
  - Features: Custom RGB colors and styled text boxes for theme/layout reconciliation testing

- **slide-assembly-notes-media.py** — Slides with speaker notes and embedded media for assembly testing.
  - Output: `../pptx/slide-assembly-notes-media/presentation.pptx`
  - Slides: 5 slides with speaker notes on all slides
  - Media: 3 embedded images (PNG) on slides 2-4
  - Purpose: Test media and notes preservation during slide assembly operations
  - Features: Multi-line notes, mixed image placement, different layout types

### Corrupted Fixtures

These are deliberately broken PPTX files for testing validator features (M3+).

- **corrupted-missing-media.py** — PPTX with an image relationship pointing to a non-existent media file.
  - Output: `../pptx/corrupted-missing-media/presentation.pptx`
  - Corruption: `ppt/slides/slide1.xml` references `../media/image1.png` (file deleted)
  - Provenance: python-pptx base + ZIP manipulation

- **corrupted-dangling-layout.py** — PPTX with a slide referencing a non-existent slide layout.
  - Output: `../pptx/corrupted-dangling-layout/presentation.pptx`
  - Corruption: `ppt/slides/slide2.xml.rels` references `slideLayout99.xml` (does not exist)
  - Provenance: python-pptx base + ZIP manipulation

## Running Generators

To regenerate all fixtures:

```bash
cd /path/to/ooxml-cli
make fixtures
```

Or manually:

```bash
python testdata/generate/python/minimal_title.py
python testdata/generate/python/title_content.py
python testdata/generate/python/picture_placeholder.py
python testdata/generate/python/table_slide.py
python testdata/generate/python/chart_simple.py
python testdata/generate/python/notes_slide.py
python testdata/generate/python/multi_layout.py
python testdata/generate/python/notes_handout.py
python testdata/generate/python/corrupted_missing_media.py
python testdata/generate/python/corrupted_dangling_layout.py
python testdata/generate/python/slide_assembly_multi.py
python testdata/generate/python/slide_assembly_import_source.py
python testdata/generate/python/slide_assembly_notes_media.py
python testdata/generate/python/create_geometry_fixtures.py
python testdata/generate/python/create_rich_text_fixtures.py
python testdata/generate/python/create_producer_fixtures.py
```

## Validation

Valid PPTX fixtures should open without errors in LibreOffice:

```bash
libreoffice --headless testdata/pptx/minimal-title/presentation.pptx
```

Corrupted fixtures will produce validation errors when parsed, which is expected.

## Dependencies

- python-pptx 0.6.23 — PPTX generation
- Pillow (optional) — Image creation for picture-placeholder.py
- zipfile (stdlib) — ZIP manipulation for corrupted fixtures

## Notes

- All fixtures are committed to version control to ensure tests are reproducible and not dependent on Python or library versions.
- No hand-authored PPTX XML — all structure is generated by python-pptx or programmatic ZIP manipulation.
- Corruption is applied at the ZIP level after python-pptx has created a valid base structure.
