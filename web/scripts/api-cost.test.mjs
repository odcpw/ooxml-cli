import test from 'node:test';
import assert from 'node:assert/strict';
import { DatabaseSync } from 'node:sqlite';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { apiCostSummary } from '../src/shared/api-cost.ts';
import { createThreadFromUploads } from '../src/shared/storage.ts';

test('durable costs isolate owners, count individual calls once, include failed partial calls, and read spilled batches', async () => {
  const old = process.env.OOXML_WEB_DATA_DIR; const dir = await mkdtemp(join(tmpdir(), 'ooxml-cost-')); process.env.OOXML_WEB_DATA_DIR = dir;
  let db;
  try {
    assert.equal((await apiCostSummary('a')).total.usd, 0);
    const bytes = await readFile(new URL('../../testdata/pptx/minimal-title/presentation.pptx', import.meta.url));
    const make = ownerUserId => createThreadFromUploads({ ownerUserId, files: [{ originalName: 'Source.pptx', bytes }] });
    const a = await make('a'), second = await make('a'), other = await make('b');
    db = new DatabaseSync(join(dir, 'flue.db'));
    db.exec('CREATE TABLE flue_conversation_streams(path TEXT, identity_json TEXT); CREATE TABLE flue_conversation_stream_batches(path TEXT, seq INTEGER, data TEXT); CREATE TABLE flue_conversation_stream_batch_chunks(path TEXT, seq INTEGER, chunk_index INTEGER, chunk_count INTEGER, data TEXT);');
    const stream = db.prepare('INSERT INTO flue_conversation_streams VALUES (?,?)');
    const batch = db.prepare('INSERT INTO flue_conversation_stream_batches VALUES (?,?,?)');
    for (const thread of [a, second, other]) stream.run(thread.id, JSON.stringify({ agentName: 'ooxml-editor', instanceId: thread.id }));
    const call = (id, cost, extra = {}) => ({ id, type: 'assistant_message_completed', timestamp: '2026-09-10T10:00:00Z', usage: { cost: { total: cost } }, ...extra });
    batch.run(a.id, 0, JSON.stringify([call('one', .12), call('two', .03, { stopReason: 'error' }), { id: 'summary', type: 'submission_settled', usage: { cost: { total: .15 } } }]));
    batch.run(a.id, 1, JSON.stringify([call('one', .12), call('unknown', undefined)]));
    batch.run(other.id, 0, JSON.stringify([call('foreign', 999)]));
    const spill = JSON.stringify([call('three', .25)]); const mid = Math.floor(spill.length / 2);
    batch.run(second.id, 0, JSON.stringify({ $flueChunkCount: 2 }));
    const chunk = db.prepare('INSERT INTO flue_conversation_stream_batch_chunks VALUES (?,?,?,?,?)');
    chunk.run(second.id, 0, 0, 2, spill.slice(0, mid)); chunk.run(second.id, 0, 1, 2, spill.slice(mid));
    const result = await apiCostSummary('a', a.id);
    assert.equal(result.total.usd, .4); assert.equal(result.job.usd, .15); assert.equal(result.total.calls, 4); assert.equal(result.total.unpricedCalls, 1);
    assert.deepEqual(await apiCostSummary('a', a.id), result);
    await assert.rejects(apiCostSummary('a', other.id), /Thread not found/);
    assert.equal((await apiCostSummary('b')).total.usd, 999);
    db.exec('DELETE FROM flue_conversation_stream_batch_chunks WHERE chunk_index = 1');
    await assert.rejects(apiCostSummary('a'), /incomplete/);
  } finally { db?.close(); if (old === undefined) delete process.env.OOXML_WEB_DATA_DIR; else process.env.OOXML_WEB_DATA_DIR = old; await rm(dir, { recursive: true, force: true }); }
});
