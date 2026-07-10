# Rust 0.1.0 Release Integration Handoff

## Scope

- candidate branch: `integrate/rust-master-0.1.0`
- candidate version: `0.1.0`
- product branch after acceptance: `master`
- legacy Go status: reference material only, not a release oracle
- detailed audited finding disposition: [`FIXNOTES.md`](../../FIXNOTES.md)

## Integrated Evidence

- Remote `master` worksheet-form support and limited XLSM `.frm` package/list/extract support were reconciled with the current Rust product line. Runtime-loadable generated UserForms, `.frx`, MSForms designer type-info, and binary-backed controls remain explicitly unsupported.
- CLI/serve/MCP drift was repaired, including real selector-based `pptx replace text`, JSON-RPC notification/error behavior, global flag normalization, and a dispatcher coverage guard for every `opCompatible=true` capability.
- OOXML entity handling now preserves named and numeric references across DOCX, PPTX, and XLSX reads and unrelated writes.
- OPC relationship/content-type checks now handle URI decoding, ASCII-case rules, explicit internal targets, malformed relationship diagnostics, and legal existing override serializations. DOCX diff includes relevant secondary parts and media.
- VBA hardening covers Windows-1252 extensions, MS-OVBA chunk boundaries and decompression ceilings, CFB FAT/mini-FAT cycle rejection, and local smoke-argument validation before platform availability errors.

## Proof Contract

- `make check-ci` is the complete Rust gate and runs `cargo test --all-targets`.
- Historical `*_baseline_*` helper names mean current-subject repeatability by default. Intentional differential runs set `OOXML_RUST_COMPARISON_BIN`; the harness rejects a comparison path that resolves to the current executable.
- The tag-triggered release workflow requires `vX.Y.Z` to match `Cargo.toml`, reruns format/clippy/all-target tests, builds Linux x86_64, macOS arm64/x86_64, and Windows x86_64 archives, and publishes `SHA256SUMS` with the GitHub Release.
- Windows Open XML SDK and desktop Office proof remain separate compatibility gates. A successful Linux Rust gate must not be reported as Office-open or macro-execution proof.

## Release Sequence

1. Land the reconciled candidate on `master` only after the full local Rust gate is green.
2. Run the applicable Windows schema and Office COM gates against that exact commit and binary.
3. Push `master`, create annotated tag `v0.1.0`, and push the tag.
4. Verify all four platform assets and `SHA256SUMS` on the GitHub Release.
5. Install the released binary and repo skill only after the published asset smoke passes.

## Open Proof

- The new workflow is not proven until it runs from the pushed `v0.1.0` tag.
- Windows Office/Open XML SDK validation must be rerun after integration because the candidate combines previously independent form, entity, OPC, CLI, and VBA changes.

## Local Verification

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test --all-targets`: passed, 420 tests total (82 binary unit tests, 323 Rust contract tests, and 15 focused integration/golden tests).
- `make check-ci`: passed end to end, including the full all-target test gate and debug binary build.
- Conflict-sensitive PPTX replacement, DOCX secondary diff, OPC relationship, serve dispatcher, worksheet form, VBA codec/CFB, entity, and smoke-contract filters: passed.
- `RCH_DISABLED=1 cargo build --release --locked --bin ooxml`: passed; `target/release/ooxml --json version` reported `0.1.0`.
- Release workflow YAML parsed locally, and its `v0.1.0` tag/version guard passed locally. Cross-platform runner execution remains tag-time proof.
