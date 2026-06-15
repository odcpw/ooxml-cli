# Theme Mutation Guide

## Overview

The `ooxml pptx theme update` command allows you to modify the colors and fonts of a presentation. There are two distinct modes:

1. **Deck Mode** (default): Modifies the theme itself, affecting all slides
2. **Slide Mode**: Applies color overrides to specific slides without changing the theme

## Deck Mode (Theme Mutation)

In deck mode, the command updates the presentation's theme directly. This affects all slides that use the theme colors and fonts.

### Usage

```bash
# Update a single theme color
ooxml pptx theme update deck.pptx --color "accent1=FF0000" --out updated.pptx

# Update multiple colors
ooxml pptx theme update deck.pptx \
  --color "accent1=FF0000" \
  --color "accent2=00FF00" \
  --color "accent3=0000FF" \
  --out updated.pptx

# Update theme fonts
ooxml pptx theme update deck.pptx \
  --major-font "Arial" \
  --minor-font "Calibri" \
  --out updated.pptx

# Modify both colors and fonts
ooxml pptx theme update deck.pptx \
  --color "accent1=FF0000" \
  --major-font "Arial" \
  --in-place
```

### Valid Color Names

The following theme color names can be updated:

- `dk1`, `lt1` — Dark and Light primary colors
- `dk2`, `lt2` — Dark and Light secondary colors
- `accent1`, `accent2`, `accent3`, `accent4`, `accent5`, `accent6` — Accent colors
- `hlink` — Hyperlink color
- `folHlink` — Followed hyperlink color

All hex values must be 6-character uppercase or lowercase hexadecimal (e.g., `FF0000`, `00ff00`).

### What Gets Updated

- **Theme XML** (`/ppt/theme/theme1.xml`) is modified
- All slides using the theme see the new colors/fonts
- The theme is the "source of truth" for all slides

## Slide Mode (Color Overrides)

In slide mode, the command applies color overrides to specific slides only. The theme itself is unchanged, and the override affects only the targeted slides.

### Usage

```bash
# Update a single slide's colors
ooxml pptx theme update deck.pptx --slide 1 \
  --color "accent1=FF0000" \
  --mode slide \
  --out updated.pptx

# Update multiple slides
ooxml pptx theme update deck.pptx --for-slides "1,3,5" \
  --color "accent1=FF0000" \
  --color "accent2=00FF00" \
  --mode slide \
  --out updated.pptx

# Update slides using range notation
ooxml pptx theme update deck.pptx --for-slides "1-5,7,9-10" \
  --color "accent1=FF0000" \
  --mode slide \
  --out updated.pptx
```

### Slide Targeting

Use either `--slide` or `--for-slides`:

- `--slide N` — Target a single slide (1-based numbering)
- `--for-slides "spec"` — Target multiple slides:
  - `"1,3,5"` — Slides 1, 3, and 5
  - `"1-5"` — Slides 1 through 5 (inclusive)
  - `"1,3-5,7"` — Mix of single and ranges

### What Gets Modified

- Individual slide XML files (`/ppt/slides/slide1.xml`, etc.)
- A `clrMapOvr` element is added or updated in each targeted slide
- The override only affects the specific slide(s)
- The theme is unchanged

### Font Updates Not Supported in Slide Mode

Currently, font updates are only supported in deck mode. To apply font changes to specific slides, use deck mode and recreate the theme with the desired fonts.

## Comparing Deck Mode vs Slide Mode

| Aspect | Deck Mode | Slide Mode |
|--------|-----------|-----------|
| **What changes** | Theme file | Individual slide files |
| **Scope** | All slides using theme | Specific slides only |
| **Font updates** | Supported | Not supported |
| **Use case** | Brand-wide recoloring | Highlight specific slides |
| **No-op fail** | Yes | Yes |

## Error Handling

The command fails clearly when:

- No colors or fonts are specified (no-op case)
- Invalid color name is used
- Hex value is not 6 hex digits
- File doesn't exist
- Slide numbers are out of range
- Conflicting flags are used (e.g., both `--slide` and `--for-slides`)

### Examples

```bash
# This fails: no updates specified
ooxml pptx theme update deck.pptx --out out.pptx
# Error: no updates specified; use --color, --major-font, or --minor-font

# This fails: invalid color name
ooxml pptx theme update deck.pptx --color "invalid=FF0000" --out out.pptx
# Error: invalid color name 'invalid'; must be one of: ...

# This fails: invalid hex value
ooxml pptx theme update deck.pptx --color "accent1=GGGGGG" --out out.pptx
# Error: invalid hex color 'GGGGGG'; must be 6 hexadecimal characters

# This fails: both flags specified
ooxml pptx theme update deck.pptx --slide 1 --for-slides "1-3" --mode slide --out out.pptx
# Error: cannot specify both --slide and --for-slides
```

## JSON Output

Use `--format json --pretty` for structured output:

```bash
ooxml pptx theme update deck.pptx --color "accent1=FF0000" --format json --pretty
```

Output:
```json
{
  "colors": [
    {
      "colorName": "accent1",
      "hexValue": "FF0000",
      "mode": "deck"
    }
  ],
  "message": "theme update completed successfully"
}
```

## Implementation Details

### Deck Mode (M17-1/M17-2)

The deck mode uses:
- `UpdateThemeColor()` to modify color scheme entries in the theme
- `UpdateThemeFont()` to modify major/minor fonts in the theme
- Strong input validation for color names and hex values
- Preservation of unrelated theme content

### Slide Mode (M17-3)

The slide mode uses:
- `ApplySlideColorOverride()` to create or update `clrMapOvr` elements
- `RemoveSlideColorOverrides()` to remove or clear overrides
- PPTX-compliant color mapping that doesn't modify the theme

## Validation and Testing

All theme updates are validated:
- Output decks validate against PPTX schema
- Decks open correctly in LibreOffice
- Untouched content is preserved
- Both deck-level and slide-level updates are covered by focused tests

## Limitations and Future Work

- Slide mode does not support font updates (M17-4 scope)
- East Asian (EA) and Complex Script (CS) fonts are not yet supported in font updates (M17-2 scope)
- Color overrides use simple srgbClr elements (gradient and scheme color support is possible future work)
