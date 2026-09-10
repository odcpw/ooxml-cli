import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtemp, rm, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import vm from 'node:vm';
import { CodexStore, assertSlideCheckpoint } from '../src/shared/codex-store.ts';
import { apiCostSummary } from '../src/shared/api-cost.ts';
import { createThreadFromUploads } from '../src/shared/storage.ts';
import { workbenchHtml } from '../src/page.ts';

test('translation coverage refuses unread slides, stale readbacks and invalid slide numbers', () => {
  const job = { slideCount: 3, readbackVersions: { 1: 'v2', 2: 'v1' } };
  assert.doesNotThrow(() => assertSlideCheckpoint(job, [1], 'Translated and read back', 'v2'));
  assert.throws(() => assertSlideCheckpoint(job, [1, 2], 'Done', 'v2'), /Read back/);
  assert.throws(() => assertSlideCheckpoint(job, [3], 'Done', 'v2'), /Read back/);
  assert.throws(() => assertSlideCheckpoint(job, [4], 'Done', 'v2'), /valid reviewed/);
});

test('queued jobs, slide checkpoints and event cursors survive a process restart; duplicate admission cannot create two active jobs', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'codex-queue-'));
  const path = join(dir, 'jobs.db'); let store = new CodexStore(path);
  try {
    const job = { id: 'one', threadId: 'thread-one', ownerId: 'owner', status: 'running', slides: { 1: 'saved French text' }, codexThreadId: 'durable-codex-thread' };
    store.save(job); store.event(job, { type: 'message-delta', delta: 'saved' });
    assert.throws(() => store.save({ ...job, id: 'duplicate' }), /UNIQUE/);
    const cursor = store.events(job.threadId, 0).next;
    store.db.close(); store = new CodexStore(path);
    assert.equal(store.active().length, 1);
    assert.equal(store.active()[0].codexThreadId, 'durable-codex-thread');
    assert.deepEqual(store.active()[0].slides, { 1: 'saved French text' });
    assert.deepEqual(store.events('another-owner-thread', 0).events, []);
    assert.deepEqual(store.events(job.threadId, cursor).events, []);
    job.status = 'completed'; store.save(job); store.event(job, { type: 'submission-settled', outcome: 'completed' });
    assert.equal(store.events(job.threadId, cursor).events[0].type, 'submission-settled');
    assert.equal(store.active().length, 0);
  } finally { store.db.close(); await rm(dir, { recursive: true, force: true }); }
});

test('Codex costs count cumulative updates once, price each request tier and isolate owners', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'codex-cost-')); const previous = process.env.OOXML_WEB_DATA_DIR; process.env.OOXML_WEB_DATA_DIR = dir;
  let store;
  try {
    const thread = await createThreadFromUploads({ ownerUserId: 'owner', files: [{ originalName: 'deck.pptx', bytes: await readFile(new URL('../../testdata/pptx/minimal-title/presentation.pptx', import.meta.url)) }] });
    store = new CodexStore();
    const job = { id: 'cost-job', threadId: thread.id, ownerId: 'owner' };
    const tokens = { inputTokens: 1000, cachedInputTokens: 100, outputTokens: 100 };
    store.usage(job, 'total', tokens); store.usage(job, 'total', tokens);
    assert.equal((await apiCostSummary('owner', thread.id)).total.usd, 0.0141);
    store.usage(job, 'total', { inputTokens: 301000, cachedInputTokens: 100, outputTokens: 200 }, { inputTokens: 300000 });
    assert.equal((await apiCostSummary('owner')).total.usd, 6.0216);
    const followup = { ...job, id: 'followup', createdAt: new Date().toISOString() };
    store.inheritUsage(followup, job.id);
    store.usage(followup, 'total', { inputTokens: 302000, cachedInputTokens: 200, outputTokens: 300 }, { inputTokens: 1000 });
    assert.ok(Math.abs((await apiCostSummary('owner')).total.usd - 6.0357) < 1e-10, 'Follow-up usage must exclude already billed history');
    assert.equal((await apiCostSummary('outsider')).total.usd, 0);
    await assert.rejects(apiCostSummary('outsider', thread.id), /Thread not found/);
  } finally { store?.db.close(); if (previous === undefined) delete process.env.OOXML_WEB_DATA_DIR; else process.env.OOXML_WEB_DATA_DIR = previous; await rm(dir, { recursive: true, force: true }); }
});

test('the shipped browser polls Codex updates without opening SSE, and settles only its saved job', async () => {
  const script = workbenchHtml().match(/<script>([\s\S]*?)<\/script>/)[1]; new vm.Script(script);
  const start = script.indexOf('async function streamAgentEvents(admission)');
  const end = script.indexOf('function renderMarkdown(', start);
  const messages = []; let calls = 0;
  const context = vm.createContext({
    URL, AbortController, AbortSignal, Date, setTimeout, clearTimeout, setInterval, clearInterval,
    EventSource: class { constructor() { throw Error('SSE must not be opened for Codex'); } },
    state: {}, chat: {}, AGENT_IDLE_TIMEOUT_MS: 60000,
    normalizedEventStreamUrl: value => new URL(value, 'https://example.com'),
    renderMarkdown: value => value, readableError: String, toolTraceText: () => '',
    addMessage: (kind, text) => { const item = { kind, textContent: text }; messages.push(item); return item; },
    apiFetch: async () => { calls++; return { ok: true, headers: new Headers({ 'stream-next-offset': '8', 'stream-up-to-date': 'true' }), json: async () => [
      { type: 'submission-settled', submissionId: 'older-job', outcome: 'completed', position: { batch: 6, index: 0 } },
      { type: 'message-delta', kind: 'text', delta: 'Translation saved', position: { batch: 7, index: 0 } },
      { type: 'submission-settled', submissionId: 'saved-job', outcome: 'completed', position: { batch: 8, index: 0 } },
    ] }; },
  });
  vm.runInContext(script.slice(start, end), context);
  await context.streamAgentEvents({ transport: 'poll', streamUrl: '/api/threads/thread/agent', offset: '5', submissionId: 'saved-job' });
  assert.equal(calls, 1); assert.equal(messages.find(m => m.kind === 'assistant').innerHTML, 'Translation saved');
});
