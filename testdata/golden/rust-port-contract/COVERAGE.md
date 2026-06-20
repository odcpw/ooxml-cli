# Rust Port Contract Coverage

| Surface | Contract Frozen | Evidence |
| --- | --- | --- |
| CLI binary | stdout, stderr, and exit codes for success and JSON error cases | `baseline.json` `cli` |
| PPTX mutation | `pptx replace text` publishes a changed deck | `baseline.json` `mutation.edit` |
| Validation | strict validation of the changed deck | `baseline.json` `mutation.validate` |
| Render | deterministic render manifest shape with mocked render tools | `baseline.json` `mutation.render` |
| Verify | validation/render/diff envelope when render is unavailable | `baseline.json` `mutation.verify` |
| Serve | JSON-RPC open, op, inspect, validate, plan, commit, abort | `baseline.json` `serve.flow` |
| MCP | initialize, tools, resources, command resource, session tools | `baseline.json` `mcp` |
| Web smoke | smoke scripts route readback through `OOXML_BIN` | `baseline.json` `webSmoke` |
| Apply batch orchestration | Go-vs-Rust dry-run plan, real XLSX mutation/readback, session-owned arg rejection, and strict validation/readback of saved output | `tests/rust_contract_smoke/agent_surface.rs` `apply_*` |
| PPTX placement | `pptx add-textbox` and `pptx place image` saved output, dry-run, readback, error envelopes, and strict/Open XML SDK validation of proof decks | `tests/rust_contract_smoke/pptx.rs` `pptx_add_textbox_*`, `pptx_place_image_*` |
| PPTX layout and slide authoring | `pptx layouts clone`, `pptx masters add-placeholder`, `pptx clone-slide`, and `pptx new-slide-from-layout` saved output, dry-run, readback, error envelopes, and strict/Open XML SDK validation of proof decks | `tests/rust_contract_smoke/pptx.rs` `pptx_layout_*`, `pptx_slides_*`, `pptx_layout_slide_authoring_*` |
| PPTX chart leaf commands | Go-vs-Rust differential tests for `pptx charts list/show`, saved-output readback/strict validation for chart style edits, `set-axis`, `convert-type`, and `copy-style`, plus representative error envelopes | `cargo test --test rust_contract_smoke pptx_chart -- --nocapture` |
| Focused PPTX table mutation | `pptx tables set-cell` saved output, dry-run, text-file, readback, and error envelopes | `cargo test --test rust_contract_smoke pptx_tables_set_cell` |
| Focused PPTX table columns/XLSX update | `pptx tables insert-col`, `delete-col`, and `update-from-xlsx` saved output, readback, strict validation, dry-run, source guards, and error envelopes | `cargo test --test rust_contract_smoke pptx_tables_column`, `cargo test --test rust_contract_smoke pptx_tables_update_from_xlsx` |
| XLSX filters/sorts | Go-vs-Rust differential tests for `xlsx filters-sorts show`, direct `set-autofilter`, saved readback, dry-run, error behavior, table target, and serve inspect | `tests/rust_contract_smoke/xlsx.rs`, `tests/rust_contract_smoke/serve.rs` |
| XLSX chart leaf commands | Go-vs-Rust differential tests for `xlsx charts list/show`, dry-run parity and saved-output readback/strict validation for chart style edits, `convert-type`, `copy-style`, and `set-axis`, plus representative error envelopes | `cargo test --test rust_contract_smoke xlsx_charts` |
| XLSX pivot/table formatting | Go-vs-Rust differential tests for `xlsx pivots list/show/create`, generated readback commands, dry-run and error behavior, strict validation of saved outputs, and promoted `xlsx tables set-column-format` capability | `cargo test --test rust_contract_smoke xlsx_pivots`, `cargo test --test rust_contract_smoke xlsx_tables_set_column_format` |
| XLSX data validations | Go-vs-Rust differential tests for `xlsx data-validations list/show/create/update/delete`, saved readback commands, dry-run, guard/error behavior, and strict validation of saved XLSX outputs | `tests/rust_contract_smoke/xlsx.rs` `xlsx_data_validations_*` |
| DOCX image mutation | Go-vs-Rust saved replace/insert, strict validation/readback, dry-run, stale hash, and missing-target errors | `tests/rust_contract_smoke/docx.rs` `docx_images_replace_insert_match_go_oracle` |
| Focused PPTX table row mutation | `pptx tables insert-row` saved output, `pptx tables show` readback, dry-run, and error envelopes | `cargo test --test rust_contract_smoke pptx_tables_insert_row` |

Out of scope for this Linux-local freeze:

- Microsoft Open XML SDK validation.
- Desktop Microsoft Office COM open proof.
- Real LibreOffice/pdftoppm image bytes.

Those remain compatibility proof gates, not Rust-port contract fixtures.
