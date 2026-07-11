# Release Readiness — 2026-07-11

## Recommendation

The typed-command-manifest branch is ready to become `master`. Do not create a public release yet. Run a separate release-preparation pass after Legion remote access is restored so the current candidate can receive desktop Word, Excel, and PowerPoint open proof.

## Evidence

- The branch has one production command metadata authority: a typed 309-command manifest. The direct CLI remains the explicit authority for positional grammar, flags, defaults, and error precedence.
- Capability JSON, leaf help, shell completion, MCP resources, and Serve inspect/mutation namespaces have committed equality or bidirectional contract proofs.
- The crate has a four-line binary adapter and one doc-hidden external Rust entry point; implementation lives in the library.
- Local qualification passed formatting, check, warnings-as-errors clippy, documentation, debug and release builds, 503 tests, web typecheck/build and isolated smoke, strict/conformance validation, and LibreOffice rendering.
- GitHub Actions run `29135760620` passed the main Rust gate, Linux, macOS, Windows portable tests, and Windows Open XML SDK/conformance smoke.

## Release holds and residual risks

- Legion responds on Tailscale, but SSH and WinRM listeners are unavailable. The current commit therefore lacks a fresh desktop Office COM pass. Historical Office proof remains useful context but is not substituted for current-candidate proof.
- The optional model-backed web agent smoke was not run because no `OPENAI_API_KEY` was available. Deterministic web build and non-model smoke passed.
- `npm audit` reports one low and four high transitive advisories in the Cloudflare/Flue development stack. The fix crosses a pinned Flue beta boundary and should be handled as a focused dependency upgrade, not folded into this Rust architecture refactor.
- No tag, GitHub release, package publication, or public release artifact has been created.

## Proposed release pass

1. Restore Windows OpenSSH or WinRM on Legion and install the dedicated `t14` public key.
2. Build the exact candidate commit on Legion and run the repository's Office proof lanes.
3. Upgrade and requalify the Flue/Cloudflare dependency chain, or explicitly document why the development-only exposure is accepted for the release.
4. Run the credentialed agent smoke in the intended deployment environment.
5. Re-run release workflow dry checks, verify installation and checksums, then make an explicit tag/release decision.
