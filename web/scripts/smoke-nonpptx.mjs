#!/usr/bin/env node
import { execFile } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

const execFileAsync = promisify(execFile);
const baseUrl = process.env.OOXML_WEB_BASE_URL || 'http://localhost:3583';
const ooxmlBin = process.env.OOXML_BIN || 'ooxml';
const scriptDir = fileURLToPath(new URL('.', import.meta.url));
const docxFixture = resolve(process.env.OOXML_WEB_SMOKE_DOCX || resolve(scriptDir, '../../testdata/docx/minimal/document.docx'));
const xlsxFixture = resolve(process.env.OOXML_WEB_SMOKE_XLSX || resolve(scriptDir, '../../testdata/xlsx/minimal-workbook/workbook.xlsx'));
const magicLinkLog = resolve(process.env.OOXML_MAGIC_LINK_LOG || resolve(process.env.OOXML_WEB_DATA_DIR || '../.flue-ooxml-web-data', 'auth/magic-links.jsonl'));
const runId = new Date().toISOString().replace(/[:.]/g, '-').toLowerCase();
const email = `nonpptx-${runId}@example.test`;
const jar = new Map();

await main();

async function main() {
  const tmp = await mkdtemp(join(tmpdir(), 'ooxml-flue-nonpptx-'));
  try {
    await healthCheck();
    await signIn();
    const thread = await uploadFixtures();
    if ((thread.documents || []).length !== 2) throw new Error(`Expected two uploaded documents: ${JSON.stringify(thread)}`);
    log('uploaded', { threadId: thread.id, documents: thread.documents.length });

    for (const document of thread.documents) {
      const selected = await postJson(`/api/threads/${encodeURIComponent(thread.id)}/documents/${encodeURIComponent(document.id)}/select`, {});
      const render = await postJson(`/api/threads/${encodeURIComponent(thread.id)}/render`, {});
      if (render.rendered !== false) throw new Error(`Expected non-PPTX render to be declined: ${JSON.stringify(render)}`);
      if (!String(render.reason || '').includes('PPTX/PPTM')) throw new Error(`Unexpected non-PPTX render reason: ${JSON.stringify(render)}`);

      const current = currentDocument(selected);
      const version = currentVersion(selected, current);
      const destination = join(tmp, `${current.id}-${version.originalName}`);
      await downloadFile(version.downloadUrl, destination);
      await strictValidate(destination);
      log('checked_document', {
        documentId: current.id,
        extension: selected.currentExtension,
        versionId: version.id,
      });
    }

    console.log(JSON.stringify({ ok: true, threadId: thread.id, documents: thread.documents.length }, null, 2));
  } finally {
    await rm(tmp, { recursive: true, force: true });
  }
}

async function healthCheck() {
  const health = await getJson('/health');
  if (!health.ok) throw new Error(`Health check failed: ${JSON.stringify(health)}`);
  log('health', health);
}

async function uploadFixtures() {
  const form = new FormData();
  form.set('title', 'non-PPTX smoke');
  await appendFile(form, docxFixture, 'application/vnd.openxmlformats-officedocument.wordprocessingml.document');
  await appendFile(form, xlsxFixture, 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet');
  const response = await fetchWithCookies(new URL('/api/upload', baseUrl), { method: 'POST', body: form });
  return parseJsonResponse(response, 'upload');
}

async function appendFile(form, file, type) {
  const bytes = await readFile(file);
  form.append('files', new Blob([bytes], { type }), basename(file));
}

async function signIn() {
  await postJson('/api/auth/magic-link/request', { email });
  const link = await newestMagicLinkFor(email);
  const token = new URL(link.magicLinkUrl).searchParams.get('token');
  if (!token) throw new Error(`Magic-link log entry did not include token: ${JSON.stringify(link)}`);
  const verified = await postJson('/api/auth/magic-link/verify', { token });
  if (verified.user?.email !== email) throw new Error(`Signed in as unexpected user: ${JSON.stringify(verified)}`);
  log('signed_in', { email });
}

async function newestMagicLinkFor(targetEmail) {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    const entries = await readMagicLinkLog().catch(() => []);
    const latest = entries.filter((entry) => entry.to === targetEmail && entry.magicLinkUrl).at(-1);
    if (latest) return latest;
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`No magic link for ${targetEmail} found in ${magicLinkLog}. Use EMAIL_TRANSPORT=dev for local smoke tests.`);
}

async function readMagicLinkLog() {
  const raw = await readFile(magicLinkLog, 'utf8');
  return raw
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

async function downloadFile(urlPath, destination) {
  if (!urlPath) throw new Error('Missing download URL.');
  const response = await fetchWithCookies(new URL(urlPath, baseUrl));
  if (!response.ok) throw new Error(`Download failed with HTTP ${response.status}: ${await response.text()}`);
  await writeFile(destination, new Uint8Array(await response.arrayBuffer()));
  log('downloaded', { destination });
}

async function strictValidate(file) {
  const { stdout } = await execFileAsync(ooxmlBin, ['--json', '--strict', 'validate', file], {
    maxBuffer: 16 * 1024 * 1024,
  });
  const parsed = JSON.parse(stdout);
  const errors = Number(parsed.errors ?? parsed.summary?.errors ?? 0);
  if (errors > 0) throw new Error(`Strict validation failed: ${stdout}`);
  log('validated', { file: basename(file), errors });
}

async function getJson(path) {
  const response = await fetchWithCookies(new URL(path, baseUrl));
  return parseJsonResponse(response, path);
}

async function postJson(path, body) {
  const response = await fetchWithCookies(new URL(path, baseUrl), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  return parseJsonResponse(response, path);
}

async function fetchWithCookies(url, init = {}) {
  const headers = new Headers(init.headers || {});
  const cookie = cookieHeader();
  if (cookie) headers.set('Cookie', cookie);
  const method = String(init.method || 'GET').toUpperCase();
  if (!['GET', 'HEAD', 'OPTIONS'].includes(method)) {
    if (!headers.has('Origin')) headers.set('Origin', new URL(baseUrl).origin);
    const csrf = jar.get('ooxml_csrf');
    if (csrf) headers.set('x-ooxml-csrf', csrf);
  }
  const response = await fetch(url, { ...init, headers });
  rememberSetCookies(response);
  return response;
}

function rememberSetCookies(response) {
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

function cookieHeader() {
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
