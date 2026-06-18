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

Out of scope for this Linux-local freeze:

- Microsoft Open XML SDK validation.
- Desktop Microsoft Office COM open proof.
- Real LibreOffice/pdftoppm image bytes.

Those remain compatibility proof gates, not Rust-port contract fixtures.
