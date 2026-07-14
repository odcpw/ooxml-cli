# Release Readiness — 2026-07-11 (updated 2026-07-14)

## Recommendation

The Rust product line is established on `master`, and the release candidate now
has current desktop Word, Excel, and PowerPoint proof. The remaining work is
final qualification and release operations: push this polish commit through
hosted CI, run the optional model-backed web smoke when credentials are
available, and make an explicit tag/release decision. No public release has
been created.

## Evidence

- The branch has one production command metadata authority: a typed 309-command manifest. The direct CLI remains the explicit authority for positional grammar, flags, defaults, and error precedence.
- Capability JSON, leaf help, shell completion, MCP resources, and Serve inspect/mutation namespaces have committed equality or bidirectional contract proofs.
- The crate has a four-line binary adapter and one doc-hidden external Rust entry point; implementation lives in the library.
- Local qualification passed formatting, warnings-as-errors clippy, 504 tests,
  web typecheck/build and credential-free smokes, strict/conformance validation,
  and LibreOffice rendering.
- GitHub Actions run `29135760620` passed the main Rust gate, Linux, macOS, Windows portable tests, and Windows Open XML SDK/conformance smoke.
- Legion release proof passed 64/64 normal edit scenarios: 23 XLSX, 25 PPTX,
  and 16 DOCX outputs all reached `microsoft-office-com-open` on Microsoft
  Office 16.0 build 20131 after strict validation, conformance checks, and Open
  XML SDK validation.
- Legion VBA proof passed 19/19 scenarios, including 13/13 desktop Office opens
  across XLSM, PPTM, and DOCM outputs, with conformance and Open XML SDK
  validation enabled.
- The Cloudflare/Miniflare/Wrangler development stack was refreshed within the
  existing Flue beta contract. `npm audit` and `npm audit --omit=dev` now both
  report zero vulnerabilities.
- The web smoke pass found and fixed an incorrect default fixture path, a
  cross-user render route that returned 500 instead of privacy-preserving 404,
  and LibreOffice chatter corrupting `pptx render --json`. All three
  credential-free smokes pass, and the renderer has a focused regression test.

## Release holds and residual risks

- The optional model-backed web agent smoke was not run because no
  `OPENAI_API_KEY` is configured on either t14 or Legion. Deterministic web
  build and non-model smokes passed.
- The Office proof was run through interactive scheduled tasks because SSH
  commands execute in Windows session 0. First-run Office UI was completed,
  temporary tasks were removed, Excel add-in settings were restored, and no
  test Office process remains.
- No tag, GitHub release, package publication, or public release artifact has been created.

## Proposed release pass

1. Push the polish commit and require the full hosted CI matrix to pass.
2. Run `web/scripts/smoke-agent-edit.mjs` with model credentials in the intended
   deployment environment, or explicitly defer that optional integration gate.
3. Verify the release workflow and installer/asset expectations, then make an
   explicit version/tag/release decision.
