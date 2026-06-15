# Translation Manifest Entry ID Rules

**Version:** 1.0.0  
**Date:** 2026-03-10  
**Status:** Ratified  
**Risk Level:** Critical; IDs are a permanent contract for translation workflows

## Overview

Translation entry IDs are stable, deterministic identifiers for translatable text units within a PPTX presentation. Once exported in a manifest, **IDs must never change**; downstream translation workflows, diff tools, and version control systems depend on them for tracking changes and correlating translations across versions.

This document defines:
1. **ID Format** — the structure and components of entry IDs
2. **ID Generation Algorithm** — how to compute a deterministic ID from text location
3. **Stability Contract** — what can and cannot change
4. **Examples** — concrete ID examples from different text types

## Part 1: ID Format

Entry IDs follow a deterministic, hierarchical format that uniquely identifies a text unit within a deck:

```
slide:<slide-id>_<shape-key>_p<para-idx>_r<run-idx>
```

### Component Definitions

#### slide-id (0-based)
- Zero-based index of the slide in presentation order
- Examples: `0` (first slide), `5` (sixth slide), `42` (forty-third slide)
- Always prefixed with `slide:`

#### shape-key
- Canonical placeholder key (from [placeholder-key-rules.md](placeholder-key-rules.md))
- Examples: `title`, `subtitle`, `body:0`, `body:5`, `shape:123`
- Combines role with index for non-unique roles
- Falls back to `shape:N` for untypified shapes

#### para-idx (0-based)
- Zero-based paragraph index within the shape's text body
- Examples: `0` (first paragraph), `2` (third paragraph)
- Always prefixed with `p`

#### run-idx (0-based)
- Zero-based run (or segment) index within the paragraph
- Includes text runs, breaks, tabs, fields
- Examples: `0` (first run), `5` (sixth run)
- Always prefixed with `r`

### Full Examples

| ID | Meaning |
|---|---|
| `slide:0_title_p0_r0` | First run of first paragraph of title placeholder on slide 0 |
| `slide:0_body:0_p1_r2` | Third run of second paragraph of first body placeholder on slide 0 |
| `slide:2_subtitle_p0_r0` | First run of first paragraph of subtitle on slide 2 |
| `slide:5_shape:123_p0_r0` | First run of first paragraph of untypified shape 123 on slide 5 |
| `slide:10_body:3_p5_r8` | Ninth run of sixth paragraph of fourth body placeholder on slide 10 |

---

## Part 2: ID Generation Algorithm

IDs are generated deterministically from three inputs:

1. **Slide location**: Zero-based slide index
2. **Shape context**: Placeholder key (using rules from placeholder-key-rules.md)
3. **Text location**: Paragraph index and run index within that paragraph

### Algorithm Steps

```
1. Compute shape-key using GenerateKey() from placeholder-key-rules.md
2. For each paragraph in the shape (index 0, 1, 2, ...):
   3. For each run/segment in the paragraph (index 0, 1, 2, ...):
      4. Generate ID: slide:<slide-id>_<shape-key>_p<para-idx>_r<run-idx>
      5. Store entry in manifest with this ID
```

### Determinism Contract

**The same text at the same location must always produce the same ID.**

This means:
- Adding/removing text elsewhere in the deck does NOT change IDs of existing text
- Reordering slides changes slide IDs (breaking change, documented in release notes)
- Reordering paragraphs within a shape changes para-idx (documented)
- Reordering runs within a paragraph changes run-idx (documented)
- Placeholder keys are immutable (per placeholder-key-rules.md)

---

## Part 3: Validation

Entry IDs must match the format regex:

```regex
^slide:\d+_[a-zA-Z0-9:]+_p\d+_r\d+$
```

Valid IDs pass these checks:
- Exactly 4 components separated by `_`
- First component: `slide:` prefix + numeric slide ID
- Second component: alphanumeric shape key (may contain `:` for indexed placeholders)
- Third component: `p` prefix + numeric paragraph index
- Fourth component: `r` prefix + numeric run index

### Invalid ID Examples

| ID | Reason |
|---|---|
| `0_title_p0_r0` | Missing `slide:` prefix |
| `slide:0_title_0_r0` | Missing `p` prefix on paragraph index |
| `slide:0_title_p0_0` | Missing `r` prefix on run index |
| `slide:a_title_p0_r0` | Non-numeric slide ID |
| `slide:0_title_px_r0` | Non-numeric paragraph index |
| `slide:0__p0_r0` | Empty shape key |

---

## Part 4: Manifest Versioning

Manifests include a version field (`metadata.version`) for backward compatibility and migration planning.

### Current Version: 1.0.0

- Defines entry ID format and generation algorithm
- Specifies metadata fields and optional entry metadata
- Requires entry IDs to be unique per manifest

### Version Upgrade Scenarios

If the ID generation algorithm must change (e.g., to fix a collision bug or accommodate a new feature), the process is:

1. **Propose** the change with rationale
2. **Bump** ManifestVersion (e.g., 1.0.0 → 1.1.0 for compatible change, 2.0.0 for breaking)
3. **Document** the change in this file
4. **Implement** a migration strategy (old manifest → new ID format)
5. **Communicate** to users about the breaking change

### Migration Example (Hypothetical)

If v1.0.0 had a collision in body placeholders, v2.0.0 might change the format:

```
v1.0.0: slide:0_body:0_p0_r0  (potential collision)
v2.0.0: slide:0_shape:2_p0_r0 (always use shape ID, no collision)
```

Tools must support reading both formats and offer a conversion path.

---

## Part 5: Integration with Export/Apply

### Export Workflow (task-61)

When exporting a deck to a translation manifest:

```
For each slide in deck:
  For each shape with text:
    Compute placeholder-key (or shape-key fallback)
    For each paragraph:
      For each run:
        Generate ID = slide:<slide-id>_<shape-key>_p<para-idx>_r<run-idx>
        Create entry with:
          - ID (immutable)
          - SourceText (current text)
          - TargetText (empty or from previous manifest)
          - ContextHash (for freshness detection)
          - Metadata (bullet info, formatting, etc.)
```

### Apply Workflow (task-68)

When applying translations back to a deck:

```
For each entry in translation manifest:
  Parse ID to get slide, shape-key, para-idx, run-idx
  Verify ContextHash matches (detect stale entries)
  If stale: warn or skip
  If fresh:
    Locate the shape by placeholder-key
    Update paragraph[para-idx].run[run-idx].text = entry.TargetText
```

### Diff Workflow (task-62)

When comparing two manifests:

```
For each entry ID in manifest-v1:
  If ID not in manifest-v2: entry was removed
For each entry ID in manifest-v2:
  If ID not in manifest-v1: entry was added
  If ID in both:
    If SourceText changed: content drift (warn)
    If TargetText changed: translation updated (track)
```

---

## Part 6: Stability Contract

### What Can Change (Safe)

- JSON representation (field names, structure, optional fields)
- Documentation and examples
- Internal ID parsing implementation (as long as output format stays the same)
- Optional entry metadata fields (BulletInfo, RunFormat, Notes, etc.)

### What Cannot Change (Breaking)

- ID format (must always be `slide:X_Y_pZ_rW`)
- ID generation algorithm (same input → same output forever)
- Meaning of components (slide-id is always slide index, para-idx is always paragraph index)
- Manifest version semantics (version field always means schema version)

**If a change is necessary**, it must be:
1. Ratified in this document with version bump
2. Documented as a breaking change
3. Communicated in release notes with migration guidance
4. Considered a critical issue that justifies the break

---

## Part 7: Test Coverage

All ID generation and validation must be unit-tested:

### GenerateEntryID() Tests
- ✓ Simple IDs (title on slide 0)
- ✓ Indexed placeholders (body:0, body:5)
- ✓ Shape fallbacks (shape:123)
- ✓ Multiple paragraphs and runs
- ✓ Determinism (same input → same output)

### ValidateID() Tests
- ✓ Valid IDs pass validation
- ✓ Invalid format IDs fail (missing prefixes, wrong components)
- ✓ Non-numeric components fail
- ✓ Empty components fail

### ParseID() Tests
- ✓ Valid IDs parse correctly
- ✓ Components extracted accurately
- ✓ Roundtrip: GenerateEntryID → ParseID returns original components
- ✓ Invalid IDs return error

### Collision Tests
- ✓ Different locations produce different IDs
- ✓ ID space is collision-free for practical deck sizes

### Manifest Roundtrip Tests
- ✓ Manifest → JSON → Manifest is lossless
- ✓ Entry IDs survive serialization
- ✓ Unchanged decks produce identical manifests across exports

---

## Part 8: Glossary

| Term | Definition |
|---|---|
| **Entry ID** | Stable identifier for a translation entry; immutable once exported |
| **Shape Key** | Placeholder key or shape fallback; generated per placeholder-key-rules.md |
| **Para Index** | Zero-based paragraph index within a shape's text body |
| **Run Index** | Zero-based run/segment index within a paragraph |
| **Context Hash** | SHA256 hash of surrounding text for freshness detection |
| **Manifest Version** | Schema version for backward compatibility and migration |
| **Stale Entry** | Translation entry whose source text no longer matches the deck |

---

## Changelog

### v1.0.0 (2026-03-10)

- Initial specification
- Entry ID format: `slide:X_Y_pZ_rW`
- Deterministic generation algorithm
- Validation rules and regex
- Integration with export/apply/diff workflows
- Stability contract and version upgrade process
