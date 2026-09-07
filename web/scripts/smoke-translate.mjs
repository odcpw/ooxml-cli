#!/usr/bin/env node
// End-to-end smoke for POST /api/translate. Run against a dev server started
// with EMAIL_TRANSPORT=dev and OOXML_TRANSLATE_MODE=identity so the export ->
// apply -> validate plumbing is proved without model credentials.
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
const pptxFixture = resolve(
  process.env.OOXML_WEB_SMOKE_PPTX ||
    resolve(scriptDir, '../../testdata/pptx/layout-qa/inherited-title-chart-overlap/presentation.pptx'),
);
const docxFixture = resolve(process.env.OOXML_WEB_SMOKE_DOCX || resolve(scriptDir, '../../testdata/docx/minimal/document.docx'));
const magicLinkLog = resolve(
  process.env.OOXML_MAGIC_LINK_LOG || resolve(process.env.OOXML_WEB_DATA_DIR || '../.flue-ooxml-web-data', 'auth/magic-links.jsonl'),
);
const runId = new Date().toISOString().replace(/[:.]/g, '-').toLowerCase();
const email = `translate-${runId}@example.test`;
const jar = new Map();

await main();

async function main() {
  const tmp = await mkdtemp(join(tmpdir(), 'ooxml-flue-translate-'));
  try {
    await healthCheck();
    await signIn();

    const sourceManifest = await exportManifest(pptxFixture);
    const expectedIds = sourceManifest.entries.filter((entry) => typeof entry.sourceText === 'string' && entry.sourceText.trim()).map((entry) => entry.id);
    if (expectedIds.length === 0) throw new Error(`Fixture has no translatable text: ${pptxFixture}`);

    const translated = await translate(pptxFixture, { targetLang: 'de', sourceLang: 'en' });
    if (translated.changed !== true) throw new Error(`Expected a published translation: ${JSON.stringify(translated)}`);
    if (translated.entryCount !== expectedIds.length) {
      throw new Error(`Expected ${expectedIds.length} translated segments, got ${translated.entryCount}`);
    }
    if (translated.validate?.valid !== true) throw new Error(`Translated copy is not strictly valid: ${JSON.stringify(translated.validate)}`);
    if (translated.targetLang !== 'de' || translated.sourceLang !== 'en') throw new Error(`Language tags were not echoed: ${JSON.stringify(translated)}`);
    log('translated', { threadId: translated.threadId, entryCount: translated.entryCount, versionId: translated.version?.id });

    const destination = join(tmp, `translated-${basename(pptxFixture)}`);
    await downloadFile(translated.downloadUrl, destination);
    await strictValidate(destination);

    // Identity mode copies every segment unchanged, so the re-exported text of
    // the translated copy must equal the source for every stable id.
    const roundTrip = await exportManifest(destination);
    const byId = new Map(roundTrip.entries.map((entry) => [entry.id, entry.sourceText]));
    for (const entry of sourceManifest.entries) {
      if (byId.get(entry.id) !== entry.sourceText) {
        throw new Error(`Segment ${entry.id} drifted: ${JSON.stringify(entry.sourceText)} -> ${JSON.stringify(byId.get(entry.id))}`);
      }
    }
    log('round_trip', { segments: sourceManifest.entries.length });

    const refusedDocx = await translateRaw(docxFixture, { targetLang: 'de' });
    if (refusedDocx.status !== 400 || !String(refusedDocx.body.error || '').includes('PPTX/PPTM')) {
      throw new Error(`Expected a PPTX/PPTM refusal for DOCX: ${refusedDocx.status} ${JSON.stringify(refusedDocx.body)}`);
    }
    const refusedLang = await translateRaw(pptxFixture, { targetLang: '' });
    if (refusedLang.status !== 400) throw new Error(`Expected a 400 for a missing target language: ${refusedLang.status}`);
    const refusedTag = await translateRaw(pptxFixture, { targetLang: 'not a tag' });
    if (refusedTag.status !== 400 || !String(refusedTag.body.error || '').includes('BCP 47')) {
      throw new Error(`Expected a BCP 47 refusal: ${refusedTag.status} ${JSON.stringify(refusedTag.body)}`);
    }
    log('refusals', { docx: refusedDocx.status, missingLang: refusedLang.status, badTag: refusedTag.status });

    console.log(JSON.stringify({ ok: true, threadId: translated.threadId, segments: translated.entryCount }, null, 2));
  } finally {
    await rm(tmp, { recursive: true, force: true });
  }
}

async function healthCheck() {
  const health = await getJson('/health');
  if (!health.ok) throw new Error(`Health check failed: ${JSON.stringify(health)}`);
  log('health', health);
}

async function translate(file, fields) {
  const { status, body } = await translateRaw(file, fields);
  if (status !== 200) throw new Error(`POST /api/translate failed with HTTP ${status}: ${JSON.stringify(body)}`);
  return body;
}

async function translateRaw(file, fields) {
  const form = new FormData();
  const bytes = await readFile(file);
  form.append('file', new Blob([bytes], { type: 'application/octet-stream' }), basename(file));
  for (const [key, value] of Object.entries(fields)) form.set(key, value);
  const response = await fetchWithCookies(new URL('/api/translate', baseUrl), { method: 'POST', body: form });
  const text = await response.text();
  let body = {};
  try {
    body = JSON.parse(text);
  } catch {
    body = { raw: text };
  }
  return { status: response.status, body };
}

async function exportManifest(file) {
  const { stdout } = await execFileAsync(
    ooxmlBin,
    ['--json', 'pptx', 'translate', 'export', file, '--include-notes', '--source-lang', 'en', '--target-lang', 'de'],
    { maxBuffer: 16 * 1024 * 1024 },
  );
  return JSON.parse(stdout);
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
    await new Promise((resolveWait) => setTimeout(resolveWait, 250));
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
  if (errors > 0 || parsed.valid === false) throw new Error(`Strict validation failed: ${stdout}`);
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

async function parseJsonResponse(response, label) {
  const text = await response.text();
  if (!response.ok) throw new Error(`${label} failed with HTTP ${response.status}: ${text}`);
  return JSON.parse(text);
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

function log(event, data) {
  console.error(JSON.stringify({ event, ...data }));
}
