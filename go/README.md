# Go Reference Implementation

The active `ooxml` CLI is the Rust binary at the repository root.

This directory contains the deprecated Go implementation kept as source history
and an oracle reference. New product work should land in Rust. If a Rust
contract test needs the Go oracle, it should use the frozen
`codex/ooxml-go-reference` branch through the existing
`OOXML_GO_ORACLE_REF` / `OOXML_GO_ORACLE_DIR` mechanism rather than treating
this directory as the normal development target.
