# Entity Batch Fix Notes

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
