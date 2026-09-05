# Flue agentic layer: audit and upgrade proposal

Audit: 2026-09-05, bead `ooxml-w2-yxg.10`, SapphireGrove.
Source baseline: `6af3842b91557c79380229a989746d33339401ba`.
This is a docs-only assessment of that committed snapshot; concurrent web smoke
and Wave 2 rework are excluded until their own receipts land. No secrets were
read, no model calls were made, and no production data was changed.

The web app is a working, upload-first Office editor with a partial bridge to
modern CLI authoring. Its beta Flue runtime, custom stream reader, and separate
file metadata store make upgrading more than a dependency bump. First preserve
and qualify current behavior; then migrate Flue and expose bounded authoring
contracts. CLI green gates alone do not establish web or model-loop parity.

## Inventory and boundaries

| Surface | Committed implementation and boundary |
|---|---|
| Runtime/build | [package.json](../web/package.json), [lock](../web/package-lock.json), [config](../web/flue.config.ts): runtime/SDK/CLI pinned to `1.0.0-beta.9`; Hono, Valibot, TypeScript; `flue dev/build --target node`, `node dist/server.mjs`. |
| Browser | [page.ts](../web/src/page.ts): upload, document selection, version/download/preview UI and chat; manually reads admission `streamUrl`/offset and SSE events. Office-file upload creates a thread; no empty-document authoring or Markdown/brand asset workspace. |
| Public routes | [app.ts](../web/src/app.ts): `/`, `/signin`, `/about`, `/privacy`, `/terms`, `/health`; root requires sign-in, health returns only `ok`. Public magic-link request/verify, Google/Microsoft OAuth start/callback, opt-in dev-session route. |
| Owned HTTP data | Same app: authenticated `/api/auth/me`, logout, thread list/detail, initial and per-thread upload, document select, render, document/version download and registered artifacts. Legacy version URLs reject ambiguous document versions. |
| Agent admission | `/flue/*` middleware plus [agent route](../web/src/agents/ooxml-editor.ts) checks user ownership before admitting the thread ID. The auto-mounted beta Flue router owns execution/streams; tools trust the admitted thread ID. New routes must retain this ownership boundary. |
| Prompt/model | Same agent imports [OOXML skill](../skills/ooxml/SKILL.md), sets `OOXML_FLUE_MODEL` or `openai/gpt-5.5`, medium thinking, compaction keeping 6000 recent tokens. It teaches inspect-before-edit, capability discovery, guarded generic mutations, template limitations and download links. Its explicit loop remains presentation/edit oriented. |
| Tool bridge | [ooxml-tools.ts](../web/src/shared/ooxml-tools.ts) registers 19 Valibot tools; [ooxml-actions.ts](../web/src/shared/ooxml-actions.ts) uses CLI subprocesses, per-call Serve sessions, and one-shot typed MCP calls. No remote MCP connection or resource-reading adapter. |
| Conversation storage | [db.ts](../web/src/db.ts), [runtime-paths.ts](../web/src/shared/runtime-paths.ts): Flue SQLite `flue.db` beneath configured data root. Separate from application document/version JSON and binary artifacts. |
| Document storage | [storage.ts](../web/src/shared/storage.ts), [fs-atomic.ts](../web/src/shared/fs-atomic.ts): owner-scoped thread JSON, immutable version files and render records, atomic file replacement; mutation queue is process-local. `publishNewVersion` strict-validates and rechecks selected document/version before publishing metadata. This is not a transaction across SQLite, JSON and artifacts. |
| Auth/storage | [auth.ts](../web/src/shared/auth.ts): hashed session/magic tokens in local JSON, 30-day sessions, 15-minute magic links, OAuth state/PKCE, CSRF/origin checks and rate limits. Google/Microsoft and email transports exist; real provider callbacks/delivery are not proved by local dev-transport smokes. |
| Resource bounds | [app.ts](../web/src/app.ts), [storage.ts](../web/src/shared/storage.ts): upload body limit, Office-extension and ZIP-directory/expansion checks. Actions cap subprocess duration/output (120 seconds/24 MiB defaults); this does not impose a per-user model spend or worker-concurrency budget. |
| Deployment | [web README](../web/README.md): single Node host with persistent data directory, absolute CLI binary, optional LibreOffice/poppler/fonts; base URL/path and trusted-proxy configuration. No checked-in web deployment manifest or automated restore drill. Local filesystem and subprocess assumptions preclude moving this app unchanged to an edge worker. |
| Observability/CI | CLI errors have a short reference plus server stderr detail; smoke scripts emit phase records. No application-wide cost/latency traces, durable-job correlation or readiness dependency checks. [CI](../.github/workflows/ci.yml) and [Makefile](../Makefile) do not run the web npm smoke suite in the hosted Rust gates. |

`web/.env` is outside this audit. Production configuration must disable dev
session/bypass modes; development mail logs contain authentication links and
must stay outside code, diagnostics and retained smoke artifacts.

## Evidence vocabulary and receipts

**Proven** below means the named existing smoke directly exercises the registered
Flue tool and asserts its result for a stated fixture. **Partial** means the
implementation exists but that tool lacks direct smoke proof, or the broader
workflow is incomplete. **Missing** means no first-class adapter exists; it does
not imply the underlying CLI operation is missing.

Evidence IDs link to executable definitions, not a blanket pass for every branch:

- **T**: [smoke-flue-tools.mjs](../web/scripts/smoke-flue-tools.mjs), `npm run smoke:tools`.
  Directly invokes status, filtered capabilities and typed check on minimal XLSX;
  asserts document ID, capability contract version, strict pass and zero errors.
  SDK is explicitly skipped, render disabled. Exactly **3/19 registered tools**.
- **A**: [smoke-agent-edit.mjs](../web/scripts/smoke-agent-edit.mjs), `npm run smoke:agent`.
  Credentialed model prompt requires capability, generic inspection/mutation and
  check events; checks a new downloaded PPTX version, strict validation and title
  readback. It requests preview but does not require the preview tool event or a
  successful render. A is **not run in this audit**; bead `ooxml-y7v` records the
  earlier credential blocker. Pending stream-parser work is outside this snapshot.
- **N**: [smoke-nonpptx.mjs](../web/scripts/smoke-nonpptx.mjs), `npm run smoke:nonpptx`.
  Uploads/selects/downloads DOCX and XLSX; requires the current non-PPTX preview
  refusal; checks downloads with CLI strict validation and direct typed MCP check.
  It does not invoke a Flue model or any build wrapper.
- **H**: [smoke-auth-isolation.mjs](../web/scripts/smoke-auth-isolation.mjs), `npm run smoke:auth`.
  Unauthenticated and cross-user rejection, missing CSRF, own download, logout
  replay; artifact isolation branch only runs if PPTX rendering succeeds.
- **B**: [smoke-auth-abuse.mjs](../web/scripts/smoke-auth-abuse.mjs), `npm run smoke:auth-abuse`.
  Bad origin/referer, invalid/replaced/reused magic links, unsupported upload.
  These assertions do not establish OAuth, distributed rate-limit or ZIP-bomb proof.

Fresh local receipts are recorded at the end. Historical `ooxml-y7v` comments
also cite beta.9 build, T/N/H/B and dependency checks at `a9421ae`; those receipts
are not evidence for Flue 2 or the unexecuted model loop.

## Every registered tool mapped to its backend

All names come from [the tool registry](../web/src/shared/ooxml-tools.ts); backend
functions are in [the actions module](../web/src/shared/ooxml-actions.ts).

| Flue tool | Backend / proof status |
|---|---|
| `get_thread_status` | Local document/version metadata and URLs. **Proven T**, one XLSX thread; multi-document behavior remains outside T. |
| `select_document` | Owner-admitted thread metadata selection. **Partial**: N proves HTTP selection, not tool invocation. |
| `get_ooxml_capabilities` | CLI `--json capabilities --for`; compact projection or full response. **Proven T**, `check` filter; A also requires its event. |
| `get_ooxml_command_help` | CLI normalized command `--help`. **Partial**: no direct existing smoke. |
| `build_presentation` | Typed MCP `build_presentation`, then web strict validation/version publish. **Partial**: zero build-wrapper executions in T/N/A. |
| `build_workbook` | Typed MCP `build_workbook`, same publish seam. **Partial**: zero build-wrapper executions in T/N/A. |
| `build_document` | Typed MCP `build_document`, same publish seam. **Partial**: zero build-wrapper executions in T/N/A. |
| `check_package` | Typed MCP unified proof; `openXmlSdk`, `failOn`, `render`. **Proven T** for read-only XLSX strict proof; N checks direct MCP for DOCX/XLSX, A requires the event. No fix path. |
| `inspect_current_with_ooxml` | Serve `open(dryRun) → inspect → abort`; adds selection IDs. **Partial**: A defines live coverage but has no audit pass. |
| `apply_ooxml_ops_to_current` | Serve `open → op[] → validate → commit`, then strict/CAS publish. **Partial**: A defines one title edit; no direct T coverage or broad command parity claim. |
| `inspect_current_document` | CLI `inspect`. **Partial**: no direct smoke. |
| `validate_current_document` | CLI strict `validate`. **Partial**: A/N validate downloaded files directly, not this wrapper. |
| `search_current_document_text` | CLI `find`. **Partial**: no direct smoke. |
| `show_current_presentation_slide` | CLI `pptx slides show --include-text` with optional bounds. **Partial**: A readback invokes CLI, not this tool. |
| `replace_text_in_current_document` | CLI `find --replace --to-ops`, then `apply` and publish. **Partial**: A deliberately excludes this shortcut. |
| `set_current_presentation_slide_shape_text` | CLI `pptx replace text` and publish. **Partial**: A deliberately excludes this shortcut. |
| `apply_template_to_current_document` | Extract/apply template tokens, optional style/chart work and publish. **Partial**: no web smoke of transfer; not the full brand-kit API. |
| `create_template_form_slide_from_current` | PPTX layout import/add/fill pipeline and publish; selection guards. **Partial**: no existing web smoke for layout or placeholder fidelity. |
| `render_current_presentation_preview` | CLI PPTX render, artifact registration/URLs. **Partial**: H exercises HTTP rendering conditionally; A requests tool rendering but does not require success. N proves DOCX/XLSX refusal. |

Thus direct registry smoke coverage is **3 proven, 16 partial, 0 missing out of
19 registered names**; this is invocation coverage, not feature completeness.

## Agent-facing gaps against Waves 1 and 2

[Bridge plan](bridge-plan-2026-09.md), [typed MCP](../src/mcp/typed.rs),
[Serve](../src/serve.rs) and [manifest](../src/command_manifest/core.rs) are the
source of truth. Rust tests such as [typed tool tests](../tests/mcp_typed_tools.rs)
prove their own layer; they do not exercise Flue ownership, publication or chat.

| CLI/MCP capability | Web gap and concrete consequence |
|---|---|
| 10 typed MCP tools | Only **4/10** have direct typed adapters (three builds/check). `edit_package`, `outline_package`, `validate_package`, `render_preview`, `find_text`, `replace_text` are **missing as typed adapters**. Generic/convenience paths cover parts of these, but do not publish their schemas or full options. |
| Typed build schemas | Web accepts opaque `specJson` strings; underlying MCP embeds nested schema and accepts `spec` or `markdown`, `dryRun`, `check`, `force`, session/output. Tool descriptions mention `resource://schema/...` without a resource-reading tool. Invalid nested input costs a tool round trip. |
| From-scratch builds | Wrappers require a selected existing file of exact `.pptx`, `.xlsx` or `.docx` family and publish its next version. Empty authoring threads, cross-family creation, and macro variants are unavailable. |
| Wave 2 Markdown workbook plus DOCX/PPTX Markdown | CLI `build --from-markdown` and typed MCP `markdown` exist. Web has no Markdown build input or Markdown upload/source editor; a user must rely on model-generated JSON. |
| Wave 2 `check --fix` | Typed MCP exposes `fix`, `dryRun`, output/in-place/backup and `maxRounds`; web check is read-only. Findings can suggest filesystem commands the web agent cannot execute directly. Add a separate guarded, version-publishing fix action rather than passing arbitrary output paths. |
| Batched typed editing | `edit_package` supports named operation results and `$ref` dependencies. Web parses each operation down to `command`/`args`, dropping IDs; it has no equivalent reference-resolution contract or agent-visible dry-run review. Generic Serve compatibility is bounded by Serve's supported command set. |
| Mutation envelope | [CLI envelope](../src/mutation_envelope.rs) includes destination, change list, readback/validate/conformance/check commands, warnings, aliases and proof state. Web returns `changed: true`, version/download URL, nested `apply`, `validate`, and sometimes `opResults`; fields may survive nested but are not normalized across tools. A boolean `changed` also differs from the CLI change array. |
| Structured errors and paths | MCP bridge turns error objects into prose, losing machine-readable code/valid fields. Error paths are scrubbed; successful raw CLI/MCP payloads are not uniformly translated to document/artifact handles. Absolute paths and CLI proof commands can reach the model. Path spelling and safe public serialization need explicit contracts. |
| Selection consistency | Generic mutations and form-slide creation accept expected IDs; build/replace/shape/template shortcuts lack that input. Publish-time selection/version checks exist, but cannot detect that an agent intended a different file before a tool began. Concurrent/retried work also needs stable operation IDs to prevent duplicate versions. |
| Wave 2 brand parity | [Brand behavior](brand-parity.md), [brand tests](../tests/brand_kit.rs): CLI accepts brand JSON/assets through builds and `template apply --brand`; web has token transfer but no brand-kit/schema/asset-ID input. Office-only uploads prevent a natural logo/brand.json workflow. |
| Recipes and discovery | [Recipes](../src/recipes.rs), `robot-docs recipes`, `robot-docs recipe <name>` and `capabilities --workflows` provide runnable instructions. Web capabilities can carry workflow metadata, but there is no dedicated recipe/schema fetch or thread-aware recipe execution. Do not mistake metadata discovery for execution parity. |
| Current PPTX preview contract | **Observed failure:** `renderCurrent` reads `thumbnails`, while [the shared renderer](../src/render.rs) emits `slides[].imagePath`. H produced a PNG/PDF on disk but registered zero thumbnails; its artifact assertions therefore skipped. The adapter also records `thumbnails-manifest.json`, which this renderer does not write. Fix and require a nonempty, downloadable preview in P0. |
| Cross-family previews | Typed `render_preview` supports three families; web preview deliberately accepts PPTX/PPTM only. N currently pins that refusal, so expanding preview requires an explicit test-contract change and DOCX/XLSX visual evidence. |

## Proposed phases and exit evidence

Estimates are engineering days for one engineer familiar with this repository,
including review and tests, excluding user decisions, provider procurement and
Office COM availability. They are planning ranges, not delivery commitments.
Sequence P0 → P1 → P2; settle storage/auth decisions in P0, then P3/P4 before
production rollout. Do not combine a runtime migration with CLI feature changes.

| Phase | Work / estimate | Exit evidence and main risks |
|---|---|---|
| P0: freeze baseline | **1–2 days**. Record exact CLI/web/lock SHA; repair preview response mapping and assert artifacts; retain current contracts; obtain credentialed A receipt and add direct wrapper scenarios for builds, stale selection and failures. Decide history/hosting/budgets. | T/N/H/B plus model A, strict/readback proof and declared render availability; redacted CI artifacts. Model nondeterminism and unresolved beta stream parsing may expose baseline defects. |
| P1: Flue 2 migration | **4–7 days**, plus **2–5** if history export/reseed is required. Follow the concrete path below in an isolated deployment; preserve thread ownership and tool names. | Build/typecheck plus baseline smokes; admission, reconnect, abort, restart and duplicate-delivery tests. Restore old deployment from untouched data copy. Main risk: reset-only conversation schema and changed events/routes. |
| P2: bounded tool parity | **6–10 days**. Typed schemas/resource lookup; explicit document/version guards; native Markdown builds/new documents; edit references/dry run; versioned check-fix; brand/asset handles; recipe discovery; common result/error envelope. | Per-tool direct tests, hostile/unknown fields, stale selection, no-publish failures, check-fix readback, all-family branded Markdown outputs through strict/schema checks; model task suite with declared success denominator. Asset paths require confinement, not just prompt instructions. |
| P3: durable application data/auth | **4–7 days single-host**, or **9–15 shared-service** alternative. Define retention, backups/deletion, idempotent publication and orphan cleanup; couple job/version status. Keep SQLite/local data for one worker or introduce transactional shared metadata, object storage and a queue for replicas. Configure chosen auth provider and tenant policy. | Restart/restore drill, duplicate mutation suppression, owner checks on every stream/download, real OAuth/email callback tests and multi-worker race tests if selected. Export/reseed and rolling deploys cannot silently invalidate active work. |
| P4: deploy and operate | **3–5 days**. Reproducible Node/CLI/render image or service, persistent volume/DB and secrets setup, proxy/base-path tests; concurrency/time/spend limits, dependency readiness and structured traces. | Staged deployment/rollback, tool/LLM latency and cost attribution, cancellation/timeout load checks, backups restored, redacted diagnostics. Rendering resource contention and document content in logs are material risks. |

Baseline through single-host rollout: **18–31 engineering days**, plus optional
history work. Shared-service P3 makes it **23–39**, plus history. Scope can shrink
by deferring recipe execution or DOCX/XLSX preview, but record those as explicit
product gaps; never count CLI tests as their web acceptance evidence.

### Concrete Flue version path

On 2026-09-05, `npm view @flue/runtime version dist-tags repository --json`
reported latest `2.0.3`; repository pins beta.9. Check the chosen runtime, SDK,
CLI and new Vite package versions together before installing; their release
numbers need not match. Pin compatible versions and review the entire lock diff.
The [official Flue 2 announcement](https://flueframework.com/blog/flue-2/)
describes the hooks, Vite and conversation-client redesign.

The [official beta.9 migration guide](https://flueframework.com/docs/guide/migration/)
explicitly documents a reset-only persisted-state boundary, with no in-place
beta database upgrade. Preserve/export beta conversations if required before
reseed; keep document JSON/binaries separate and test their ownership links.
Then replace build/dev scripts with Vite and `@flue/vite`, migrate the agent to
an exported hooks function, mount explicit routes with ownership middleware,
and replace the browser/smoke admission and event parsing with the supported
conversation client. Retest skill loading, compaction and model options.
Keep the old database and deployment for rollback; never let the new runtime
open the only beta-state copy. These are upstream requirements, not proof that
this application's history can already be migrated.

Do not automatically mount every raw MCP tool into Flue 2. The current Rust
server is a local stdio subprocess; remote MCP support does not remove the need
for authorized document IDs, controlled artifact paths and version publication.

## Questions the user must answer before implementation

1. **Hosting:** Is the target one persistent Node host, containers on a managed
   service, or an edge frontend with a separate execution service? Which region,
   domain/base path, peak concurrent users/jobs and uptime target are required?
2. **History:** May existing Flue conversations be retired at cutover, or must
   transcripts and resumable work survive? Which files/versions must remain
   downloadable, and what maintenance window is acceptable?
3. **Authentication:** Keep email magic links, Google, Microsoft/Entra, or use
   another identity provider? Personal accounts or organization tenants; public
   signup, allowlist or invitations; is tenant sharing required? Who configures
   OAuth callbacks and production mail delivery?
4. **Models:** Which provider/model(s) and credential ownership are approved?
   Are user documents allowed to leave the chosen region or reach fallback
   providers? Should credentials be application-owned or per user?
5. **Budgets:** What are the monthly infrastructure and model-spend ceilings,
   per-job/per-user limits, latency target, maximum file size and concurrent
   render allowance? Who receives alerts and may raise a limit?
6. **Data:** What retention/deletion periods, backup recovery point/time and
   encryption requirements apply to documents, chat, previews and traces?
   Must logs exclude document text and model prompts entirely?
7. **Product order:** Which first release is required: uploaded-file editing,
   blank/Markdown authoring, branded all-family output, automatic check-fix, or
   preview parity? May the agent publish safe fixes automatically, or must a
   user approve the proposed change list before publication?
8. **Acceptance:** Who supplies the credentialed deployment for model smokes,
   representative files and Office desktop follow-up? What task-set pass rate
   and cost ceiling define a successful rollout?

## Reproduction and fresh audit checks

Use a clean checkout at the audit SHA, isolated data and an absolute freshly
built CLI. Do not copy deployment environment files. The audit used installed
beta.9 dependencies through a symlink, not a fresh `npm ci` installation;
`verify:stack` checked the committed lock. Node was `26.8.1`, npm `11.19.0`.
A clean-install check remains part of P0.

```sh
# Repository root; keep builds on disk.
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_TARGET_DIR="$HOME/.cache/ooxml-cli-target/SapphireGrove-flue-audit"
mise exec rust@1.98 -- cargo build --bin ooxml
export OOXML_BIN="$CARGO_TARGET_DIR/debug/ooxml"
cd web
npm run verify:stack
npm run typecheck
npm run build
export OOXML_WEB_DATA_DIR="$HOME/.cache/ooxml-flue-audit-data"
export OOXML_WEB_BASE_URL=http://localhost:3597
# Run in a separate terminal with the same explicit configuration:
PORT=3597 APP_BASE_URL="$OOXML_WEB_BASE_URL" EMAIL_TRANSPORT=dev \
  OOXML_AUTH_DEV_BYPASS=0 OOXML_AUTH_DEV_SESSIONS=0 node dist/server.mjs
# Against that server:
npm run smoke:tools
npm run smoke:auth
npm run smoke:auth-abuse
npm run smoke:nonpptx
```

Fresh audit results on 2026-09-05:

| Check | Receipt / limit |
|---|---|
| CLI build; web stack/typecheck/production build | Passed at the source baseline. Lock-stack validation is not a fresh vulnerability/advisory audit. |
| T: tools | Passed, 3/19 tools; XLSX strict/structural passed, schema/visual skipped. |
| N: non-PPTX | Passed, 2/2 DOCX/XLSX downloads strict/structural clean; schema/visual skipped; expected preview refusals. |
| H: auth isolation | Passed ownership/CSRF/logout branches. **Artifact branch skipped:** PNG/PDF generated, but web registered zero thumbnails due to response drift described above. |
| B: auth abuse | Passed all scripted origin/token/unsupported-upload assertions. |
| Clean Rust gate | `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo check --all-targets` all passed in the clean audit worktree via Rust 1.98. |
| Document checks | Under 400 lines, LF, all 36 local source links resolve; no code changes. |

`smoke:agent` was not run; neither Office compatibility nor successful model
execution is claimed. Any tool-availability-dependent proof must name the
actual executed branch instead of treating a skipped branch as passed.
