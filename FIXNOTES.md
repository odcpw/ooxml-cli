# Audit Fix Handoff

This document consolidates the independently audited fix waves that were integrated for the Rust 0.1.0 release candidate.

## Entity Batch

## ENTITY-1
- Status: fixed.
- Files touched: `src/xml_util.rs`, `src/docx_replace.rs` behavior via `decode_xml_text`, DOCX readers under `src/docx_*`.
- Tests added: `tests/entity_conformance.rs::docx_entity_text_and_untouched_runs_survive_replace`.
- Behavior changes: numeric character references in raw `w:t` fragments now decode before rewrite, so untouched runs no longer become visible `&amp;#...;` text.

## ENTITY-2
- Status: fixed.
- Files touched: `src/xml_util.rs`, `src/xlsx_names/package.rs`, `src/conformance_invariants/spreadsheet_semantics.rs`, `src/diff.rs`.
- Tests added: `tests/entity_conformance.rs::xlsx_entity_formulas_survive_list_and_unrelated_update`.
- Behavior changes: defined-name formulas now preserve decoded `&`, `<`, `>`, quote, apostrophe, decimal numeric refs, and hex numeric refs when listed or re-rendered.

## ENTITY-3
- Status: fixed.
- Files touched: `src/xml_util.rs`, `src/xlsx_data_validations.rs`.
- Tests added: `tests/entity_conformance.rs::xlsx_entity_formulas_survive_list_and_unrelated_update`.
- Behavior changes: data-validation formulas and attributes now decode entity references correctly; unrelated updates no longer strip comparison operators or double-escape numeric refs.

## ENTITY-4
- Status: fixed.
- Files touched: `src/xml_util.rs`, `src/pptx_mutation/replace.rs`.
- Tests added: `tests/entity_conformance.rs::pptx_entity_text_survives_readback_and_matched_node_rewrite`; empirical PPTX repro verified with `target/debug/ooxml`.
- Behavior changes: `pptx replace text-occurrences` now matches and rewrites text nodes using decoded entity-aware text, preserving `&`, `<`, `>`, quotes, apostrophes, and numeric refs.

## ENTITY-5
- Status: fixed.
- Files touched: `src/xml_util.rs` and every migrated text reader that now uses its helper.
- Tests added: all tests in `tests/entity_conformance.rs`.
- Behavior changes: `xml_unescape`, `decode_xml_text`, and `xml_general_ref` now resolve decimal and hex numeric character references. Unknown or malformed entities are still preserved lossily instead of panicking.

## ENTITY-6
- Status: fixed.
- Files touched: `src/docx_block_readers.rs`, `src/docx_block_readers/rich.rs`, `src/docx_xml/text_read.rs`, `src/docx_headers/paragraphs.rs`, `src/docx_comments/read.rs`, `src/docx_fields/read.rs`.
- Tests added: `tests/entity_conformance.rs::docx_entity_text_and_untouched_runs_survive_replace`.
- Behavior changes: `docx text` and related DOCX readers now include `GeneralRef` text in extracted paragraph text.

## ENTITY-7
- Status: fixed.
- Files touched: `src/pptx_readback/comments.rs`, `src/pptx_readback/fields.rs`, `src/pptx_readback/charts.rs`, `src/pptx_readback/animations.rs`, `src/pptx_readback/shape_model.rs`, `src/pptx_mutation/tables.rs`, `src/pptx_mutation/text.rs`, `src/pptx_mutation/fields.rs`, `src/pptx_mutation/comments.rs`, `src/pptx_layout_qa.rs`, `src/pptx_mutation/charts/xml.rs`, plus related XLSX/DOCX/conformance/diff readers found by the full `Event::Text` sweep.
- Tests added: `tests/entity_conformance.rs::pptx_entity_text_survives_readback_and_matched_node_rewrite`.
- Behavior changes: PPTX notes/comments/tables/charts/fields/animations/layout and guard readbacks now collect text through `append_xml_text_event`.

## Shared Notes
- Shared helper: `src/xml_util.rs` now owns `append_xml_text_event`, `is_xml_text_event`, and `TextAccumulator`.
- Sweep evidence: `rg 'Event::Text|Event::GeneralRef' src` now reports only `src/xml_util.rs`.
- Golden updates: none. The behavior change corrects decoded text semantics and is covered by new command-path tests rather than pinned golden output updates.
## OPC Batch

### OPC-1

Status: fixed.

Files touched: `src/opc.rs`, `src/main.rs`, `src/validation.rs`, `src/conformance_invariants/relationships.rs`, `tests/rust_contract_smoke/conformance_relationships.rs`.

Tests added: `validation_matches_relationship_targets_percent_decoded_and_case_insensitive`.

Behavior change: relationship target existence checks now percent-decode target URIs and compare OPC part names ASCII-case-insensitively. Malformed percent escapes are reported as relationship target diagnostics instead of being treated as normal missing parts.

### OPC-2

Status: fixed.

Files touched: `src/conformance_invariants/relationships.rs`, `tests/rust_contract_smoke/conformance_relationships.rs`.

Tests added: `conformance_accepts_explicit_internal_target_mode`.

Behavior change: `TargetMode="Internal"` is accepted as the explicit form of the internal relationship default. Values other than empty, `Internal`, and `External` remain conformance errors.

### OPC-3

Status: fixed.

Files touched: `src/opc.rs`, `src/conformance_invariants/content_types.rs`, `tests/rust_contract_smoke/conformance_relationships.rs`.

Tests added: `conformance_default_extension_matching_is_case_insensitive`.

Behavior change: `[Content_Types].xml` Default extension keys are normalized to ASCII lowercase and lookup uses ASCII-case-insensitive extension matching.

### OPC-4

Status: fixed.

Files touched: `src/validation.rs`, `tests/rust_contract_smoke/conformance_relationships.rs`.

Tests added: `validate_reports_malformed_relationship_part_as_diagnostic`.

Behavior change: malformed `.rels` XML now yields an error-severity `REL_MALFORMED` validation diagnostic and validation continues to inspect remaining relationship parts. The command returns a validation report instead of a top-level `unexpected` error.

### OPC-5

Status: fixed.

Files touched: `src/opc.rs`, `src/main.rs`, `src/docx_comments.rs`, `src/docx_headers.rs`, `src/docx_images.rs`, `src/pptx_media.rs`, `src/pptx_mutation/charts/data.rs`, `src/pptx_mutation/comments.rs`, `src/pptx_mutation/import_merge.rs`, `src/pptx_mutation/layouts.rs`, `src/pptx_mutation/notes.rs`, `src/pptx_mutation/placement.rs`, `src/pptx_mutation/replace.rs`, `src/pptx_mutation/slides.rs`, `src/xlsx_charts/create.rs`, `src/xlsx_comments.rs`, `src/xlsx_metadata.rs`, `src/xlsx_mutation/format.rs`, `src/xlsx_mutation/format/style.rs`, `src/xlsx_pivots.rs`, `src/xlsx_sheet_lifecycle.rs`, `src/xlsx_table_create.rs`.

Tests added: `ensure_content_type_override_detects_legal_existing_override_serializations`, `ensure_content_type_override_refuses_self_closing_types_root`.

Behavior change: `ensure_content_type_override` parses existing Override elements instead of raw substring matching, so single quotes, attribute order, and ASCII case differences do not cause duplicate overrides. A self-closing or otherwise unsplicable `Types` root now returns an error instead of silently omitting the override.

### OPC-6

Status: fixed.

Files touched: `src/diff.rs`, `tests/rust_contract_smoke/diff.rs`.

Tests added: `top_level_diff_docx_reports_header_part_changes`, `top_level_diff_docx_reports_media_part_changes`.

Behavior change: DOCX diff output now includes secondary-part comparison fields under `semantic`: `secondaryPartCountA`, `secondaryPartCountB`, `secondaryPartCountEqual`, `changedParts`, and `partDiffs`. The existing main-document `blocks` diff remains intact; secondary headers, footers, footnotes, endnotes, comments, styles, numbering, and media parts are compared by SHA-256 content hash.

### Additional Test-Gate Fix

Status: fixed.

Files touched: `src/vba/run_smoke.rs`.

Tests used: `vba_run_smoke_rejects_bad_cli_contract_before_office`.

Behavior change: `vba run-smoke` now rejects invalid CLI-only options before the Windows/Office availability guard, preserving the existing test contract on non-Windows machines. This was outside the OPC findings but required for the mandated full `cargo test` gate.
