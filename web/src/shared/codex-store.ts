import { DatabaseSync } from 'node:sqlite';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';
import { ensureRuntimeDir, runtimeDataRoot } from './runtime-paths.ts';

export type Job = {
  id: string; threadId: string; ownerId: string; prompt: string;
  status: 'queued' | 'running' | 'completed' | 'failed';
  codexThreadId?: string; sourceId: string; initialVersion: string;
  workflow: any; slideCount: number; slides: Record<string, string>; eventOffset?: number; readbackVersions?: Record<string, string>;
  error?: string; createdAt: string; updatedAt: string;
};

export class CodexStore {
  db: DatabaseSync;
  constructor(path = join(runtimeDataRoot(), 'codex-jobs.db')) {
    ensureRuntimeDir(path); this.db = new DatabaseSync(path);
    this.db.exec(`PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;
      CREATE TABLE IF NOT EXISTS jobs (id TEXT PRIMARY KEY, thread_id TEXT NOT NULL, status TEXT NOT NULL, data TEXT NOT NULL);
      CREATE UNIQUE INDEX IF NOT EXISTS one_active_job ON jobs(thread_id) WHERE status IN ('queued','running');
      CREATE TABLE IF NOT EXISTS events (seq INTEGER PRIMARY KEY AUTOINCREMENT, thread_id TEXT NOT NULL, job_id TEXT NOT NULL, data TEXT NOT NULL);
      CREATE INDEX IF NOT EXISTS events_thread ON events(thread_id,seq);
      CREATE TABLE IF NOT EXISTS usage (job_id TEXT NOT NULL, turn_id TEXT NOT NULL, data TEXT NOT NULL, PRIMARY KEY(job_id,turn_id));`);
  }
  save(job: Job) {
    job.updatedAt = new Date().toISOString();
    this.db.prepare('INSERT INTO jobs VALUES (?,?,?,?) ON CONFLICT(id) DO UPDATE SET status=excluded.status,data=excluded.data').run(job.id, job.threadId, job.status, JSON.stringify(job));
  }
  latest(threadId: string): Job | undefined {
    const row = this.db.prepare('SELECT data FROM jobs WHERE thread_id=? ORDER BY rowid DESC LIMIT 1').get(threadId);
    return row ? JSON.parse(String(row.data)) : undefined;
  }
  active(): Job[] { return this.db.prepare("SELECT data FROM jobs WHERE status IN ('queued','running') ORDER BY rowid").all().map(r => JSON.parse(String(r.data))); }
  event(job: Job, data: Record<string, unknown>) {
    this.db.prepare('INSERT INTO events(thread_id,job_id,data) VALUES(?,?,?)').run(job.threadId, job.id, JSON.stringify(data));
  }
  events(threadId: string, after: number) {
    const rows = this.db.prepare('SELECT seq,data FROM events WHERE thread_id=? AND seq>? ORDER BY seq LIMIT 200').all(threadId, after);
    return { events: rows.map(r => ({ ...JSON.parse(String(r.data)), position: { batch: Number(r.seq), index: 0 } })), next: rows.length ? Number(rows.at(-1)!.seq) : after, upToDate: rows.length < 200 };
  }
  offset(threadId: string): number { return Number(this.db.prepare('SELECT COALESCE(MAX(seq),0) AS seq FROM events WHERE thread_id=?').get(threadId)!.seq); }
  usage(job: Job, turnId: string, tokens: any, last: any = tokens) {
    const row = this.db.prepare('SELECT data FROM usage WHERE job_id=? AND turn_id=?').get(job.id, turnId);
    const previous = row ? JSON.parse(String(row.data)) : {};
    const model = process.env.OOXML_CODEX_MODEL || 'gpt-6-astra';
    const delta = (key: string) => tokens[key] - (previous[key] || 0);
    const input = delta('inputTokens'), cached = delta('cachedInputTokens'), output = delta('outputTokens');
    const valid = [input, cached, output].every(n => Number.isFinite(n) && n >= 0) && cached <= input;
    const large = last.inputTokens > 272000;
    const usd = model === 'gpt-6-astra' && valid && (!row || previous.usd !== null)
      ? (previous.usd || 0) + ((input - cached) * (large ? 20 : 10) + cached * (large ? 2 : 1) + output * (large ? 75 : 50)) / 1_000_000 : null;
    this.db.prepare('INSERT INTO usage VALUES(?,?,?) ON CONFLICT(job_id,turn_id) DO UPDATE SET data=excluded.data').run(job.id, turnId, JSON.stringify({ ownerId: job.ownerId, threadId: job.threadId, timestamp: new Date().toISOString(), model, ...tokens, usd }));
  }
}
export function newJobId() { return 'job-' + randomUUID(); }

export function assertSlideCheckpoint(job: Job, slides: unknown, note: unknown, versionId: string): asserts slides is number[] {
  if (!Array.isArray(slides) || !slides.length || slides.some(n => !Number.isInteger(n) || n < 1 || n > job.slideCount) || typeof note !== 'string' || !note.trim()) throw Error('Provide valid reviewed slide numbers and a description of the saved translation or why no change was needed.');
  if (slides.some(n => job.readbackVersions?.[String(n)] !== versionId)) throw Error('Read back each slide in the current saved source version with show_current_presentation_slide before recording progress.');
}
