#!/usr/bin/env node
import { readFile } from 'node:fs/promises';
import { basename, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const baseUrl = process.env.OOXML_WEB_BASE_URL || 'http://localhost:3583';
const scriptDir = fileURLToPath(new URL('.', import.meta.url));
const defaultFixture = resolve(scriptDir, '../../../testdata/pptx/minimal-title/presentation.pptx');
const fixture = resolve(process.argv[2] || process.env.OOXML_WEB_SMOKE_FIXTURE || defaultFixture);
const magicLinkLog = resolve(process.env.OOXML_MAGIC_LINK_LOG || resolve(process.env.OOXML_WEB_DATA_DIR || '../.flue-ooxml-web-data', 'auth/magic-links.jsonl'));
const runId = new Date().toISOString().replace(/[:.]/g, '-').toLowerCase();
const userA = `auth-a-${runId}@example.test`;
const userB = `auth-b-${runId}@example.test`;

await main();

async function main() {
  await expectStatus(new Map(), '/api/threads', {}, 401, 'unauthenticated thread list');

  const jarA = new Map();
  const jarB = new Map();
  await signIn(jarA, userA);
  await signIn(jarB, userB);
  await assertWorkbenchChrome(jarA, userA);

  const thread = await uploadFixture(jarA);
  log('uploaded', { threadId: thread.id, owner: userA });
  await expectStatus(jarA, `/api/threads/${encodeURIComponent(thread.id)}/render`, { method: 'POST' }, 403, 'missing csrf render', {
    skipCsrf: true,
  });
  await expectStatus(
    jarA,
    `/flue/agents/ooxml-editor/${encodeURIComponent(thread.id)}`,
    { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ message: 'hello' }) },
    403,
    'missing csrf agent admission',
    { skipCsrf: true },
  );

  const listA = await getJson(jarA, '/api/threads');
  if (!listA.threads?.some((candidate) => candidate.id === thread.id)) {
    throw new Error('User A could not see their uploaded thread.');
  }

  const listB = await getJson(jarB, '/api/threads');
  if (listB.threads?.some((candidate) => candidate.id === thread.id)) {
    throw new Error('User B saw user A thread in list.');
  }

  await expectStatus(jarB, `/api/threads/${encodeURIComponent(thread.id)}`, {}, 404, 'cross-user read');
  await expectStatus(jarB, `/api/threads/${encodeURIComponent(thread.id)}/render`, { method: 'POST' }, 404, 'cross-user render');
  await expectStatus(
    jarB,
    `/flue/agents/ooxml-editor/${encodeURIComponent(thread.id)}`,
    { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ message: 'hello' }) },
    404,
    'cross-user agent admission',
  );

  const document = currentDocument(thread);
  const version = currentVersion(thread, document);
  await expectStatus(jarA, version.downloadUrl, {}, 200, 'owner download');
  await expectStatus(jarB, version.downloadUrl, {}, 404, 'cross-user download');

  const render = await postJson(jarA, `/api/threads/${encodeURIComponent(thread.id)}/render`, {});
  if (render.rendered && render.thumbnails?.[0]?.path) {
    const artifactPath =
      `/api/threads/${encodeURIComponent(thread.id)}` +
      `/documents/${encodeURIComponent(render.currentDocumentId)}` +
      `/versions/${encodeURIComponent(render.currentVersionId)}` +
      `/artifact?path=${encodeURIComponent(render.thumbnails[0].path)}`;
    await expectStatus(jarA, artifactPath, {}, 200, 'owner artifact');
    await expectStatus(jarB, artifactPath, {}, 404, 'cross-user artifact');
  }

  console.log(JSON.stringify({ ok: true, threadId: thread.id, users: [userA, userB] }, null, 2));
}

async function uploadFixture(jar) {
  const bytes = await readFile(fixture);
  const form = new FormData();
  form.set('title', `auth smoke ${basename(fixture)}`);
  form.append(
    'files',
    new Blob([bytes], {
      type: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
    }),
    basename(fixture),
  );
  const response = await fetchWithCookies(jar, new URL('/api/upload', baseUrl), { method: 'POST', body: form });
  return parseJsonResponse(response, 'upload');
}

async function signIn(jar, email) {
  await postJson(jar, '/api/auth/magic-link/request', { email });
  const link = await newestMagicLinkFor(email);
  const token = new URL(link.magicLinkUrl).searchParams.get('token');
  if (!token) throw new Error(`Magic-link log entry did not include token: ${JSON.stringify(link)}`);
  const verified = await postJson(jar, '/api/auth/magic-link/verify', { token });
  if (verified.user?.email !== email) throw new Error(`Signed in as unexpected user: ${JSON.stringify(verified)}`);
  log('signed_in', { email });
}

async function newestMagicLinkFor(email) {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    const entries = await readMagicLinkLog().catch(() => []);
    const latest = entries.filter((entry) => entry.to === email && entry.magicLinkUrl).at(-1);
    if (latest) return latest;
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`No magic link for ${email} found in ${magicLinkLog}. Use EMAIL_TRANSPORT=dev for local smoke tests.`);
}

async function readMagicLinkLog() {
  const raw = await readFile(magicLinkLog, 'utf8');
  return raw
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

async function getJson(jar, path) {
  const response = await fetchWithCookies(jar, new URL(path, baseUrl));
  return parseJsonResponse(response, path);
}

async function postJson(jar, path, body) {
  const response = await fetchWithCookies(jar, new URL(path, baseUrl), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  return parseJsonResponse(response, path);
}

async function expectStatus(jar, path, init, expected, label, options = {}) {
  const response = await fetchWithCookies(jar, new URL(path, baseUrl), init, options);
  if (response.status !== expected) {
    throw new Error(`${label} expected HTTP ${expected}, got ${response.status}: ${await response.text()}`);
  }
  log('status', { label, status: response.status });
}

async function assertWorkbenchChrome(jar, expectedEmail) {
  const response = await fetchWithCookies(jar, new URL('/', baseUrl));
  const html = await response.text();
  if (!response.ok) throw new Error(`workbench root failed with HTTP ${response.status}: ${html}`);
  if (!html.includes('logoutBtn') || !html.includes('Sign out') || !html.includes('accountLine')) {
    throw new Error('Workbench HTML did not include visible account/sign-out controls.');
  }
  const me = await getJson(jar, '/api/auth/me');
  if (me.user?.email !== expectedEmail) throw new Error(`auth/me returned unexpected user: ${JSON.stringify(me)}`);
  log('workbench_chrome', { accountControls: true, email: expectedEmail });
}

async function fetchWithCookies(jar, url, init = {}, options = {}) {
  const headers = new Headers(init.headers || {});
  const cookie = cookieHeader(jar);
  if (cookie) headers.set('Cookie', cookie);
  const method = String(init.method || 'GET').toUpperCase();
  if (!options.skipCsrf && !['GET', 'HEAD', 'OPTIONS'].includes(method)) {
    const csrf = jar.get('ooxml_csrf');
    if (csrf) headers.set('x-ooxml-csrf', csrf);
  }
  const response = await fetch(url, { ...init, headers });
  rememberSetCookies(jar, response);
  return response;
}

function rememberSetCookies(jar, response) {
  const values =
    typeof response.headers.getSetCookie === 'function'
      ? response.headers.getSetCookie()
      : splitCombinedSetCookie(response.headers.get('set-cookie') || '');
  for (const value of values) {
    const pair = value.split(';')[0];
    const index = pair.indexOf('=');
    if (index <= 0) continue;
    const name = pair.slice(0, index);
    const cookieValue = pair.slice(index + 1);
    if (!cookieValue) jar.delete(name);
    else jar.set(name, cookieValue);
  }
}

function splitCombinedSetCookie(value) {
  if (!value) return [];
  return value.split(/,(?=\s*[^;,=\s]+=)/g).map((part) => part.trim()).filter(Boolean);
}

function cookieHeader(jar) {
  return [...jar.entries()].map(([name, value]) => `${name}=${value}`).join('; ');
}

async function parseJsonResponse(response, label) {
  const text = await response.text();
  let parsed;
  try {
    parsed = text ? JSON.parse(text) : {};
  } catch {
    throw new Error(`${label} returned non-JSON HTTP ${response.status}: ${text}`);
  }
  if (!response.ok) {
    throw new Error(`${label} failed with HTTP ${response.status}: ${JSON.stringify(parsed)}`);
  }
  return parsed;
}

function currentDocument(thread) {
  const document = (thread.documents || []).find((candidate) => candidate.id === thread.currentDocumentId);
  if (!document) throw new Error('Current document missing from thread response.');
  return document;
}

function currentVersion(thread, document) {
  const versionId = thread.currentVersionId || document.currentVersionId;
  const version = (document.versions || []).find((candidate) => candidate.id === versionId);
  if (!version) throw new Error('Current version missing from thread response.');
  return version;
}

function log(phase, data) {
  process.stderr.write(`${JSON.stringify({ ts: new Date().toISOString(), phase, ...data })}\n`);
}
