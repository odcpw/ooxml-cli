# OOXML Flue Workbench

Small Flue 1.0 beta web workbench for uploading Office files, chatting with a
thread-scoped OOXML agent, and previewing PPTX/PPTM outputs.

## Run

```bash
npm install
npm run dev
```

Open `http://localhost:3583`.

Required local environment:

```bash
OPENAI_API_KEY=...
OOXML_FLUE_MODEL=openai/gpt-5.5
OOXML_BIN=ooxml
OOXML_WEB_DATA_DIR=../.flue-ooxml-web-data
APP_BASE_URL=http://localhost:3583
OOXML_WEB_BASE_PATH=
EMAIL_TRANSPORT=dev
```

`EMAIL_TRANSPORT=dev` writes magic links to
`../.flue-ooxml-web-data/auth/magic-links.jsonl`. For real login, configure
Microsoft or Google OAuth with:

```bash
MICROSOFT_OAUTH_CLIENT_ID=...
MICROSOFT_OAUTH_CLIENT_SECRET=...
MICROSOFT_OAUTH_REDIRECT_URI=https://your-host/api/auth/oauth/microsoft/callback

GOOGLE_OAUTH_CLIENT_ID=...
GOOGLE_OAUTH_CLIENT_SECRET=...
GOOGLE_OAUTH_REDIRECT_URI=https://your-host/api/auth/oauth/google/callback
```

When deploying below a path prefix, set `APP_BASE_URL` to the public URL with
that path, for example `https://ss.odc.pw/ooxml`. `OOXML_WEB_BASE_PATH` can be
set explicitly, but defaults to the pathname from `APP_BASE_URL`.

For local smoke testing only, `OOXML_AUTH_DEV_SESSIONS=1` enables
`POST /api/auth/dev-session`. `OOXML_AUTH_DEV_BYPASS=1` auto-signs local
requests into `OOXML_DEV_AUTH_EMAIL`; do not enable it in production.

Set `OOXML_TRUST_PROXY_HEADERS=1` only when the app is behind a trusted reverse
proxy that overwrites forwarding headers. Otherwise the magic-link IP limiter
ignores client-supplied forwarding headers and relies on global limits.

PPTX/PPTM browser previews require the Office renderer stack on the host. On
Ubuntu, install:

```bash
sudo apt-get install -y libreoffice-impress libreoffice-java-common default-jre-headless poppler-utils fonts-dejavu
```

`libreoffice-impress` provides the PPTX import/render filter and
`poppler-utils` provides `pdftoppm` for PNG thumbnails.

## Architecture

- `src/app.ts` owns the Hono app, upload/download/render APIs, and mounts Flue
  at `/flue`.
- `src/shared/auth.ts` implements the SafetySecretary-style custom auth layer:
  server-side session cookies, double-submit CSRF, magic links, Microsoft/Google
  OAuth, dev sessions, and simple file-backed rate limits.
- `src/agents/ooxml-editor.ts` exposes the continuing Flue agent at
  `/flue/agents/ooxml-editor/:id`.
- Threads are private to `ownerUserId`. Listing, opening, upload-into-thread,
  render, download, artifacts, and Flue agent admission all check the signed-in
  user before touching the thread workspace.
- `src/db.ts` uses Flue's Node SQLite adapter so agent session/submission state
  survives process restarts on this host.
- `src/page.ts` is the small browser workbench. It uses Flue's direct HTTP
  prompt admission and durable event stream shape, including `submissionId`,
  `streamUrl`, offsets, `tool`/`tool_call` events, and `message_end`.
- `src/shared/ooxml-tools.ts` defines the Valibot-backed Flue tools exposed to
  the agent.
- `src/shared/ooxml-actions.ts` is the OOXML bridge. Generic inspect/apply calls
  go through `ooxml serve`, not one app method per CLI command.
- Render artifacts are served only when registered on the requested version.
  Thread-level legacy version URLs reject ambiguous multi-document version ids;
  browser links use document-scoped URLs.

## Verification

```bash
npm run typecheck
npm run build
curl -fsS http://localhost:3583/health
```

With the dev server running and `EMAIL_TRANSPORT=dev`, verify auth isolation
without spending model tokens:

```bash
npm run smoke:auth
npm run smoke:auth-abuse
npm run smoke:nonpptx
```

Then exercise the real sign-in -> upload -> Flue agent -> stream -> OOXML edit
-> strict validate -> readback path:

```bash
npm run smoke:agent
```

For edited PPTX/PPTM artifacts, verify with:

```bash
ooxml validate --strict <file>.pptx
ooxml --json pptx slides show <file>.pptx --slide 1 --include-text
```

## Current Limits

- Browser preview is wired for PPTX/PPTM render thumbnails only.
- Node deployment is single-host. Use file-backed SQLite for this VPS shape; use
  a shared Flue database adapter before running multiple Node replicas.
- Auth state is file-backed under `OOXML_WEB_DATA_DIR/auth` for the single-VPS
  shape. Move it to a database before running multiple Node replicas.
- Generic mutations in multi-file threads require `expectedDocumentId` and
  `expectedVersionId` guards so selection changes fail instead of editing the
  wrong uploaded file.
