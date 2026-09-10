# Flue upgrade — 2026-09-10

The workbench moves from Flue 1.0.0-beta.9 to 2.0.3, with Vite 8.2.2.
All four Flue packages are pinned consistently. npm lists 2.0.4 as latest,
but its CLI and Vite packages publish invalid workspace:* dependencies;
both ordinary installation and explicit dependency overrides fail with
EUNSUPPORTEDPROTOCOL. Version 2.0.3 is the latest installable stable release.
No third-party package contents were patched.

The migration uses Vite build/dev, an explicitly mounted agent, synchronous
agent hooks, and tools receiving data and returning output envelopes. HTTP
messages use the delivered-message shape. The existing projected stream
contract remains supported; completion now requires the matching submission
settlement, and the browser reports disconnects and failures accurately.

Sources: [official migration guide](https://flueframework.com/docs/guide/migration/),
[HTTP routing contract](https://flueframework.com/docs/guide/routing/), and the
installed npm runtime/SDK source for 2.0.3.

Verification: clean npm ci succeeds; npm audit reports zero vulnerabilities;
typecheck, production build, dependency-stack verification, and seven stream
regression tests pass. Local tool smoke passed status and capabilities but
Windows storage path containment rejected check_package; Linux qualification
and the credentialed full agent edit/render/download smoke are deployment gates.

Flue beta storage is incompatible with the new schema. Deployment must use
fresh Flue storage or explicitly export/reseed prior conversations. The user
authorized fresh deployment data for this installation.

Live qualification exposed string-only JSON argument schemas despite object examples.
The tool boundary now accepts native objects for argsJson/specJson and arrays
for opsJson, alongside encoded strings, preserving the existing action validation.
Three input regression tests pass; the generated model JSON schema advertises
these native alternatives. Tool smoke now exercises both inspection forms and
a native operation array with strict validation/readback. The artifact isolation
smoke now fails when rendering produces no thumbnails instead of skipping proof.

Linux smoke:tools passed on cx43: five of nineteen tools exercised, including native/encoded inspection equivalence, structured mutation and readback, and strict/structural package proof. Inspection comparison normalizes only the per-process working-file path in file and generated command fields.

Public credentialed agent smoke passed at https://ss.odc.pw/ooxml on 2026-09-10 04:28 UTC: private-link sign-in, upload, streamed model tool calls, version v0002, preview with one thumbnail, download, strict validation with zero errors, and exact requested title readback. Stream URL normalization preserves the mount prefix and origin; non-SSE responses fail explicitly. Ten stream regression tests pass.
