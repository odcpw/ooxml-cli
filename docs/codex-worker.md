# Private slide worker

The web workbench runs one Codex App Server worker at a time. New requests go
to `/api/threads/:id/agent`. Flue routes are read-only compatibility routes for
historical conversations; Flue does not execute new slide jobs. Its existing
build/server adapter is retained.

## Runtime

- Pin `@openai/codex` to **0.154.0**. The dynamic-tool protocol is experimental;
  regenerate/check the protocol and run the real translation smoke when upgrading.
- `OOXML_CODEX_BIN`: absolute path to the pinned `codex` executable.
- `OOXML_CODEX_MODEL`: defaults to `gpt-6-astra`; reasoning effort is `low`.
- `OPENAI_API_KEY`: existing server API credential; never sent to the browser.
- `OOXML_SKILL_PATH`: optional absolute skill path; defaults to
  `../skills/ooxml/SKILL.md` relative to the web working directory.
- `OOXML_BIN`: the current Rust binary, used by the same validated document
  tools as before. The agent receives the OOXML skill in its instructions.

Run the website under its existing unprivileged service account. Codex is a
child process with a separate home and empty working directory, read-only
sandbox, shell/apply-patch disabled, and thread-scoped dynamic tools. Document
mutations stay in the host's existing stage/validate/publish seam. Only the
source document may be edited; references and templates are read-only inputs.

## Persistence and completion

`OOXML_WEB_DATA_DIR/codex-jobs.db` stores job admission, status, slide checkpoints,
ordered browser events and cumulative usage. `codex-home/` stores Codex threads
and API authentication; keep it private to the service account. Back up these
along with the existing `threads/`, library, auth data and historical `flue.db`.

An accepted job runs independently of the browser. The browser polls saved
events; opening a running job attaches to that same job instead of resubmitting
it. Only one active job per thread is allowed, and the server processes the
queue serially. On service restart, queued/running jobs are recovered. A
running job resumes its Codex thread and inspects current file versions before
continuing, rather than replaying old tool calls.

For translations, the worker inventories the source slides. The agent records
reviewed slides after saving and reading back each batch. Completion requires
a new source version, every slide recorded as reviewed, and a host-side strict
validation pass. This is coverage/structural evidence, not an independent
linguistic quality or Microsoft Office compatibility proof. Image-only slides
can be recorded as unchanged with a reason; translating text embedded in
pictures is not automatically guaranteed.

If a model turn ends too early, the host asks it to continue. Three turns
without version/checkpoint progress, 100 continuation rounds, or a 60-minute
turn timeout produce an explicit failed status while retaining saved edits.
There is no silent declaration of success after inspection. Template work
requires a new validated source version; the agent must disclose unsupported
layout mapping and partial conversions.

Usage snapshots are upserted, not added repeatedly on browser refresh. Astra
estimates apply the configured rates to each request's usage delta, including
cached input and the large-context tier. Historical Flue costs remain included.
Unsupported model prices are shown as unknown. These are estimates, not invoices.

## Verification and deployment

Run `npm run typecheck`, `npm run build`, and
`OOXML_BIN=/absolute/ooxml node --test scripts/*.test.mjs` in `web/` on Linux.
The Codex tests cover durable queue/checkpoint recovery, duplicate admission,
ordered event replay, polling without SSE and owner-isolated cost accounting.
Also exercise a real translation through the website, close/reopen the job,
and check the produced deck with the Rust CLI.

Deploy the tested commit, install the pinned Codex executable, set its absolute
path in the existing service environment, build, and restart only the slide
workbench service. Do not restart the other SafetySecretary services or Caddy.
Rollback uses the preceding application commit and leaves saved data intact.
