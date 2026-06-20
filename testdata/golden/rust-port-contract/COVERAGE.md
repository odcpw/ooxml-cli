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
| Focused PPTX table mutation | `pptx tables set-cell` saved output, dry-run, text-file, readback, and error envelopes | `cargo test --test rust_contract_smoke pptx_tables_set_cell` |
| XLSX filters/sorts | Go-vs-Rust differential tests for `xlsx filters-sorts show`, direct `set-autofilter`, saved readback, dry-run, error behavior, table target, and serve inspect | `tests/rust_contract_smoke/xlsx.rs`, `tests/rust_contract_smoke/serve.rs` |
| XLSX data validations | Go-vs-Rust differential tests for `xlsx data-validations list/show/create/update/delete`, saved readback commands, dry-run, guard/error behavior, and strict validation of saved XLSX outputs | `tests/rust_contract_smoke/xlsx.rs` `xlsx_data_validations_*` |
| DOCX image mutation | Go-vs-Rust saved replace/insert, strict validation/readback, dry-run, stale hash, and missing-target errors | `tests/rust_contract_smoke/docx.rs` `docx_images_replace_insert_match_go_oracle` |
| Focused PPTX table row mutation | `pptx tables insert-row` saved output, `pptx tables show` readback, dry-run, and error envelopes | `cargo test --test rust_contract_smoke pptx_tables_insert_row` |

Out of scope for this Linux-local freeze:

- Microsoft Open XML SDK validation.
- Desktop Microsoft Office COM open proof.
- Real LibreOffice/pdftoppm image bytes.

Those remain compatibility proof gates, not Rust-port contract fixtures.
