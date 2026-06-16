import { Hono, type Context } from 'hono';
import { flue } from '@flue/runtime/routing';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { extname } from 'node:path';
import {
  authUserResponse,
  checkRateLimit,
  clearAuthCookies,
  consumeMagicLink,
  finishOAuth,
  getAuthUser,
  getSafeRedirectPath,
  hasAllowedVerificationOrigin,
  isOAuthProvider,
  issueSessionForEmail,
  rateLimitResponse,
  requestMagicLink,
  requireAuth,
  startOAuth,
  type AuthEnv,
} from './shared/auth.ts';
import {
  absoluteVersionPath,
  addDocumentsToThread,
  createThreadFromUploads,
  documentById,
  listThreads,
  readThread,
  removeDocumentFromThread,
  safeId,
  selectDocument,
  type UploadedOfficeFile,
  versionById,
} from './shared/storage.ts';
import { publicThreadSummary, readVersionRenderArtifact, renderCurrent } from './shared/ooxml-actions.ts';
import { workbenchHtml } from './page.ts';

const app = new Hono<AuthEnv>();

app.get('/signin', (c) => c.html(signInHtml(getSafeRedirectPath(c.req.query('returnTo')))));

app.get('/health', (c) => c.json({ ok: true }));

app.post('/api/auth/magic-link/request', async (c) => {
  if (!hasAllowedVerificationOrigin(c)) return c.json({ error: 'Sign-in request rejected.' }, 403);
  const email = await readEmail(c.req.raw);
  try {
    const result = await requestMagicLink({ c, email });
    if (!result.ok) return rateLimitResponse(c, result.retryAfterSeconds ?? 60);
    return c.json({ message: 'Check your email for a sign-in link.' }, 202);
  } catch (error) {
    return c.json({ error: publicAuthError(error) }, 400);
  }
});

app.get('/api/auth/magic-link/verify', (c) => {
  const token = c.req.query('token') ?? '';
  if (!token) return c.json({ error: 'Magic link is invalid or expired.' }, 400);
  return c.html(confirmMagicLinkHtml(token));
});

app.post('/api/auth/magic-link/verify', async (c) => {
  if (!hasAllowedVerificationOrigin(c)) return c.json({ error: 'Magic link is invalid or expired.' }, 403);
  const token = await readToken(c.req.raw);
  if (!token) return c.json({ error: 'Magic link is invalid or expired.' }, 400);
  try {
    const user = await consumeMagicLink(c, token);
    const accept = c.req.header('accept') ?? '';
    if (accept.includes('text/html')) return c.redirect('/', 303);
    return c.json({ user: authUserResponse(user) });
  } catch {
    return c.json({ error: 'Magic link is invalid or expired.' }, 400);
  }
});

app.get('/api/auth/oauth/:provider/start', async (c) => {
  const provider = c.req.param('provider');
  if (!isOAuthProvider(provider)) return c.json({ error: 'Unsupported OAuth provider.' }, 404);
  return startOAuth(c, provider, c.req.query('returnTo'));
});

app.get('/api/auth/oauth/:provider/callback', async (c) => {
  const provider = c.req.param('provider');
  if (!isOAuthProvider(provider)) return c.redirect('/signin?error=oauth_provider', 303);
  return finishOAuth(c, provider);
});

app.post('/api/auth/dev-session', async (c) => {
  if (process.env.NODE_ENV === 'production' || (process.env.OOXML_AUTH_DEV_SESSIONS !== '1' && process.env.OOXML_AUTH_DEV_BYPASS !== '1')) {
    return c.json({ error: 'Development sessions are disabled.' }, 404);
  }
  if (!hasAllowedVerificationOrigin(c)) return c.json({ error: 'Development session rejected.' }, 403);
  const contentType = c.req.header('content-type') ?? '';
  const email = contentType.toLowerCase().startsWith('application/json')
    ? String(((await c.req.json().catch(() => ({}))) as { email?: unknown }).email ?? '')
    : String((await c.req.formData().catch(() => new FormData())).get('email') ?? '');
  try {
    const user = await issueSessionForEmail(c, email || process.env.OOXML_DEV_AUTH_EMAIL || 'oliver@local.test');
    return c.json({ user: authUserResponse(user) });
  } catch (error) {
    return c.json({ error: publicAuthError(error) }, 400);
  }
});

app.use('/api/*', requireAuth);
app.use('/flue/*', requireAuth);
app.use('/flue/*', async (c, next) => {
  if (c.req.method.toUpperCase() !== 'POST') return next();
  const user = getAuthUser(c);
  const limit = await checkRateLimit(`agent:${user.id}`, Number(process.env.OOXML_AGENT_RATE_LIMIT_PER_MINUTE || 20), 60_000);
  if (!limit.allowed) return rateLimitResponse(c, limit.retryAfterSeconds);
  await next();
});

app.get('/', requireAuth, (c) => c.html(workbenchHtml()));

app.get('/api/auth/me', (c) => c.json({ user: authUserResponse(getAuthUser(c)) }));

app.post('/api/auth/logout', (c) => {
  clearAuthCookies(c);
  return c.json({ ok: true });
});

app.get('/api/threads', async (c) => {
  try {
    const user = getAuthUser(c);
    return c.json({
      threads: (await listThreads(user.id)).map((thread) => publicThreadSummary(thread)),
    });
  } catch (error) {
    return errorResponse(c, error, 500);
  }
});

app.post('/api/upload', async (c) => {
  try {
    const user = getAuthUser(c);
    const limit = await checkRateLimit(`upload:${user.id}`, Number(process.env.OOXML_UPLOAD_RATE_LIMIT_PER_HOUR || 60), 60 * 60 * 1000);
    if (!limit.allowed) return rateLimitResponse(c, limit.retryAfterSeconds);
    const form = await c.req.formData();
    const title = String(form.get('title') ?? '');
    const threadId = String(form.get('threadId') ?? '').trim();
    const files = await officeFilesFromForm(form);
    const thread = threadId
      ? await addDocumentsToThread(threadId, files, user.id)
      : await createThreadFromUploads({
          files,
          title,
          ownerUserId: user.id,
          ownerEmail: user.email,
        });
    return c.json(publicThreadSummary(thread));
  } catch (error) {
    return errorResponse(c, error, 400, { expose: true });
  }
});

app.get('/api/threads/:id', async (c) => {
  try {
    const thread = await readThread(c.req.param('id'), getAuthUser(c).id);
    return c.json(publicThreadSummary(thread));
  } catch (error) {
    return errorResponse(c, error, 404, { expose: true });
  }
});

app.post('/api/threads/:id/render', async (c) => {
  try {
    const user = getAuthUser(c);
    await readThread(c.req.param('id'), user.id);
    const limit = await checkRateLimit(`render:${user.id}`, Number(process.env.OOXML_RENDER_RATE_LIMIT_PER_HOUR || 120), 60 * 60 * 1000);
    if (!limit.allowed) return rateLimitResponse(c, limit.retryAfterSeconds);
    return c.json(await renderCurrent(c.req.param('id')));
  } catch (error) {
    return errorResponse(c, error, 500);
  }
});

app.post('/api/threads/:id/upload', async (c) => {
  try {
    const user = getAuthUser(c);
    const limit = await checkRateLimit(`upload:${user.id}`, Number(process.env.OOXML_UPLOAD_RATE_LIMIT_PER_HOUR || 60), 60 * 60 * 1000);
    if (!limit.allowed) return rateLimitResponse(c, limit.retryAfterSeconds);
    const form = await c.req.formData();
    const files = await officeFilesFromForm(form);
    const thread = await addDocumentsToThread(c.req.param('id'), files, user.id);
    return c.json(publicThreadSummary(thread));
  } catch (error) {
    return errorResponse(c, error, 400, { expose: true });
  }
});

app.post('/api/threads/:id/documents/:documentId/select', async (c) => {
  try {
    const thread = await selectDocument(c.req.param('id'), c.req.param('documentId'), getAuthUser(c).id);
    return c.json(publicThreadSummary(thread));
  } catch (error) {
    return errorResponse(c, error, 404, { expose: true });
  }
});

app.delete('/api/threads/:id/documents/:documentId', async (c) => {
  try {
    const thread = await removeDocumentFromThread(c.req.param('id'), c.req.param('documentId'), getAuthUser(c).id);
    return c.json(publicThreadSummary(thread));
  } catch (error) {
    return errorResponse(c, error, 400, { expose: true });
  }
});

app.get('/api/threads/:id/documents/:documentId/versions/:versionId/download', async (c) => {
  try {
    const thread = await readThread(c.req.param('id'), getAuthUser(c).id);
    const document = documentById(thread, c.req.param('documentId'));
    const version = versionById(document, c.req.param('versionId'));
    const path = absoluteVersionPath(thread, version);
    const bytes = await readFile(path);
    c.header('Content-Disposition', `attachment; filename="${version.originalName.replace(/"/g, '')}"`);
    return c.body(toArrayBuffer(bytes), 200, { 'Content-Type': contentTypeFor(extname(version.path)) });
  } catch (error) {
    return errorResponse(c, error, 404, { expose: true });
  }
});

app.get('/api/threads/:id/versions/:versionId/download', async (c) => {
  try {
    const thread = await readThread(c.req.param('id'), getAuthUser(c).id);
    const versionId = safeId(c.req.param('versionId'));
    const match = uniqueVersionMatch(thread, versionId);
    if (!match.ok) return c.json({ error: match.error }, match.status);
    const { version } = match;
    const path = absoluteVersionPath(thread, version);
    const bytes = await readFile(path);
    c.header('Content-Disposition', `attachment; filename="${version.originalName.replace(/"/g, '')}"`);
    return c.body(toArrayBuffer(bytes), 200, { 'Content-Type': contentTypeFor(extname(version.path)) });
  } catch (error) {
    return errorResponse(c, error, 404, { expose: true });
  }
});

app.get('/api/threads/:id/documents/:documentId/versions/:versionId/artifact', async (c) => {
  try {
    const thread = await readThread(c.req.param('id'), getAuthUser(c).id);
    const document = documentById(thread, c.req.param('documentId'));
    const version = versionById(document, c.req.param('versionId'));
    const artifactPath = c.req.query('path');
    if (!artifactPath) return c.json({ error: 'Missing artifact path' }, 400);
    const artifact = await readVersionRenderArtifact({ thread, document, version, path: artifactPath });
    return c.body(toArrayBuffer(artifact.bytes), 200, { 'Content-Type': contentTypeFor(extname(artifact.filename)) });
  } catch (error) {
    return errorResponse(c, error, 404, { expose: true });
  }
});

app.get('/api/threads/:id/versions/:versionId/artifact', async (c) => {
  try {
    const thread = await readThread(c.req.param('id'), getAuthUser(c).id);
    const versionId = safeId(c.req.param('versionId'));
    const match = uniqueVersionMatch(thread, versionId);
    if (!match.ok) return c.json({ error: match.error }, match.status);
    const { document, version } = match;
    const artifactPath = c.req.query('path');
    if (!artifactPath) return c.json({ error: 'Missing artifact path' }, 400);
    const artifact = await readVersionRenderArtifact({ thread, document, version, path: artifactPath });
    return c.body(toArrayBuffer(artifact.bytes), 200, { 'Content-Type': contentTypeFor(extname(artifact.filename)) });
  } catch (error) {
    return errorResponse(c, error, 404, { expose: true });
  }
});

app.route('/flue', flue());

export default app;

function contentTypeFor(ext: string): string {
  switch (ext.toLowerCase()) {
    case '.pptx':
      return 'application/vnd.openxmlformats-officedocument.presentationml.presentation';
    case '.pptm':
      return 'application/vnd.ms-powerpoint.presentation.macroEnabled.12';
    case '.docx':
      return 'application/vnd.openxmlformats-officedocument.wordprocessingml.document';
    case '.xlsx':
      return 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet';
    case '.xlsm':
      return 'application/vnd.ms-excel.sheet.macroEnabled.12';
    case '.png':
      return 'image/png';
    case '.pdf':
      return 'application/pdf';
    case '.json':
      return 'application/json';
    default:
      return 'application/octet-stream';
  }
}

function toArrayBuffer(bytes: Uint8Array): ArrayBuffer {
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}

function uniqueVersionMatch(
  thread: Awaited<ReturnType<typeof readThread>>,
  versionId: string,
):
  | { ok: true; document: ReturnType<typeof documentById>; version: ReturnType<typeof versionById> }
  | { ok: false; error: string; status: 404 | 409 } {
  const matches = thread.documents
    .filter((document) => document.versions.some((version) => version.id === versionId))
    .map((document) => ({ document, version: versionById(document, versionId) }));
  if (matches.length === 0) return { ok: false, error: 'Version not found', status: 404 };
  if (matches.length > 1) {
    return {
      ok: false,
      error: 'Version id is ambiguous in a multi-file thread. Use the document-scoped download or artifact route.',
      status: 409,
    };
  }
  const match = matches[0];
  if (!match) return { ok: false, error: 'Version not found', status: 404 };
  return { ok: true, ...match };
}

async function officeFilesFromForm(form: FormData): Promise<UploadedOfficeFile[]> {
  const values = [...form.getAll('files'), ...form.getAll('file')];
  const files = values.filter((value): value is File => value instanceof File && value.size > 0);
  if (files.length === 0) {
    throw new Error('Missing Office file upload.');
  }
  const maxFiles = Number(process.env.OOXML_UPLOAD_MAX_FILES || 10);
  const maxBytes = Number(process.env.OOXML_UPLOAD_MAX_BYTES || 50 * 1024 * 1024);
  if (files.length > maxFiles) throw new Error(`Upload at most ${maxFiles} file(s) at once.`);
  for (const file of files) {
    if (file.size > maxBytes) throw new Error(`Upload files must be ${Math.floor(maxBytes / 1024 / 1024)} MB or smaller.`);
  }
  return Promise.all(
    files.map(async (file) => ({
      originalName: file.name,
      bytes: new Uint8Array(await file.arrayBuffer()),
    })),
  );
}

async function readEmail(request: Request): Promise<string> {
  const contentType = request.headers.get('content-type') ?? '';
  if (contentType.toLowerCase().startsWith('application/json')) {
    const body = (await request.json().catch(() => ({}))) as { email?: unknown };
    return typeof body.email === 'string' ? body.email : '';
  }
  const form = await request.formData().catch(() => new FormData());
  const email = form.get('email');
  return typeof email === 'string' ? email : '';
}

async function readToken(request: Request): Promise<string> {
  const urlToken = new URL(request.url).searchParams.get('token');
  if (urlToken) return urlToken;
  const contentType = request.headers.get('content-type') ?? '';
  if (contentType.toLowerCase().startsWith('application/json')) {
    const body = (await request.json().catch(() => ({}))) as { token?: unknown };
    return typeof body.token === 'string' ? body.token : '';
  }
  const form = await request.formData().catch(() => new FormData());
  const token = form.get('token');
  return typeof token === 'string' ? token : '';
}

function errorResponse(c: Context, error: unknown, status: 400 | 404 | 500, options: { expose?: boolean } = {}): Response {
  const message = errorMessage(error);
  const effectiveStatus = message === 'Thread not found' ? 404 : status;
  if (options.expose && effectiveStatus !== 500) return c.json({ error: message }, effectiveStatus);
  const errorId = randomUUID();
  console.error('[ooxml-web] request failed', { errorId, error: message });
  return c.json({ error: 'Request failed.', errorId }, effectiveStatus);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function publicAuthError(error: unknown): string {
  const message = errorMessage(error);
  if (message.includes('domain is not allowed') || message.includes('valid email')) return message;
  return 'Sign-in request could not be completed.';
}

function signInHtml(returnTo: string): string {
  return `<!doctype html>
<html lang="en" class="dark">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Sign in · OOXML Workbench</title>
    <style>
      :root { color-scheme: dark; --bg:#0e0e10; --surface:#16161a; --border:#2a2a32; --text:#e4e4e8; --muted:#8e8e9a; --accent:#7b83ff; }
      * { box-sizing: border-box; }
      body { margin:0; min-height:100vh; display:grid; place-items:center; background:var(--bg); color:var(--text); font-family:Inter, ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif; }
      main { width:min(420px, calc(100vw - 32px)); border:1px solid var(--border); border-radius:8px; background:var(--surface); padding:22px; }
      h1 { margin:0 0 8px; font-size:20px; letter-spacing:0; }
      p { color:var(--muted); line-height:1.45; margin:0 0 16px; }
      form { display:grid; gap:10px; }
      input { width:100%; border:1px solid var(--border); border-radius:7px; background:#111115; color:var(--text); padding:10px; font:inherit; }
      button, a.button { border:1px solid transparent; border-radius:7px; padding:10px 12px; background:var(--accent); color:white; font-weight:650; cursor:pointer; text-align:center; text-decoration:none; font:inherit; }
      a.button.secondary, button.secondary { background:#202028; border-color:var(--border); color:var(--text); }
      .row { display:flex; gap:8px; flex-wrap:wrap; }
      .row > * { flex:1; }
      .status { min-height:20px; margin-top:10px; color:var(--muted); font-size:13px; }
    </style>
  </head>
  <body>
    <main>
      <h1>OOXML Workbench</h1>
      <p>Sign in to keep your threads and files private to your account.</p>
      <form id="magicForm">
        <input name="email" type="email" autocomplete="email" placeholder="you@company.com" required />
        <button type="submit">Send magic link</button>
      </form>
      <div class="row" style="margin-top:10px">
        <a class="button secondary" href="/api/auth/oauth/microsoft/start?returnTo=${encodeURIComponent(returnTo)}">Microsoft</a>
        <a class="button secondary" href="/api/auth/oauth/google/start?returnTo=${encodeURIComponent(returnTo)}">Google</a>
      </div>
      <div id="status" class="status"></div>
    </main>
    <script>
      const form = document.getElementById('magicForm');
      const status = document.getElementById('status');
      form.addEventListener('submit', async (event) => {
        event.preventDefault();
        status.textContent = 'Sending sign-in link...';
        const email = new FormData(form).get('email');
        const response = await fetch('/api/auth/magic-link/request', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ email })
        });
        const data = await response.json().catch(() => ({}));
        status.textContent = data.message || data.error || (response.ok ? 'Check your email.' : 'Sign-in failed.');
      });
    </script>
  </body>
</html>`;
}

function confirmMagicLinkHtml(token: string): string {
  return `<!doctype html>
<html lang="en">
  <head><meta charset="utf-8" /><meta name="viewport" content="width=device-width, initial-scale=1" /><title>Confirm sign in</title></head>
  <body>
    <main>
      <h1>Confirm sign in</h1>
      <form method="post" action="/api/auth/magic-link/verify">
        <input type="hidden" name="token" value="${escapeHtml(token)}" />
        <button type="submit">Sign in</button>
      </form>
    </main>
  </body>
</html>`;
}

function escapeHtml(value: string): string {
  return value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}
