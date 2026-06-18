#!/usr/bin/env node
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const baseUrl = process.env.OOXML_WEB_BASE_URL || 'http://localhost:3583';
const magicLinkLog = resolve(process.env.OOXML_MAGIC_LINK_LOG || resolve(process.env.OOXML_WEB_DATA_DIR || '../.flue-ooxml-web-data', 'auth/magic-links.jsonl'));
const runId = new Date().toISOString().replace(/[:.]/g, '-').toLowerCase();

await main();

async function main() {
  await expectStatus(
    new Map(),
    '/api/auth/magic-link/request',
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Origin: 'https://evil.example' },
      body: JSON.stringify({ email: `bad-origin-${runId}@example.test` }),
    },
    403,
    'bad origin magic-link request',
    { skipCsrf: true },
  );

  await expectStatus(
    new Map(),
    '/api/auth/magic-link/request',
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Referer: 'https://evil.example/signin' },
      body: JSON.stringify({ email: `bad-referer-${runId}@example.test` }),
    },
    403,
    'bad referer magic-link request',
    { skipCsrf: true, skipOrigin: true },
  );

  await expectStatus(
    new Map(),
    '/api/auth/magic-link/verify',
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ token: 'not-a-real-token' }),
    },
    400,
    'invalid magic-link token',
    { skipCsrf: true },
  );

  await assertMagicLinkReplacement();

  const jar = new Map();
  const email = `reuse-${runId}@example.test`;
  const token = await requestMagicToken(email);
  const verified = await postJson(jar, '/api/auth/magic-link/verify', { token }, { skipCsrf: true });
  if (verified.user?.email !== email) throw new Error(`Signed in as unexpected user: ${JSON.stringify(verified)}`);
  await expectStatus(jar, '/api/auth/magic-link/verify', jsonInit({ token }), 400, 'reused magic-link token', { skipCsrf: true });

  const form = new FormData();
  form.append('files', new Blob([new Uint8Array([1, 2, 3])], { type: 'text/plain' }), 'not-office.txt');
  await expectStatus(jar, '/api/upload', { method: 'POST', body: form }, 400, 'unsupported upload extension');

  console.log(JSON.stringify({ ok: true, runId }, null, 2));
}

async function assertMagicLinkReplacement() {
  const jar = new Map();
  const email = `limited-${runId}@example.test`;
  await expectStatus(jar, '/api/auth/magic-link/request', jsonInit({ email }), 202, 'initial magic-link request', { skipCsrf: true });
  const firstToken = new URL((await newestMagicLinkFor(email)).magicLinkUrl).searchParams.get('token');
  if (!firstToken) throw new Error('Initial magic-link token missing.');
  for (let index = 0; index < 3; index += 1) {
    await expectStatus(jar, '/api/auth/magic-link/request', jsonInit({ email }), 202, `magic-link request ${index + 1}`, { skipCsrf: true });
  }
  const latestToken = new URL((await newestMagicLinkFor(email)).magicLinkUrl).searchParams.get('token');
  if (!latestToken || latestToken === firstToken) throw new Error('Repeated magic-link request did not create a fresh token.');
  await expectStatus(jar, '/api/auth/magic-link/verify', jsonInit({ token: firstToken }), 400, 'older replaced magic-link token', {
    skipCsrf: true,
  });
  const verified = await postJson(jar, '/api/auth/magic-link/verify', { token: latestToken }, { skipCsrf: true });
  if (verified.user?.email !== email) throw new Error(`Latest magic link did not sign in expected user: ${JSON.stringify(verified)}`);
  log('magic_link_replacement', { email });
}

async function requestMagicToken(email) {
  await expectStatus(new Map(), '/api/auth/magic-link/request', jsonInit({ email }), 202, `magic-link request for ${email}`, { skipCsrf: true });
  const link = await newestMagicLinkFor(email);
  const token = new URL(link.magicLinkUrl).searchParams.get('token');
  if (!token) throw new Error(`Magic-link log entry did not include token: ${JSON.stringify(link)}`);
  return token;
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

async function postJson(jar, path, body, options = {}) {
  const response = await fetchWithCookies(jar, new URL(path, baseUrl), jsonInit(body), options);
  return parseJsonResponse(response, path);
}

async function expectStatus(jar, path, init, expected, label, options = {}) {
  const response = await fetchWithCookies(jar, new URL(path, baseUrl), init, options);
  if (response.status !== expected) {
    throw new Error(`${label} expected HTTP ${expected}, got ${response.status}: ${await response.text()}`);
  }
  log('status', { label, status: response.status });
}

async function fetchWithCookies(jar, url, init = {}, options = {}) {
  const headers = new Headers(init.headers || {});
  const cookie = cookieHeader(jar);
  if (cookie) headers.set('Cookie', cookie);
  const method = String(init.method || 'GET').toUpperCase();
  if (!options.skipCsrf && !['GET', 'HEAD', 'OPTIONS'].includes(method)) {
    if (!options.skipOrigin && !headers.has('Origin')) headers.set('Origin', new URL(baseUrl).origin);
    const csrf = jar.get('ooxml_csrf');
    if (csrf) headers.set('x-ooxml-csrf', csrf);
  }
  if (options.skipCsrf && !options.skipOrigin && !['GET', 'HEAD', 'OPTIONS'].includes(method) && !headers.has('Origin')) {
    headers.set('Origin', new URL(baseUrl).origin);
  }
  const response = await fetch(url, { ...init, headers });
  rememberSetCookies(jar, response);
  return response;
}

function jsonInit(body) {
  return {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  };
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

function log(phase, data) {
  process.stderr.write(`${JSON.stringify({ ts: new Date().toISOString(), phase, ...data })}\n`);
}
