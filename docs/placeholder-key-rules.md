# Placeholder Key-Generation Rules

**Version:** 1.0  
**Date:** 2026-03-09  
**Status:** Ratified  
**Risk Level:** Critical; keys are a permanent contract

## Overview

Placeholder keys are stable, semantic identifiers for shape placeholders in PPTX presentations. Once shipped, **keys must never change**; downstream LLM workflows depend on them for cross-version correlation and mutation tracking.

This document defines:
1. **Role mapping** — how `p:ph@type` XML values map to canonical role names
2. **Key generation** — the four-priority algorithm for generating keys from placeholder metadata
3. **Rationale** — why each rule exists and what it handles

## Part 1: Role Mapping

The OOXML spec defines `p:ph@type` values for placeholder types. We map these to canonical, human-readable role names used throughout the CLI and in keys.

### Mapping Table

| PPTX p:ph@type | Canonical Role | Rationale |
|---|---|---|
| `title` | `title` | Standard title placeholder; unique per layout |
| `ctrTitle` | `title` | Center title (alternative form); treated as title |
| `subTitle` | `subtitle` | Subtitle placeholder; common on title slides |
| `body` | `body` | Content placeholder; often multiple per layout |
| `pic` | `pic` | Picture placeholder |
| `tbl` | `table` | Table placeholder |
| `chart` | `chart` | Chart placeholder |
| `obj` | `object` | Generic object placeholder |
| `dt` | `date` | Date placeholder; often auto-filled by renderer |
| `ftr` | `footer` | Footer placeholder; typically global |
| `sldNum` | `slideNumber` | Slide number placeholder; typically global |
| *(unknown)* | *(preserve literally)* | If type doesn't match known mappings, preserve as-is for forward compatibility |

### Rationale

- **title + ctrTitle → title**: The OOXML spec uses two values for the title slot; we unify them semantically.
- **Canonical names are lowercase**: Matches CLI output style and is easier to parse in scripts.
- **Unknown types are preserved**: Allows graceful forward compatibility if newer PPTX specs introduce new placeholder types.

---

## Part 2: Key Generation

Keys are generated using a **four-priority algorithm**. The first matching rule wins; lower priorities are fallbacks for edge cases.

### Priority 1: Unique Canonical Role

**Applies when:** Placeholder has a canonical role, AND that role is unique within the layout.

**Format:** `{role}`

**Examples:**
- `title` (title role is unique in a title layout)
- `subtitle` (subtitle role is unique where it appears)
- `footer` (often unique in a layout)

**Rationale:**
- Simplest, most readable key
- Guarantees stability since we enforce uniqueness
- Works for layouts with only one title, one subtitle, etc.

### Priority 2: Non-Unique Role

**Applies when:** Placeholder has a canonical role, but that role appears multiple times in the layout.

**Format:** `{role}:{idx}`

**Examples:**
- `body:0` (first body placeholder)
- `body:3` (fourth body placeholder)
- `pic:1` (second picture placeholder)

**Rationale:**
- Disambiguates multiple placeholders of the same type
- Index is 0-based for consistency with common programming conventions
- Allows stable correlation across layout changes (reordering updates indices predictably)

### Priority 3: No Type, But Has Index

**Applies when:** Placeholder lacks a resolved type (no `p:ph@type` or it's unmapped) but has an index.

**Format:** `ph:{idx}`

**Examples:**
- `ph:11` (placeholder at index 11, type unknown)
- `ph:0` (first untyped placeholder)

**Rationale:**
- Handles corrupt or future PPTX files where type is missing
- `ph:` prefix signals "generic placeholder"
- Better than using shape ID alone because index is often more stable across versions

### Priority 4: Shape ID Fallback

**Applies when:** Placeholder has no type, no index, or both are missing. Fall back to shape ID.

**Format:** `shape:{shapeId}`

**Examples:**
- `shape:4` (shape with ID 4)
- `shape:123` (shape with ID 123)

**Rationale:**
- Last resort when all metadata is absent
- Shape IDs are unique per slide and stable within a version
- May drift across major layout changes, but better than no key at all

---

## Part 3: Layout Context

Key generation is context-dependent — a role might be unique in one layout but not another. Implementations must provide a `LayoutContext` interface that answers: "Is role X unique in this layout?"

```go
type LayoutContext interface {
    // IsRoleUniqueInLayout returns true if the role appears exactly once
    IsRoleUniqueInLayout(role string) bool
}
```

### Responsibilities

- Count placeholders by role in the layout
- Handle missing/unknown roles (treat them as singletons if they only appear once)
- Be consistent — the same layout context should always return the same answer for the same role

### Implementation Notes

- If a layout has no placeholders, all roles are "unique" (vacuous truth)
- A role with count == 1 is unique; count > 1 is not
- Unknown roles (not mapped to a canonical name) should still be counted when computing uniqueness

---

## Part 4: Examples

### Example 1: Title Layout

Layout has placeholders: `[title, subtitle, body]`

Uniqueness: title=1, subtitle=1, body=1 (all unique)

| Placeholder | Role | Is Unique | Generated Key |
|---|---|---|---|
| p:ph@type="title" | title | ✓ | `title` |
| p:ph@type="subTitle" | subtitle | ✓ | `subtitle` |
| p:ph@type="body" idx=0 | body | ✓ | `body` |

### Example 2: Content Layout with Multiple Bodies

Layout has placeholders: `[title, body idx=0, body idx=1, body idx=2]`

Uniqueness: title=1 (unique), body=3 (not unique)

| Placeholder | Role | Is Unique | Generated Key |
|---|---|---|---|
| p:ph@type="title" | title | ✓ | `title` |
| p:ph@type="body" idx=0 | body | ✗ | `body:0` |
| p:ph@type="body" idx=1 | body | ✗ | `body:1` |
| p:ph@type="body" idx=2 | body | ✗ | `body:2` |

### Example 3: Corrupt/Unknown Metadata

Layout has placeholders with mixed metadata.

| Placeholder | Role | Metadata | Generated Key |
|---|---|---|---|
| p:ph@type="title" | title | type=title | `title` |
| (no p:ph) | (none) | no type, no idx, shapeId=5 | `shape:5` |
| p:ph@type="unknown" idx=7 | unknown | type=unknown, idx=7 | `ph:7` |

---

## Part 5: Stability Contract

### What Can Change (Safe)

- JSON representation of roles (e.g., output field names)
- Documentation and examples
- Internal implementation details

### What Cannot Change (Breaking)

- Key format for existing rules (e.g., `title` must always mean priority 1, unique role)
- Role mapping (e.g., `ctrTitle` must always map to `title`)
- Priority order (e.g., we cannot add a new priority above priority 1)
- The `LayoutContext` interface behavior

**If a change is necessary**, it must be:
1. Ratified in this document with version bump
2. Communicated as a breaking change in release notes
3. Considered a security/correctness issue that justifies the break

---

## Part 6: Test Coverage

All rules must be unit-tested with pure tests (no fixture I/O). Test cases must cover:

### CanonicalRole() tests
- ✓ Each mapped type (title, ctrTitle, subTitle, body, pic, tbl, chart, obj, dt, ftr, sldNum)
- ✓ Unknown type (preserves literally)

### GenerateKey() tests
- ✓ Priority 1: Unique role → key is role name (title, subtitle, etc.)
- ✓ Priority 2: Non-unique role with index → key is `role:idx`
- ✓ Priority 3: No type, has index → key is `ph:idx`
- ✓ Priority 4: No type, no index → key is `shape:{shapeId}`
- ✓ All combinations with LayoutContext mocks

---

## Changelog

### v1.0 (2026-03-09)

- Initial specification
- Four-priority key generation algorithm
- 11-entry role mapping table
- LayoutContext interface contract
