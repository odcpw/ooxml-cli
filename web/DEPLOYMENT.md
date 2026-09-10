# cx43 workbench

Deployed 2026-09-10 over Tailscale SSH as `root@cx43`.
Public URL: https://ss.odc.pw/ooxml/.

- Service: `ooxml-workbench-v2.service`, enabled at boot, running as `safetysecretary`.
- Release directory: `/opt/ooxml-releases/zw2`.
- Working directory: `/opt/ooxml-releases/zw2/source/web`.
- CLI: `/opt/ooxml-releases/zw2/ooxml`, Rust 0.1.0 built from repository `4ed9e17`.
- Environment: `/opt/ooxml-releases/zw2/runtime.env`, restricted permissions.
- Private access link: `/opt/ooxml-releases/zw2/access-link.txt`, root-readable only.
- Application data: `/home/safetysecretary/data/ooxml-workbench-v2-live`.
- Upstream port: 3594. Caddy strips `/ooxml` before proxying.

The user authorized a fresh application database. Qualification used a separate
`ooxml-workbench-v2` data directory; it retains test artifacts. Production started
empty after successful qualification. The existing API credential and model
selection were retained; email, OAuth and development sign-in are disabled.

To rotate access, generate a new 32-byte base64url secret, put its SHA-256 hex
digest in `OOXML_ACCESS_KEY_SHA256`, securely replace the saved link, and restart
the v2 service. This revokes both the old link and sessions created by it. Never
commit the environment or access link.

The old service and its files remain available for rollback. The prior Caddy
configuration is `/opt/ooxml-releases/zw2/Caddyfile.before`. Restore only the
OOXML upstream to `127.0.0.1:3584` and validate/reload Caddy; preserve changes to
other sites if the proxy configuration has since changed.

Qualification covered clean npm install, build, typecheck, npm audit, dependency
pins, private access/CSRF/revocation, streaming, structured input contracts,
cross-user document and thumbnail isolation, DOCX/XLSX download and strict checks,
and a real model-backed PPTX edit with exact text readback, strict validation,
thumbnail rendering and download through the public proxy. Browser checks used
Chrome at 1440×1000 and 390×844; sign-in, upload, preview and streamed chat passed
without JavaScript errors. LibreOffice rendering is not desktop Office proof.

For code changes, deploy an isolated release directory, run `npm ci`,
`npm run typecheck`, `npm run build`, `npm run verify:stack` and the focused
tests, then qualify the candidate against its own data before changing Caddy.
`smoke:agent` accepts `OOXML_WEB_ACCESS_TOKEN`; load it without printing it.
