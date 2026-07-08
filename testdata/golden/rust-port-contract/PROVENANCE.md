# Rust Port Contract Provenance

This directory stores the legacy frozen contract fixture that was used during
the Rust port. It is retained as a historical compatibility snapshot, not as an
active oracle.

Generated on: 2026-06-18

Original fixture source: historical legacy checkout on this branch.

Original golden generator: historical porting generator. Active regeneration
should use Rust-native contract or release-trace goldens instead of legacy
code.

Primary golden:

- `baseline.json`

The active harness scrubs temp files, serve/MCP working-copy paths, and session
IDs. It intentionally keeps stable user-facing handles, selectors, JSON field
names, exit codes, command strings, and readback envelopes because those are
part of the Rust compatibility target.

Web smoke note:

- `make web-smoke-agent` and `make web-smoke-nonpptx` build `./ooxml` first and
  pass it to the web smoke scripts through `OOXML_BIN`.
- Those targets require a running web server and do not start one themselves.
