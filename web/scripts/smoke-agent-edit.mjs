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
const defaultFixture = resolve(scriptDir, '../../../testdata/pptx/minimal-title/presentation.pptx');
const fixture = resolve(process.argv[2] || process.env.OOXML_WEB_SMOKE_FIXTURE || defaultFixture);
const marker = `Flue smoke ${new Date().toISOString().replace(/[:.]/g, '-')}`;
const smokeEmail = process.env.OOXML_WEB_SMOKE_EMAIL || 'smoke-ooxml@example.test';
const magicLinkLog = resolve(process.env.OOXML_MAGIC_LINK_LOG || resolve(process.env.OOXML_WEB_DATA_DIR || '../.flue-ooxml-web-data', 'auth/magic-links.jsonl'));
const cookieJar = new Map();

const summary = {
  baseUrl,
  fixture,
  marker,
  threadId: undefined,
  versionId: undefined,
  toolNames: [],
  rendered: false,
  thumbnails: 0,
};

await main();

async function main() {
  const tmp = await mkdtemp(join(tmpdir(), 'ooxml-flue-smoke-'));
  try {
    await healthCheck();
    await signIn();
    const thread = await uploadFixture();
    summary.threadId = thread.id;
    log('uploaded', { threadId: thread.id, documents: thread.documents?.length || 0 });

    const agent = await runAgent(thread.id);
    summary.toolNames = [...agent.toolNames];
    log('agent_complete', { assistantChars: agent.assistantText.length, toolNames: summary.toolNames });

    const render = await postJson(`/api/threads/${encodeURIComponent(thread.id)}/render`, {});
    summary.rendered = Boolean(render.rendered);
    summary.thumbnails = Array.isArray(render.thumbnails) ? render.thumbnails.length : 0;
    log('rendered', { rendered: summary.rendered, thumbnails: summary.thumbnails, reason: render.reason });

    const refreshed = await getJson(`/api/threads/${encodeURIComponent(thread.id)}`);
    const document = currentDocument(refreshed);
    const version = currentVersion(refreshed, document);
    summary.versionId = version.id;
    if (version.id === 'v0001') {
      throw new Error('Agent did not publish a new version.');
    }

    const editedPath = join(tmp, `edited-${version.id}.pptx`);
    await downloadFile(version.downloadUrl || refreshed.downloadUrl, editedPath);
    await strictValidate(editedPath);
    await assertSlideContains(editedPath, marker);

    if (summary.toolNames.length && !summary.toolNames.includes('apply_ooxml_ops_to_current')) {
      throw new Error(`Agent did not use apply_ooxml_ops_to_current. Saw tools: ${summary.toolNames.join(', ')}`);
    }

    console.log(JSON.stringify({ ok: true, ...summary }, null, 2));
  } finally {
    await rm(tmp, { recursive: true, force: true });
  }
}

async function healthCheck() {
  const health = await getJson('/health');
  if (!health.ok) throw new Error(`Health check failed: ${JSON.stringify(health)}`);
  log('health', health);
}

async function uploadFixture() {
  const bytes = await readFile(fixture);
  const form = new FormData();
  form.set('title', `smoke ${basename(fixture)}`);
  form.append(
    'files',
    new Blob([bytes], {
      type: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
    }),
    basename(fixture),
  );
  const response = await fetchWithCookies(new URL('/api/upload', baseUrl), { method: 'POST', body: form });
  return parseJsonResponse(response, 'upload');
}

async function signIn() {
  await postJson('/api/auth/magic-link/request', { email: smokeEmail });
  const link = await newestMagicLinkFor(smokeEmail);
  const token = new URL(link.magicLinkUrl).searchParams.get('token');
  if (!token) throw new Error(`Magic-link log entry did not include token: ${JSON.stringify(link)}`);
  const verified = await postJson('/api/auth/magic-link/verify', { token });
  if (!verified.user?.email) throw new Error(`Magic-link verify did not return a user: ${JSON.stringify(verified)}`);
  log('signed_in', { email: verified.user.email });
}

async function newestMagicLinkFor(email) {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    const entries = await readMagicLinkLog().catch(() => []);
    const matches = entries.filter((entry) => entry.to === email && entry.magicLinkUrl);
    const latest = matches.at(-1);
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

async function runAgent(threadId) {
  const prompt = [
    'This is a smoke test. Change slide 1 title to this exact text:',
    marker,
    '',
    'Use only the generic OOXML route for the edit:',
    '1. get_ooxml_capabilities with filter "pptx"',
    '2. inspect_current_with_ooxml for slide 1 text',
    '3. apply_ooxml_ops_to_current with command "pptx replace text" and args {"slide":1,"target":"title","text":"' + marker + '"}',
    '   Include expectedDocumentId and expectedVersionId from the inspection/status output.',
    '4. render_current_presentation_preview',
    '',
    'Do not use replace_text_in_current_document or set_current_presentation_slide_shape_text for this smoke.',
  ].join('\n');
  const admission = await postJson(`/flue/agents/ooxml-editor/${encodeURIComponent(threadId)}`, { message: prompt });
  log('admitted', { submissionId: admission.submissionId, offset: admission.offset });
  if (!admission.streamUrl || admission.offset === undefined || admission.offset === null) {
    return { assistantText: extractAgentText(admission), toolNames: new Set() };
  }
  return readAgentStream(admission);
}

async function readAgentStream(admission) {
  const streamUrl = new URL(admission.streamUrl, baseUrl);
  streamUrl.searchParams.set('offset', String(admission.offset));
  streamUrl.searchParams.set('live', 'sse');
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(new Error('Agent stream smoke timed out after 180s.')), 180_000);
  const response = await fetchWithCookies(streamUrl, {
    headers: { Accept: 'text/event-stream' },
    signal: controller.signal,
  });
  if (!response.ok || !response.body) {
    clearTimeout(timeout);
    throw new Error(`Stream failed with HTTP ${response.status}: ${await response.text()}`);
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  const toolNames = new Set();
  let assistantText = '';
  let buffer = '';
  let done = false;

  try {
    while (!done) {
      const read = await reader.read();
      if (read.done) break;
      buffer += decoder.decode(read.value, { stream: true });
      let boundary = buffer.indexOf('\n\n');
      while (boundary !== -1) {
        const block = buffer.slice(0, boundary);
        buffer = buffer.slice(boundary + 2);
        const parsed = parseSseBlock(block);
        if (parsed.event === 'control') {
          const control = safeJson(parsed.data);
          if (control?.streamClosed) done = true;
        }
        if (parsed.event === 'data') {
          const events = safeJson(parsed.data);
          const batch = Array.isArray(events) ? events : events ? [events] : [];
          for (const event of batch) {
            const eventDone = handleAgentEvent(event, { toolNames, appendText: (text) => (assistantText += text) });
            done = done || eventDone;
          }
        }
        boundary = buffer.indexOf('\n\n');
      }
    }
  } finally {
    clearTimeout(timeout);
    await reader.cancel().catch(() => {});
  }

  return { assistantText, toolNames };
}

function handleAgentEvent(event, state) {
  if (!event || typeof event !== 'object') return false;
  if (event.type === 'tool_start' || event.type === 'tool' || event.type === 'tool_call') {
    if (event.toolName) state.toolNames.add(event.toolName);
  }
  if (event.type === 'text_delta' && typeof event.text === 'string') {
    state.appendText(event.text);
  }
  if (event.type === 'message_end' && typeof event.message?.content?.[0]?.text === 'string') {
    state.appendText(event.message.content[0].text);
  }
  if (event.type === 'operation' && (event.isError || event.error)) {
    throw new Error(`Agent operation failed: ${JSON.stringify(event.error || event, null, 2)}`);
  }
  return event.type === 'idle';
}

function parseSseBlock(block) {
  let event = 'message';
  const data = [];
  for (const line of block.split(/\r?\n/)) {
    if (line.startsWith('event:')) event = line.slice(6).trim();
    if (line.startsWith('data:')) data.push(line.slice(5).trimStart());
  }
  return { event, data: data.join('\n') };
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
  log('validated', { errors });
}

async function assertSlideContains(file, expected) {
  const { stdout } = await execFileAsync(ooxmlBin, ['--json', 'pptx', 'slides', 'show', file, '--slide', '1', '--include-text'], {
    maxBuffer: 16 * 1024 * 1024,
  });
  if (!stdout.includes(expected)) {
    throw new Error(`Edited slide did not contain expected marker ${JSON.stringify(expected)}.\n${stdout}`);
  }
  log('readback', { containsMarker: true });
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
    const csrf = cookieJar.get('ooxml_csrf');
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
    if (!cookieValue) cookieJar.delete(name);
    else cookieJar.set(name, cookieValue);
  }
}

function splitCombinedSetCookie(value) {
  if (!value) return [];
  return value.split(/,(?=\s*[^;,=\s]+=)/g).map((part) => part.trim()).filter(Boolean);
}

function cookieHeader() {
  return [...cookieJar.entries()].map(([name, value]) => `${name}=${value}`).join('; ');
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

function safeJson(value) {
  try {
    return JSON.parse(value);
  } catch {
    return undefined;
  }
}

function extractAgentText(data) {
  if (typeof data?.result?.text === 'string') return data.result.text;
  if (typeof data?.text === 'string') return data.text;
  if (typeof data?.result === 'string') return data.result;
  return JSON.stringify(data);
}

function log(phase, data) {
  process.stderr.write(`${JSON.stringify({ ts: new Date().toISOString(), phase, ...data })}\n`);
}
