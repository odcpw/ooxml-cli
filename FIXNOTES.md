# Audit Fix Handoff

This document consolidates the independently audited fix waves that were integrated for the Rust 0.1.0 release candidate.

## Entity Batch

### ENTITY-1
- Status: fixed.
- Files touched: `src/xml_util.rs`, `src/docx_replace.rs` behavior via `decode_xml_text`, DOCX readers under `src/docx_*`.
- Tests added: `tests/entity_conformance.rs::docx_entity_text_and_untouched_runs_survive_replace`.
- Behavior changes: numeric character references in raw `w:t` fragments now decode before rewrite, so untouched runs no longer become visible `&amp;#...;` text.

### ENTITY-2
- Status: fixed.
- Files touched: `src/xml_util.rs`, `src/xlsx_names/package.rs`, `src/conformance_invariants/spreadsheet_semantics.rs`, `src/diff.rs`.
- Tests added: `tests/entity_conformance.rs::xlsx_entity_formulas_survive_list_and_unrelated_update`.
- Behavior changes: defined-name formulas now preserve decoded `&`, `<`, `>`, quote, apostrophe, decimal numeric refs, and hex numeric refs when listed or re-rendered.

### ENTITY-3
- Status: fixed.
- Files touched: `src/xml_util.rs`, `src/xlsx_data_validations.rs`.
- Tests added: `tests/entity_conformance.rs::xlsx_entity_formulas_survive_list_and_unrelated_update`.
- Behavior changes: data-validation formulas and attributes now decode entity references correctly; unrelated updates no longer strip comparison operators or double-escape numeric refs.

### ENTITY-4
- Status: fixed.
- Files touched: `src/xml_util.rs`, `src/pptx_mutation/replace.rs`.
- Tests added: `tests/entity_conformance.rs::pptx_entity_text_survives_readback_and_matched_node_rewrite`; empirical PPTX repro verified with `target/debug/ooxml`.
- Behavior changes: `pptx replace text-occurrences` now matches and rewrites text nodes using decoded entity-aware text, preserving `&`, `<`, `>`, quotes, apostrophes, and numeric refs.

### ENTITY-5
- Status: fixed.
- Files touched: `src/xml_util.rs` and every migrated text reader that now uses its helper.
- Tests added: all tests in `tests/entity_conformance.rs`.
- Behavior changes: `xml_unescape`, `decode_xml_text`, and `xml_general_ref` now resolve decimal and hex numeric character references. Unknown or malformed entities are still preserved lossily instead of panicking.

### ENTITY-6
- Status: fixed.
- Files touched: `src/docx_block_readers.rs`, `src/docx_block_readers/rich.rs`, `src/docx_xml/text_read.rs`, `src/docx_headers/paragraphs.rs`, `src/docx_comments/read.rs`, `src/docx_fields/read.rs`.
- Tests added: `tests/entity_conformance.rs::docx_entity_text_and_untouched_runs_survive_replace`.
- Behavior changes: `docx text` and related DOCX readers now include `GeneralRef` text in extracted paragraph text.

### ENTITY-7
- Status: fixed.
- Files touched: `src/pptx_readback/comments.rs`, `src/pptx_readback/fields.rs`, `src/pptx_readback/charts.rs`, `src/pptx_readback/animations.rs`, `src/pptx_readback/shape_model.rs`, `src/pptx_mutation/tables.rs`, `src/pptx_mutation/text.rs`, `src/pptx_mutation/fields.rs`, `src/pptx_mutation/comments.rs`, `src/pptx_layout_qa.rs`, `src/pptx_mutation/charts/xml.rs`, plus related XLSX/DOCX/conformance/diff readers found by the full `Event::Text` sweep.
- Tests added: `tests/entity_conformance.rs::pptx_entity_text_survives_readback_and_matched_node_rewrite`.
- Behavior changes: PPTX notes/comments/tables/charts/fields/animations/layout and guard readbacks now collect text through `append_xml_text_event`.

### Shared Notes
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
## VBA Batch

### VBA-1

- Status: fixed.
- Files touched: `src/vba/codepage.rs`, `src/vba/mod.rs`, `src/vba/source/codec.rs`, `src/vba/authoring/codec.rs`.
- Tests added: Windows-1252 mapping unit tests in `src/vba/codepage.rs`; source and authoring encode/decode regressions for euro, dashes, quotes, and undefined C1 controls.
- Behavior changes: code page 1252 now decodes bytes `0x80..=0x9F` through the Windows-1252 extension table, replaces the five undefined byte values with U+FFFD on decode, and encodes defined Windows-1252 characters such as U+20AC/U+2013/U+2014 instead of rejecting them.

### VBA-2

- Status: fixed.
- Files touched: `src/vba/source/codec.rs`, `src/vba/authoring/codec.rs`.
- Tests added: source and authoring chunk-boundary regressions assert that every non-terminal emitted MS-OVBA chunk decompresses to exactly 4096 bytes.
- Behavior changes: final chunks whose literal representation would exceed the compressed chunk size limit are now emitted as short raw final chunks. Deterministic bytes change for affected inputs with a final remainder in the literal-overhead overflow range, currently 3641 through 4095 bytes. The committed VBA authoring golden fixtures did not cross this boundary; `vba_authoring_golden`, `vba_pptm_authoring_golden`, and `vba_docm_authoring_golden` passed without regeneration.

### VBA-3

- Status: fixed.
- Files touched: `src/vba/cfb.rs`.
- Tests added: cyclic directory FAT and regular stream FAT fixtures assert immediate cycle errors.
- Behavior changes: CFB regular FAT chain walking now rejects repeated sectors with `CFB FAT sector chain cycle at sector ...` before appending duplicate sector data.

### VBA-4

- Status: fixed.
- Files touched: `src/vba/cfb.rs`.
- Tests added: cyclic mini-FAT fixture asserts a clean cycle error before repeating mini-sector data.
- Behavior changes: CFB mini-FAT chain walking now rejects repeated mini sectors with `CFB mini FAT sector chain cycle at mini sector ...`.

### VBA-5

- Status: fixed.
- Files touched: `src/vba/source/codec.rs`.
- Tests added: decompressor regression tests for a bounded output ceiling and per-chunk decompressed output greater than 4096 bytes.
- Behavior changes: MS-OVBA container decompression now enforces a 256 MiB total decompressed output limit and a 4096-byte decompressed chunk limit. Raw chunks up to 4096 bytes are accepted so the writer can emit a short final raw chunk.

### Additional Full-Suite Gate

- Status: fixed.
- Files touched: `src/vba/run_smoke.rs`.
- Tests added: none; existing `tests/vba_run_smoke_cli.rs` covered the contract.
- Behavior changes: `vba run-smoke` validates local CLI arguments such as `--smoke-mode`, `--timeout-seconds`, and generated-workbook-only options before returning the non-Windows Office COM availability error.
## CLI Batch

### CLI-1

Status: fixed / mitigated.

Implemented serve/apply dispatch coverage for the missing package-mutation commands through the existing command dispatch path. `vba build-bin` is now honestly marked `opCompatible=false` because it creates a standalone `vbaProject.bin` rather than mutating an open package session. Added a guard test that opens a serve session for every `opCompatible=true` capability and asserts the dispatcher does not return `unsupported serve op command`.

Files touched: `src/serve/op_dispatch.rs`, `src/serve/op.rs`, `src/capabilities/commands/vba.rs`, `tests/rust_contract_smoke/serve.rs`.

Tests added: `serve_dispatches_every_op_compatible_capability_to_validation`.

Behavior changes: unsupported drift is now caught by tests; `vba build-bin` is no longer advertised as serve/apply compatible.

### CLI-2

Status: mitigated.

No command-table rearchitecture was attempted for this batch. The new opCompatible dispatcher guard covers the concrete five-layer drift from CLI-1.

Files touched: `tests/rust_contract_smoke/serve.rs`.

Tests added: `serve_dispatches_every_op_compatible_capability_to_validation`.

Behavior changes: none beyond the CLI-1 surface correction.

### CLI-3

Status: fixed.

Global flag parsing now accepts global `--format text` and leading `--format=json` consistently. Text format remains limited to commands that already produce text output; other commands return a self-consistent `invalid_args` message telling callers to use JSON. Capabilities metadata now describes JSON as the global default and notes that text is command-limited.

Files touched: `src/main.rs`, `src/cli_core.rs`, `src/capabilities.rs`, `tests/rust_contract_smoke/utility.rs`.

Tests added: `global_json_flags_normalize_before_and_after_command`.

Behavior changes: `--format text` is accepted for text utility commands and rejected clearly elsewhere.

### CLI-4

Status: fixed.

Removed unreachable `validate` and `diff` arms from `dispatch_value`; validation and diff remain handled by the live top-level paths.

Files touched: `src/cli_dispatch.rs`, `src/diff.rs`, `src/main.rs`.

Tests added: covered by existing focused contract tests and full suite.

Behavior changes: none intended.

### CLI-5

Status: fixed.

Replaced the hardcoded `pptx replace text` stub with the existing selector-based text-target machinery used by `pptx replace text-from-xlsx`. The command now mutates the resolved shape text and returns readback derived from the output package.

Files touched: `src/pptx_mutation.rs`, `src/pptx_mutation/replace.rs`, `src/pptx_mutation/replace/output.rs`, `src/pptx_mutation/replace/text_xlsx.rs`, `src/zip_io.rs`, `src/main.rs`, `tests/rust_contract_smoke/pptx/replace.rs`.

Tests added: `pptx_replace_text_uses_real_shape_selectors_and_readback`.

Behavior changes: `pptx replace text` requires a supported shape selector and errors when the target cannot be resolved; it no longer reports success without mutating.

### CLI-6

Status: fixed.

`serve_commit` now stages the committed package to a same-directory temp path and finalizes with `rename`, with a copy/remove fallback.

Files touched: `src/serve.rs`.

Tests added: covered by `serve_open_supports_in_place_backup_commit` and full suite.

Behavior changes: commit finalization now follows the same temp-and-rename write discipline as direct mutation commands.

### CLI-7

Status: fixed.

The serve/apply/MCP `pptx replace text` op now uses the same real selector-based mutation and output-derived readback as the CLI command.

Files touched: `src/serve/op_dispatch.rs`, `src/serve/op.rs`, `src/pptx_mutation/replace.rs`, `tests/rust_contract_smoke/serve/pptx.rs`.

Tests added: `serve_pptx_replace_text_uses_real_slide_two_selector_readback`.

Behavior changes: serve/apply/MCP callers now get clear target errors instead of fabricated success readback.

### CLI-8

Status: fixed.

Added flag validation to `pptx slides show` and `pptx slides selectors`. `slides show` accepts the documented include flags; `slides selectors` rejects them and other unknown flags.

Files touched: `src/cli_dispatch.rs`, `tests/rust_contract_smoke/utility.rs`.

Tests added: `pptx_slides_show_and_selectors_reject_unknown_flags`.

Behavior changes: typoed slide flags now fail instead of silently showing slide 1.

### CLI-9

Status: fixed.

MCP notifications now receive no response, and MCP errors use JSON-RPC codes with the original CLI code preserved in `error.data.exitCode`.

Files touched: `src/mcp.rs`, `tests/rust_contract_smoke/agent_surface.rs`.

Tests added: `mcp_stdio_parse_errors_notifications_and_error_codes_are_json_rpc_compliant`.

Behavior changes: `notifications/initialized` is silent; unknown MCP methods report `-32601`.

### CLI-10

Status: fixed.

Malformed JSON lines in MCP and serve stdio now emit a JSON-RPC parse error with `id:null` and keep the loop alive.

Files touched: `src/mcp.rs`, `src/serve.rs`, `tests/rust_contract_smoke/agent_surface.rs`, `tests/rust_contract_smoke/serve.rs`.

Tests added: `mcp_stdio_parse_errors_notifications_and_error_codes_are_json_rpc_compliant`, `serve_stdio_parse_errors_are_json_rpc_errors_and_loop_continues`.

Behavior changes: one bad stdio line no longer kills the long-running server process.

### CLI-11

Status: fixed.

Global JSON flags are now normalized in a single pass, so trailing `--json` behaves like leading `--json` and is stripped before leaf dispatch. Leading `--format=json` is also accepted as a global flag; post-command `--format` is preserved for commands that use it as a local option.

Files touched: `src/main.rs`, `tests/rust_contract_smoke/utility.rs`.

Tests added: `global_json_flags_normalize_before_and_after_command`.

Behavior changes: trailing `--json` no longer produces arbitrary unsupported-command or unknown-flag errors.

### CLI-12

Status: fixed.

`doctor --only` now validates requested check ids against the catalog and returns `invalid_args` listing valid ids when an unknown id is requested.

Files touched: `src/doctor.rs`, `tests/rust_contract_smoke/utility.rs`.

Tests added: `doctor_rejects_unknown_only_check_ids_and_proof_levels_are_local`.

Behavior changes: typoed doctor check ids no longer report healthy with zero checks run.

### CLI-13

Status: fixed.

Shell completion now includes the dispatched top-level commands `agent`, `agent-triage`, `diff`, and `template`.

Files touched: `src/completion.rs`, `tests/rust_contract_smoke/utility.rs`.

Tests added: `completion_includes_dispatched_top_level_commands`.

Behavior changes: generated completions now cover those top-level commands.

### CLI-14

Status: fixed.

Doctor proof-level metadata for `repair-conformance` now requires the local `binary` check rather than the Windows-only `office-edit-smoke` check.

Files touched: `src/doctor.rs`, `tests/rust_contract_smoke/utility.rs`.

Tests added: `doctor_rejects_unknown_only_check_ids_and_proof_levels_are_local`.

Behavior changes: Linux agents are no longer told that pure-Rust conformance proof depends on the Windows Office smoke check.
