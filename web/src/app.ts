import { Hono, type Context } from 'hono';
import { flue } from '@flue/runtime/routing';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { extname } from 'node:path';
import {
  authMiddleware,
  checkRateLimit,
  confirmMagicLinkHtml as authConfirmMagicLinkHtml,
  currentUserResponse,
  devSessionRoute,
  logoutRoute,
  oauthCallbackRoute,
  isOAuthProvider,
  rateLimitResponse,
  requestMagicLinkRoute,
  requireAuthUser,
  signInHtml as authSignInHtml,
  startOAuthRoute,
  type AuthEnv,
  verifyMagicLinkRoute,
} from './shared/auth.ts';
import { withAppBasePath } from './shared/app-url.ts';
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
import { themeCss } from './shared/theme.ts';
import { workbenchHtml } from './page.ts';

const app = new Hono<AuthEnv>();

app.get('/signin', (c) => c.html(authSignInHtml({ returnTo: c.req.query('returnTo') })));

app.get('/health', (c) => c.json({ ok: true }));

app.get('/about', (c) => c.html(legalPageHtml('About', aboutSections)));

app.get('/privacy', (c) => c.html(legalPageHtml('Privacy', privacySections)));

app.get('/terms', (c) => c.html(legalPageHtml('Terms', termsSections)));

app.post('/api/auth/magic-link/request', (c) => requestMagicLinkRoute(c));

app.get('/api/auth/magic-link/verify', (c) => {
  const token = c.req.query('token') ?? '';
  if (!token) return c.json({ error: 'Magic link is invalid or expired.' }, 400);
  return c.html(authConfirmMagicLinkHtml(token));
});

app.post('/api/auth/magic-link/verify', (c) => verifyMagicLinkRoute(c));

app.get('/api/auth/oauth/:provider/start', async (c) => {
  const provider = c.req.param('provider');
  if (!isOAuthProvider(provider)) return c.json({ error: 'Unsupported OAuth provider.' }, 404);
  return startOAuthRoute(c, provider);
});

app.get('/api/auth/oauth/:provider/callback', async (c) => {
  const provider = c.req.param('provider');
  if (!isOAuthProvider(provider)) return c.redirect('/signin?error=oauth_provider', 303);
  return oauthCallbackRoute(c, provider);
});

app.post('/api/auth/dev-session', (c) => devSessionRoute(c));

app.use('/api/*', authMiddleware);
app.use('/flue/*', authMiddleware);
app.use('/flue/*', async (c, next) => {
  if (c.req.method.toUpperCase() !== 'POST') return next();
  const user = requireAuthUser(c);
  const limit = await checkRateLimit(`agent:${user.id}`, Number(process.env.OOXML_AGENT_RATE_LIMIT_PER_MINUTE || 20), 60_000);
  if (!limit.allowed) return rateLimitResponse(c, limit.retryAfterSeconds);
  await next();
});

app.get('/', authMiddleware, (c) => {
  c.header('Cache-Control', 'no-store');
  return c.html(workbenchHtml());
});

app.get('/api/auth/me', (c) => currentUserResponse(c));

app.post('/api/auth/logout', (c) => logoutRoute(c));

app.get('/api/threads', async (c) => {
  try {
    const user = requireAuthUser(c);
    const limit = positiveInteger(c.req.query('limit'), 100);
    return c.json({
      threads: (await listThreads(user.id, { limit })).map((thread) => publicThreadSummary(thread)),
    });
  } catch (error) {
    return errorResponse(c, error, 500);
  }
});

app.post('/api/upload', async (c) => {
  try {
    const user = requireAuthUser(c);
    const limit = await checkRateLimit(`upload:${user.id}`, Number(process.env.OOXML_UPLOAD_RATE_LIMIT_PER_HOUR || 60), 60 * 60 * 1000);
    if (!limit.allowed) return rateLimitResponse(c, limit.retryAfterSeconds);
    const maxTotalBytes = positiveInteger(process.env.OOXML_UPLOAD_MAX_TOTAL_BYTES, 80 * 1024 * 1024);
    const declaredBytes = Number(c.req.header('content-length') ?? 0);
    if (Number.isFinite(declaredBytes) && declaredBytes > maxTotalBytes + 1024 * 1024) {
      return c.json({ error: 'Upload is too large.' }, 413);
    }
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
    const thread = await readThread(c.req.param('id'), requireAuthUser(c).id);
    return c.json(publicThreadSummary(thread));
  } catch (error) {
    return errorResponse(c, error, 404, { expose: true });
  }
});

app.post('/api/threads/:id/render', async (c) => {
  try {
    const user = requireAuthUser(c);
    await readThread(c.req.param('id'), user.id);
    const limit = await checkRateLimit(`render:${user.id}`, Number(process.env.OOXML_RENDER_RATE_LIMIT_PER_HOUR || 120), 60 * 60 * 1000);
    if (!limit.allowed) return rateLimitResponse(c, limit.retryAfterSeconds);
    return c.json(await renderCurrent(c.req.param('id')));
  } catch (error) {
    return renderErrorResponse(c, error);
  }
});

app.post('/api/threads/:id/upload', async (c) => {
  try {
    const user = requireAuthUser(c);
    const limit = await checkRateLimit(`upload:${user.id}`, Number(process.env.OOXML_UPLOAD_RATE_LIMIT_PER_HOUR || 60), 60 * 60 * 1000);
    if (!limit.allowed) return rateLimitResponse(c, limit.retryAfterSeconds);
    const maxTotalBytes = positiveInteger(process.env.OOXML_UPLOAD_MAX_TOTAL_BYTES, 80 * 1024 * 1024);
    const declaredBytes = Number(c.req.header('content-length') ?? 0);
    if (Number.isFinite(declaredBytes) && declaredBytes > maxTotalBytes + 1024 * 1024) {
      return c.json({ error: 'Upload is too large.' }, 413);
    }
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
    const thread = await selectDocument(c.req.param('id'), c.req.param('documentId'), requireAuthUser(c).id);
    return c.json(publicThreadSummary(thread));
  } catch (error) {
    return errorResponse(c, error, 404, { expose: true });
  }
});

app.delete('/api/threads/:id/documents/:documentId', async (c) => {
  try {
    const thread = await removeDocumentFromThread(c.req.param('id'), c.req.param('documentId'), requireAuthUser(c).id);
    return c.json(publicThreadSummary(thread));
  } catch (error) {
    return errorResponse(c, error, 400, { expose: true });
  }
});

app.get('/api/threads/:id/documents/:documentId/versions/:versionId/download', async (c) => {
  try {
    const thread = await readThread(c.req.param('id'), requireAuthUser(c).id);
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
    const thread = await readThread(c.req.param('id'), requireAuthUser(c).id);
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
    const thread = await readThread(c.req.param('id'), requireAuthUser(c).id);
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
    const thread = await readThread(c.req.param('id'), requireAuthUser(c).id);
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

const aboutSections = [
  {
    title: 'Purpose',
    body: 'OOXML Workbench helps signed-in users upload Office Open XML documents, inspect and render them, prompt an agent to make document edits, and download the edited files.',
  },
  {
    title: 'Supported files',
    body: 'The workbench is designed for PowerPoint, Word, and Excel files that use Office Open XML formats such as PPTX, DOCX, and XLSX.',
  },
  {
    title: 'Private workspaces',
    body: 'Each signed-in user gets isolated document threads and a private file library, so uploaded files and generated versions are not shared with other users.',
  },
  {
    title: 'Sign in',
    body: 'Google, Microsoft, and magic-link sign-in options are used only to identify the user and keep their workbench files separate.',
  },
];

const privacySections = [
  {
    title: 'What the app stores',
    body: 'OOXML Workbench stores your sign-in email address, session records, uploaded Office documents, generated previews, edited versions, and agent thread history so you can continue work across sessions.',
  },
  {
    title: 'How files are used',
    body: 'Uploaded files are used only to inspect, render, validate, and edit documents you submit in the workbench. Agent requests may send the relevant prompt and document-derived context to the configured model provider.',
  },
  {
    title: 'Personal data',
    body: 'Do not upload unnecessary personal data, sensitive personal information, or documents containing PII that is not needed for the task. Remove or redact that information before using the workbench.',
  },
  {
    title: 'Account data',
    body: 'Google and Microsoft sign-in are used only to identify your account email and keep your private workbench data separate from other users.',
  },
  {
    title: 'Contact',
    body: 'For access or deletion requests, use the private support channel that shared this workbench with you.',
  },
];

const termsSections = [
  {
    title: 'Use',
    body: 'Use OOXML Workbench only with documents you are allowed to upload and process. Review generated edits before relying on them.',
  },
  {
    title: 'Availability',
    body: 'The service is provided as a small private workbench and may change, pause, or lose availability during maintenance.',
  },
  {
    title: 'Responsibility',
    body: 'You remain responsible for the content of uploaded files, prompts, generated outputs, and downloaded documents.',
  },
  {
    title: 'Contact',
    body: 'Questions about these terms should go through the private support channel that shared this workbench with you.',
  },
];

function legalPageHtml(title: string, sections: readonly { title: string; body: string }[]): string {
  return `<!doctype html>
<html lang="en" class="dark">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>${escapeHtml(title)} - OOXML Workbench</title>
    <style>
${themeCss()}
      /* Legal page layer (About / Privacy / Terms) */
      body { min-height: 100vh; }
      main { width: min(760px, calc(100vw - var(--space-8))); margin: 0 auto; padding: var(--space-10) 0; }
      a { color: var(--color-accent); }
      header { border-bottom: var(--border); margin-bottom: var(--space-6); padding-bottom: var(--space-5); }
      h1 { margin: 0 0 var(--space-2); font-size: var(--text-2xl); font-weight: var(--font-weight-semibold); letter-spacing: var(--tracking-tight); line-height: var(--leading-tight); }
      h2 { margin: var(--space-6) 0 var(--space-2); font-size: var(--text-lg); font-weight: var(--font-weight-medium); letter-spacing: var(--tracking-tight); line-height: var(--leading-snug); }
      p { color: var(--color-muted); font-size: var(--text-base); line-height: var(--leading-relaxed); margin: 0 0 var(--space-3); }
      nav { display: flex; gap: var(--space-4); margin-top: var(--space-4); }
    </style>
  </head>
  <body>
    <main>
      <header>
        <h1>${escapeHtml(title)}</h1>
        <p>OOXML Workbench is a private document editing workbench for Office Open XML files.</p>
        <nav><a href="${escapeHtml(withAppBasePath('/'))}">Workbench</a><a href="${escapeHtml(withAppBasePath('/about'))}">About</a><a href="${escapeHtml(withAppBasePath('/privacy'))}">Privacy</a><a href="${escapeHtml(withAppBasePath('/terms'))}">Terms</a></nav>
      </header>
      ${sections.map((section) => `<section><h2>${escapeHtml(section.title)}</h2><p>${escapeHtml(section.body)}</p></section>`).join('')}
    </main>
  </body>
</html>`;
}

function escapeHtml(value: string): string {
  return value.replace(/[&<>"']/g, (char) => {
    switch (char) {
      case '&':
        return '&amp;';
      case '<':
        return '&lt;';
      case '>':
        return '&gt;';
      case '"':
        return '&quot;';
      case "'":
        return '&#39;';
      default:
        return char;
    }
  });
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
  const maxFiles = positiveInteger(process.env.OOXML_UPLOAD_MAX_FILES, 8);
  const maxBytes = positiveInteger(process.env.OOXML_UPLOAD_MAX_BYTES, 25 * 1024 * 1024);
  const maxTotalBytes = positiveInteger(process.env.OOXML_UPLOAD_MAX_TOTAL_BYTES, 80 * 1024 * 1024);
  if (files.length > maxFiles) throw new Error(`Upload at most ${maxFiles} file(s) at once.`);
  const totalBytes = files.reduce((sum, file) => sum + file.size, 0);
  if (totalBytes > maxTotalBytes) {
    throw new Error(`Upload batches must be ${Math.floor(maxTotalBytes / 1024 / 1024)} MB or smaller.`);
  }
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

function errorResponse(c: Context, error: unknown, status: 400 | 404 | 500, options: { expose?: boolean } = {}): Response {
  const message = errorMessage(error);
  const effectiveStatus = message === 'Thread not found' ? 404 : status;
  // Never echo filesystem errors (a missing version/render file) to the client:
  // Node's ENOENT message carries the absolute data-dir path. Storage's own
  // intentional messages ("Version not found", validation text) have no path and
  // stay exposed.
  const looksLikeFsError = /\b(ENOENT|EACCES|EISDIR|ENOTDIR|EPERM)\b/.test(message) || /(^|\s)\/[^\s]/.test(message);
  if (options.expose && effectiveStatus !== 500 && !looksLikeFsError) {
    return c.json({ error: message }, effectiveStatus);
  }
  const errorId = randomUUID();
  console.error('[ooxml-web] request failed', { errorId, error: message });
  const publicMessage = effectiveStatus === 404 ? 'Not found or no longer available.' : 'Request failed.';
  return c.json({ error: publicMessage, errorId }, effectiveStatus);
}

function renderErrorResponse(c: Context, error: unknown): Response {
  const message = errorMessage(error);
  const errorId = randomUUID();
  console.error('[ooxml-web] render failed', { errorId, error: message });
  return c.json({ error: publicRenderErrorMessage(message), errorId }, 500);
}

function publicRenderErrorMessage(message: string): string {
  if (/required render tool not available:\s*pdftoppm/i.test(message)) {
    return 'Preview render failed: the server is missing pdftoppm/poppler-utils for thumbnail generation.';
  }
  if (/source file could not be loaded/i.test(message)) {
    return 'Preview render failed: LibreOffice could not load the presentation. The server may be missing LibreOffice Impress, or the file may use unsupported content.';
  }
  if (/soffice failed/i.test(message)) {
    return 'Preview render failed inside LibreOffice.';
  }
  if (/timed out|timeout/i.test(message)) {
    return 'Preview render timed out on the server.';
  }
  return 'Preview render failed on the server.';
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function positiveInteger(value: string | number | undefined, fallback: number): number {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) return fallback;
  return Math.trunc(parsed);
}
