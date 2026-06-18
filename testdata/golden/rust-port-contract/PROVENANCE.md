# Rust Port Contract Provenance

This directory freezes the Go `ooxml-cli` implementation as the reference contract
for a future Rust implementation.

Generated on: 2026-06-18

Reference implementation: current Go checkout on this branch.

Golden generator:

```bash
UPDATE_GOLDENS=1 go test ./internal/cli -run TestRustPortContractGolden -count=1
```

Primary golden:

- `baseline.json`

The harness scrubs temp files, serve/MCP working-copy paths, and session IDs.
It intentionally keeps stable user-facing handles, selectors, JSON field names,
exit codes, command strings, and readback envelopes because those are part of
the Rust compatibility target.

Web smoke note:

- `make web-smoke-agent` and `make web-smoke-nonpptx` build `./ooxml` first and
  pass it to the web smoke scripts through `OOXML_BIN`.
- Those targets require a running web server and do not start one themselves.
