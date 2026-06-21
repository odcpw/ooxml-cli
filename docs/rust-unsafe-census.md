# Rust Unsafe Census

Date: 2026-06-21

Worker: P74

Worktree: `C:\Users\olidc\OneDrive\Desktop\Projects\ooxml-cli-wt-p74-rust-unsafe-census-20260621`

Branch: `p74/rust-unsafe-census-20260621`

Base: `origin/codex/ooxml-rust-port` at `0fcc7d22fe1cd03a783b924a6d9b675eae5e7f2c`

Scope: read-only census of Rust files under `src/` and `tests/`, plus a best-effort dependency surface note. This is a triage/audit-only pass in the spirit of `rust-unsafe-code-exorcist`; it intentionally did not create a full `.unsafe-audit/` directory.

## Summary

- In-tree source/test unsafe sites: none found.
- Rust files checked: 312 total, with 275 under `src/` and 37 under `tests/`.
- `src/` and `tests/` contain no literal `unsafe` keyword matches.
- No matches were found for direct unsafe forms, FFI extern blocks, inline assembly, or common unsafe-adjacent primitives in `src/` or `tests/`.
- No `#![forbid(unsafe_code)]`, `#![deny(unsafe_code)]`, or `#![allow(unsafe_code)]` attribute was found in `src/` or `tests/`.
- Follow-up integration added a Cargo lint guard: `[lints.rust] unsafe_code = "forbid"`.
- Follow-up integration installed nightly `miri`/`rust-src` and proved the pure ZIP I/O unit tests under Miri on Windows.
- Dependency-side unsafe is not proven clean. `cargo tree` is available, but `cargo-geiger`, `cargo-expand`, and `ast-grep` are not installed.

## Commands Run

### Setup and scope

```powershell
git status --short
git rev-parse --show-toplevel
git rev-parse --abbrev-ref HEAD
git rev-parse HEAD
git rev-parse origin/codex/ooxml-rust-port
git merge-base --is-ancestor 0fcc7d2 origin/codex/ooxml-rust-port
git worktree add -b p74/rust-unsafe-census-20260621 'C:\Users\olidc\OneDrive\Desktop\Projects\ooxml-cli-wt-p74-rust-unsafe-census-20260621' origin/codex/ooxml-rust-port
```

Result: the integration worktree was clean, `0fcc7d2` is an ancestor of `origin/codex/ooxml-rust-port`, and the isolated P74 worktree was created at `0fcc7d22fe1cd03a783b924a6d9b675eae5e7f2c`.

### Tool availability

```powershell
rg --version
ast-grep --version
cargo-geiger --version
cargo --version
rustc --version
cargo expand --version
cargo geiger --version
cargo +nightly geiger --version
```

Observed:

- `ripgrep 15.1.0` available.
- `cargo 1.96.0 (30a34c682 2026-05-25)` available.
- `rustc 1.96.0 (ac68faa20 2026-05-25)` available.
- `ast-grep` unavailable.
- `cargo-expand` unavailable (`cargo expand` reported no such command).
- `cargo-geiger` unavailable (`cargo geiger` reported no such command).
- `cargo +nightly geiger --version` also did not provide geiger output; rustup synced the nightly channel first, then Cargo reported no `geiger` command. This was not used as audit evidence.

### Rust file inventory

```powershell
rg --files src -g '*.rs'
rg --files tests -g '*.rs'
$src = if (Test-Path src) { (rg --files src -g '*.rs' | Measure-Object).Count } else { 0 }
$tests = if (Test-Path tests) { (rg --files tests -g '*.rs' | Measure-Object).Count } else { 0 }
"src_rs_files=$src"
"tests_rs_files=$tests"
"total_rs_files=$($src + $tests)"
```

Observed:

```text
src_rs_files=275
tests_rs_files=37
total_rs_files=312
```

### In-tree unsafe census

```powershell
rg -n --glob '*.rs' '\bunsafe\b' src tests
rg -n --glob '*.rs' '\bunsafe\s*\{' src tests
rg -n --glob '*.rs' '\bunsafe\s+fn\b' src tests
rg -n --glob '*.rs' '\bunsafe\s+impl\b' src tests
rg -n --glob '*.rs' '\bunsafe\s+trait\b' src tests
rg -n --glob '*.rs' 'extern\s+"[^"]*"\s*\{' src tests
rg -n --glob '*.rs' '\basm!\b|\bglobal_asm!\b' src tests
rg -n --glob '*.rs' '\bMaybeUninit\b|\btransmute\b|\bfrom_raw\b|\binto_raw\b|\bget_unchecked\b|\bnew_unchecked\b|\bassume_init\b|\bUnsafeCell\b|\bNonNull\b|\*const\b|\*mut\b' src tests
rg -n --glob '*.rs' '#!\[(forbid|deny|warn|allow)\(unsafe_code\)\]|#\[(forbid|deny|warn|allow)\(unsafe_code\)\]' src tests
```

Result: every command above returned no matches. In ripgrep terms, these commands exited with code 1 because the search completed and no matching lines were found.

## Source/Test Unsafe Result

No in-tree Rust unsafe sites were found in `src/` or `tests/`.

This means the checked tree has:

- 0 `unsafe { ... }` blocks.
- 0 `unsafe fn` declarations.
- 0 `unsafe impl` declarations.
- 0 `unsafe trait` declarations.
- 0 literal `unsafe` keyword matches.
- 0 Rust FFI extern blocks matching the checked form.
- 0 inline assembly macro matches.
- 0 matches for the checked unsafe-adjacent primitives (`MaybeUninit`, `transmute`, `from_raw`, `into_raw`, `get_unchecked`, `new_unchecked`, `assume_init`, `UnsafeCell`, `NonNull`, raw pointer type markers).

## Dependency Surface

Cargo reports one Rust workspace package:

```powershell
cargo metadata --format-version 1 --no-deps
```

Result: one package, `ooxml-rs-port v0.0.1`, with one binary target (`ooxml`) and one integration test target (`rust_contract_smoke`).

Direct dependency tree:

```powershell
cargo tree --all-features --depth 1
```

```text
ooxml-rs-port v0.0.1
|-- quick-xml v0.38.4
|-- regex v1.12.4
|-- serde_json v1.0.150
|-- sha2 v0.10.9
`-- zip v2.4.2
```

Full target-inclusive tree was available:

```powershell
cargo tree --all-features --target all
```

It resolved 45 packages. Notable transitive areas for a future dependency soundness pass include fast string scanning (`memchr`, `aho-corasick`, `regex-automata`), compression/checksum code (`zip`, `flate2`, `crc32fast`, `simd-adler32`, `zopfli`, `miniz_oxide`), hash table/storage internals (`hashbrown`, `indexmap`), CPU feature detection (`cpufeatures`, `libc`), and proc-macro support crates.

Duplicate dependency check:

```powershell
cargo tree --all-features --duplicates
```

Result: no duplicates to print.

Because `cargo-geiger` was unavailable, I also ran a rough text scan over resolved registry package sources:

```powershell
$meta = cargo metadata --format-version 1 | ConvertFrom-Json
$ids = @($meta.resolve.nodes | ForEach-Object { $_.id })
$pkgs = @($meta.packages | Where-Object { ($ids -contains $_.id) -and $_.source -like 'registry+*' } | Sort-Object name)
foreach ($pkg in $pkgs) {
    $dir = Split-Path -Parent $pkg.manifest_path
    $matches = @(rg -n --glob '*.rs' '\bunsafe\b' -- $dir 2>$null)
    if ($matches.Count -gt 0) {
        "$($pkg.name) $($pkg.version) unsafe_text_lines=$($matches.Count)"
    }
}
```

This is not equivalent to `cargo-geiger`: it counts source lines containing the token `unsafe` and can include comments, docs, tests, inactive cfg branches, macro internals, and non-reachable code. It is useful only as a dependency triage signal.

Packages with rough dependency-side `unsafe` token lines:

```text
adler2 2.0.1 unsafe_text_lines=1
aho-corasick 1.1.4 unsafe_text_lines=227
arbitrary 1.4.2 unsafe_text_lines=4
block-buffer 0.10.4 unsafe_text_lines=4
bumpalo 3.20.3 unsafe_text_lines=235
cpufeatures 0.2.17 unsafe_text_lines=9
crc32fast 1.5.0 unsafe_text_lines=6
crossbeam-utils 0.8.21 unsafe_text_lines=79
flate2 1.1.9 unsafe_text_lines=37
generic-array 0.14.7 unsafe_text_lines=78
hashbrown 0.17.1 unsafe_text_lines=492
indexmap 2.14.0 unsafe_text_lines=12
itoa 1.0.18 unsafe_text_lines=13
libc 0.2.186 unsafe_text_lines=430
log 0.4.32 unsafe_text_lines=8
memchr 2.8.2 unsafe_text_lines=333
miniz_oxide 0.8.9 unsafe_text_lines=3
proc-macro2 1.0.106 unsafe_text_lines=6
quick-xml 0.38.4 unsafe_text_lines=9
regex 1.12.4 unsafe_text_lines=1
regex-automata 0.4.14 unsafe_text_lines=57
serde 1.0.228 unsafe_text_lines=2
serde_core 1.0.228 unsafe_text_lines=2
serde_json 1.0.150 unsafe_text_lines=12
sha2 0.10.9 unsafe_text_lines=29
simd-adler32 0.3.9 unsafe_text_lines=36
syn 2.0.118 unsafe_text_lines=91
thiserror-impl 2.0.18 unsafe_text_lines=1
unicode-ident 1.0.24 unsafe_text_lines=2
zip 2.4.2 unsafe_text_lines=18
zmij 1.0.21 unsafe_text_lines=74
zopfli 0.8.3 unsafe_text_lines=4
```

Dependency conclusion: the application crate has no in-tree unsafe sites, but it does depend on crates that contain unsafe token lines. A full release-readiness claim therefore still needs dependency reachability and proof-obligation analysis, preferably with `cargo-geiger` and the skill's dependency-soundness workflow.

## Miri Follow-Up

After integrating this census, the main integration lane installed the nightly
Miri components:

```powershell
rustup component add --toolchain nightly-x86_64-pc-windows-msvc miri rust-src
```

The full unit-test binary is not currently a clean Miri target on Windows
because `conformance_invariants::relationships::tests::parse_relationship_part_classifies_zip_read_errors`
uses filesystem operations that hit Windows Miri unsupported syscalls
(`GetTempPathW` under isolation, then `CreateFileW` access-mode support with
isolation disabled). This is a Miri platform/tooling limitation for that test
shape, not an in-tree unsafe finding.

The pure ZIP I/O unit-test subset did pass:

```powershell
$env:MIRIFLAGS='-Zmiri-disable-isolation'
cargo +nightly miri test --bin ooxml zip_io::tests
```

Result:

```text
running 4 tests
test zip_io::tests::zip_archive_check_rejects_declared_part_oversize ... ok
test zip_io::tests::zip_archive_check_rejects_total_uncompressed_oversize ... ok
test zip_io::tests::zip_text_reader_rejects_declared_oversize_entry ... ok
test zip_io::tests::zip_text_reader_rejects_underdeclared_stream_oversize ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out
```

## Recommended Next Full Audit Step

Under `rust-unsafe-code-exorcist`, the next full step should be:

1. Get explicit user confirmation to create the full in-project `.unsafe-audit/` artifact directory.
2. Install or otherwise make available the missing audit tools: `ast-grep`, `cargo-expand`, and `cargo-geiger`.
3. Run a full `pre-release-soundness-gate` or `dependency-soundness` pass focused on dependency reachability, since the in-tree unsafe inventory is empty.
4. Keep the Cargo `unsafe_code = "forbid"` lint green in normal check/test gates.

## What Remains Unproven

- Macro-expanded unsafe was not checked because `cargo-expand` is unavailable.
- AST-level unsafe classification was not checked because `ast-grep` is unavailable.
- Dependency unsafe counts were not measured with `cargo-geiger`.
- The rough dependency text scan is not a reachability analysis and does not prove whether dependency unsafe is reachable through this crate's public behavior.
- Miri was run only on the pure ZIP I/O unit-test subset; the filesystem-heavy unit test needs a Miri-specific shim or refactor before the full unit-test binary can be a clean Windows Miri target.
- No `cargo-careful`, fuzzing, mutation testing, or formal verification was run.
- No full `rust-unsafe-code-exorcist` Phase 1-10 audit was performed.
- Dependency-side unsafe remains unproven without `cargo-geiger`/reachability analysis.
