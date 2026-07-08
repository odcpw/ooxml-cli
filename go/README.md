# Legacy Implementation

The active `ooxml` CLI is the Rust binary at the repository root.

This directory contains the deprecated implementation kept as source history.
New product work and proof work should land in Rust. The Rust contract harness
does not build or run this directory as an oracle; use Rust-native tests,
goldens, release real-file traces, and optional `OOXML_RUST_BASELINE_BIN`
cross-version checks instead.
