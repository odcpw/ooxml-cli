# Render Font Environment

This project renders PPTX files on Linux through headless LibreOffice followed by `pdftoppm` rasterization. Visual stability depends heavily on the installed font set.

## Recommended pinned font packages

For CI and reproducible local smoke runs, install and keep pinned at the image level:

- `fonts-liberation`
- `fonts-noto-core`
- `fonts-noto-cjk`
- `fonts-noto-color-emoji`
- `fonts-dejavu-core`

These packages cover the common Latin, symbol, emoji, and CJK fallback cases that show up in sample decks and avoid many silent substitution differences across environments.

## Toolchain assumptions

The render pipeline expects these binaries on `PATH`:

- `soffice` (or `libreoffice`) for PPTX → PDF
- `pdftoppm` for PDF → PNG/JPG
- later visual diff work also expects `compare` / `magick`

## Smoke-test contract

The current smoke target uses `testdata/pptx/minimal-title/presentation.pptx` and validates that:

1. LibreOffice can produce a PDF artifact.
2. `pdftoppm` can rasterize the PDF.
3. At least one slide image is emitted.

This smoke test proves tool availability and catches broken headless environments early, but it is not a golden-pixel stability test yet.

## Updating baselines later

When the project introduces visual diff goldens, treat any font-package change as a baseline change:

1. update the pinned package list in CI/image provisioning,
2. rerun the render smoke and visual diff jobs,
3. inspect the changed artifacts,
4. refresh goldens only after confirming the font change is intentional.
