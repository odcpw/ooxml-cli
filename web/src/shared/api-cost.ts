import { DatabaseSync } from 'node:sqlite';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { runtimeDbPath, runtimeDataRoot } from './runtime-paths.ts';
import { readThread } from './storage.ts';

type Cost = { usd: number; calls: number; unpricedCalls: number };
function emptyCost(): Cost { return { usd: 0, calls: 0, unpricedCalls: 0 }; }

// Read Flue 2's durable completion records, not the browser event stream: reloads,
// reconnects and multiple tabs must not add the same call to the total again.
export async function apiCostSummary(ownerUserId: string, threadId?: string) {
  if (!ownerUserId) throw Error('Sign in to view API costs.');
  if (threadId) await readThread(threadId, ownerUserId);
  const total = emptyCost(), job = emptyCost();
  let since: string | null = null;
  const codexPath = join(runtimeDataRoot(), 'codex-jobs.db');
  if (existsSync(codexPath)) {
    const codex = new DatabaseSync(codexPath, { readOnly: true });
    try {
      for (const row of codex.prepare('SELECT data FROM usage').all()) {
        const usage = JSON.parse(String(row.data));
        if (usage.ownerId !== ownerUserId) continue;
        const cost = codexUsageCost(usage);
        for (const bucket of usage.threadId === threadId ? [total, job] : [total]) {
          bucket.calls++; if (cost === null) bucket.unpricedCalls++; else bucket.usd += cost;
        }
        if (!since || usage.timestamp < since) since = usage.timestamp;
      }
    } finally { codex.close(); }
  }
  const result = () => ({ total, job: threadId ? job : null, since, currency: 'USD', estimated: true });
  if (!existsSync(runtimeDbPath())) return result();
  const db = new DatabaseSync(runtimeDbPath(), { readOnly: true });
  try {
    const streams = db.prepare('SELECT path, identity_json FROM flue_conversation_streams').all();
    const batches = db.prepare('SELECT seq, data FROM flue_conversation_stream_batches WHERE path = ? ORDER BY seq');
    for (const stream of streams) {
      const identity = JSON.parse(String(stream.identity_json));
      if (identity.agentName !== 'ooxml-editor' || typeof identity.instanceId !== 'string') continue;
      try { await readThread(identity.instanceId, ownerUserId); }
      catch (error) {
        if ((error as NodeJS.ErrnoException).code === 'ENOENT' || (error as Error).message === 'Thread not found') continue;
        throw error;
      }
      const seen = new Set<string>();
      for (const batch of batches.iterate(stream.path)) {
        let records = JSON.parse(String(batch.data));
        if (!Array.isArray(records)) {
          const count = records.$flueChunkCount;
          if (!Number.isSafeInteger(count) || count < 1) throw Error('Unrecognized usage record format.');
          const chunks = db.prepare('SELECT chunk_index, chunk_count, data FROM flue_conversation_stream_batch_chunks WHERE path = ? AND seq = ? ORDER BY chunk_index').all(stream.path, batch.seq);
          if (chunks.length !== count || chunks.some((chunk, i) => chunk.chunk_index !== i || chunk.chunk_count !== count)) throw Error('Usage records are incomplete.');
          records = JSON.parse(chunks.map(chunk => String(chunk.data)).join(''));
          if (!Array.isArray(records)) throw Error('Unrecognized usage record format.');
        }
        for (const record of records) {
          if (record.type !== 'assistant_message_completed') continue;
          if (typeof record.id !== 'string') throw Error('Usage record has no identity.');
          if (seen.has(record.id)) continue; seen.add(record.id);
          const cost = record.usage?.cost?.total;
          for (const bucket of identity.instanceId === threadId ? [total, job] : [total]) {
            bucket.calls++;
            if (typeof cost === 'number' && Number.isFinite(cost) && cost >= 0) bucket.usd += cost;
            else bucket.unpricedCalls++;
          }
          if (typeof record.timestamp === 'string' && Number.isFinite(Date.parse(record.timestamp)) && (!since || Date.parse(record.timestamp) < Date.parse(since))) since = record.timestamp;
        }
      }
    }
    return result();
  } finally { db.close(); }
}

// Cumulative snapshots carry their accumulated per-request estimate. Missing
// prices stay explicitly unknown instead of displaying an invented zero.
export function codexUsageCost(usage: Record<string, any>): number | null {
  return typeof usage.usd === 'number' && Number.isFinite(usage.usd) && usage.usd >= 0 ? usage.usd : null;
}
