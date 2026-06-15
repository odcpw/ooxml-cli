# Layout Authoring from Existing Client Decks

This guide focuses on the highest-value real-world workflow for `ooxml-cli` today:

- start from an incoming client deck
- inspect the existing masters/layouts
- clone a layout that is already close to what you want
- rename it
- remove placeholders/shapes you do not want
- add or reposition picture placeholders
- create new slides from that authored layout
- fill picture placeholders directly by normalized slot key

This is the recommended path today instead of trying to create a brand-new master or layout from scratch.

## Supported commands

- `ooxml pptx layouts list`
- `ooxml pptx layouts show`
- `ooxml pptx layouts clone`
- `ooxml pptx layouts rename`
- `ooxml pptx layouts delete-shape`
- `ooxml pptx layouts set-bounds`
- `ooxml pptx layouts add-placeholder`
- `ooxml pptx new-slide-from-layout`

## 1. Inspect the deck

```bash
ooxml inspect client-deck.pptx
ooxml pptx layouts list client-deck.pptx --format json --pretty
ooxml pptx layouts show client-deck.pptx --layout "6pictures" --format json --pretty
```

Look for:

- layout names that are already close to the desired design
- normalized placeholder keys such as `title`, `body`, `pic:0`, `pic:1`
- geometry/bounds you want to preserve or reuse

## 2. Clone and rename a layout

```bash
ooxml pptx layouts clone client-deck.pptx \
  --layout "6pictures" \
  --name "7pictures" \
  --out client-deck-with-7pictures.pptx

# optional if you want to rename later
ooxml pptx layouts rename client-deck-with-7pictures.pptx \
  --layout "7pictures" \
  --name "7pictures authored" \
  --in-place
```

## 3. Remove shapes you do not want

```bash
# delete a title placeholder
ooxml pptx layouts delete-shape client-deck-with-7pictures.pptx \
  --layout "7pictures authored" \
  --target title \
  --in-place

# delete a specific body/content shape by shape id
ooxml pptx layouts delete-shape client-deck-with-7pictures.pptx \
  --layout "7pictures authored" \
  --target shape:3 \
  --in-place
```

Targets can be:

- normalized placeholder key: `title`, `body`, `pic:0`
- shape id: `shape:3`
- shape name: `~Picture Placeholder 1`

## 4. Add picture placeholders

```bash
ooxml pptx layouts add-placeholder client-deck-with-7pictures.pptx \
  --layout "7pictures authored" \
  --type pic \
  --bounds 550000,1450000,2500000,1800000 \
  --in-place

ooxml pptx layouts add-placeholder client-deck-with-7pictures.pptx \
  --layout "7pictures authored" \
  --type pic \
  --bounds 3200000,1450000,2500000,1800000 \
  --in-place
```

After cleanup of inherited placeholders, auto-allocation typically yields stable keys like `pic:0`, `pic:1`, `pic:2`.
Confirm the final normalized keys with `layouts show` before wiring automation to them.
Use `--idx` when you need to pin an exact placeholder index, including `--idx 0`.

## 5. Move or resize an authored shape

```bash
ooxml pptx layouts set-bounds client-deck-with-7pictures.pptx \
  --layout "7pictures authored" \
  --target pic:1 \
  --bounds 3200000,1450000,2600000,1850000 \
  --in-place
```

Bounds are always `x,y,cx,cy` in EMUs.

## 6. Verify the authored layout

```bash
ooxml pptx layouts show client-deck-with-7pictures.pptx \
  --layout "7pictures authored" \
  --format json --pretty
```

You should see placeholders with keys such as:

- `pic:0`
- `pic:1`
- `pic:2`

## 7. Create a new slide and fill picture placeholders directly

```bash
ooxml pptx new-slide-from-layout client-deck-with-7pictures.pptx \
  --layout "7pictures authored" \
  --image-fit cover \
  --set-image-slot pic:0=./img/a.jpg \
  --set-image-slot pic:1=./img/b.jpg \
  --set-image-slot pic:2=./img/c.jpg \
  --out out.pptx
```

This now works for authored picture placeholders as well as pre-existing slot targets.
The image is inserted at the placeholder bounds and the placeholder box is replaced on the created slide.
Use `--image-fit cover` when you want grid cells fully filled with crop-as-needed behavior.

You can also use `--set-image pic:0=...` when targeting normalized placeholder keys directly.

## 8. Render the result for visual QA

```bash
ooxml validate --strict out.pptx
ooxml pptx render out.pptx --out rendered/
```

For the best operator workflow, also re-run `layouts show` after layout edits so the final normalized keys are visible before wiring automation to them.

## 9. Recommended production stance

This slice is ready for practical use when you stay inside the intended workflow:

- existing client decks
- cloned/reworked existing layouts
- authored picture placeholders filled by explicit keys
- validate + render QA before delivery

Recommended checklist:

1. inspect the target layout with `layouts show`
2. apply clone / delete / set-bounds / add-placeholder edits
3. inspect the authored layout again to confirm final keys such as `pic:0`
4. create slides with `new-slide-from-layout`
5. run `ooxml validate --strict`
6. run `ooxml pptx render` and visually inspect the touched slides

## Example: minimal two-picture authoring flow

```bash
ooxml pptx layouts clone deck.pptx --layout 2 --name "Image Grid" --out work.pptx
ooxml pptx layouts delete-shape work.pptx --layout "Image Grid" --target title --in-place
ooxml pptx layouts delete-shape work.pptx --layout "Image Grid" --target shape:3 --in-place
ooxml pptx layouts add-placeholder work.pptx --layout "Image Grid" --idx 0 --type pic --bounds 1000,2000,3000,4000 --in-place
ooxml pptx layouts add-placeholder work.pptx --layout "Image Grid" --idx 1 --type pic --bounds 5000,6000,7000,8000 --in-place
ooxml pptx new-slide-from-layout work.pptx --layout "Image Grid" \
  --image-fit cover \
  --set-image-slot pic:0=./a.jpg \
  --set-image-slot pic:1=./b.jpg \
  --out out.pptx
```

## Current limits

Supported well today:

- clone an existing layout
- rename a layout
- delete layout shapes/placeholders
- move/resize layout shapes
- add picture placeholders to a layout
- create slides from authored layouts
- fill authored picture placeholders by normalized slot key

Still limited / not first-class yet:

- create a brand-new master from scratch
- create a brand-new layout part from scratch without starting from an existing layout
- richer layout shape editing beyond delete + set-bounds + add-placeholder
- higher-level image-fit ergonomics beyond current contain/cover behavior and the current raw `--image-fit` surface
- fully unattended trust for arbitrary third-party decks without operator QA
