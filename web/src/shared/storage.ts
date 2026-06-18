import { randomUUID } from 'node:crypto';
import { mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { basename, extname, join, relative, resolve } from 'node:path';
import { withAppBasePath } from './app-url.ts';
import { isUploadExtensionSupported } from './file-support.ts';
import { runtimeDataRoot } from './runtime-paths.ts';
import { atomicWriteFile } from './fs-atomic.ts';

export type FileVersion = {
  id: string;
  originalName: string;
  path: string;
  createdAt: string;
  note: string;
  render?: RenderInfo;
};

export type RenderInfo = {
  dir: string;
  manifestPath?: string;
  pdfPath?: string;
  thumbnails: Array<{
    index: number;
    path: string;
    width?: number;
    height?: number;
  }>;
};

export type ThreadDocument = {
  id: string;
  title: string;
  originalName: string;
  createdAt: string;
  currentVersionId: string;
  versions: FileVersion[];
};

export type ThreadRecord = {
  id: string;
  ownerUserId?: string;
  ownerEmail?: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  currentDocumentId: string;
  documents: ThreadDocument[];
  currentVersionId?: string;
  versions?: FileVersion[];
};

export type UploadedOfficeFile = {
  originalName: string;
  bytes: Uint8Array;
};

const threadMutationQueues = new Map<string, Promise<void>>();

export function dataRoot(): string {
  return runtimeDataRoot();
}

export function threadsRoot(): string {
  return join(dataRoot(), 'threads');
}

export function safeId(value: string): string {
  const cleaned = value.replace(/[^a-zA-Z0-9._-]/g, '-').replace(/-+/g, '-');
  if (!cleaned || cleaned === '.' || cleaned === '..') {
    throw new Error('Invalid id');
  }
  return cleaned;
}

export function threadDir(threadId: string): string {
  return join(threadsRoot(), safeId(threadId));
}

export function documentDir(threadId: string, documentId: string): string {
  return join(threadDir(threadId), 'documents', safeId(documentId));
}

export function threadJsonPath(threadId: string): string {
  return join(threadDir(threadId), 'thread.json');
}

export async function ensureDataRoot(): Promise<void> {
  await mkdir(threadsRoot(), { recursive: true });
}

export async function threadExists(threadId: string, ownerUserId?: string): Promise<boolean> {
  try {
    await readThread(threadId, ownerUserId);
    return true;
  } catch {
    return false;
  }
}

export async function readThread(threadId: string, ownerUserId?: string): Promise<ThreadRecord> {
  const raw = await readFile(threadJsonPath(threadId), 'utf8');
  const thread = normalizeThread(JSON.parse(raw) as ThreadRecord);
  assertThreadOwner(thread, ownerUserId);
  return thread;
}

export async function listThreads(ownerUserId?: string, options: { limit?: number } = {}): Promise<ThreadRecord[]> {
  await ensureDataRoot();
  const entries = await readdir(threadsRoot(), { withFileTypes: true });
  const directories = entries.filter((entry) => entry.isDirectory());
  const threads: ThreadRecord[] = [];
  const batchSize = 8;
  for (let index = 0; index < directories.length; index += batchSize) {
    const batch = directories.slice(index, index + batchSize);
    const loaded = await Promise.all(
      batch.map(async (entry) => {
        try {
          return await readThread(entry.name);
        } catch {
          return undefined;
        }
      }),
    );
    for (const thread of loaded) {
      if (thread && (!ownerUserId || thread.ownerUserId === ownerUserId)) {
        threads.push(thread);
      }
    }
  }
  const limit = clampListLimit(options.limit);
  return threads
    .sort((a, b) => Date.parse(b.updatedAt) - Date.parse(a.updatedAt))
    .slice(0, limit);
}

export async function writeThread(thread: ThreadRecord): Promise<void> {
  normalizeThread(thread);
  const document = currentDocument(thread);
  thread.currentVersionId = document.currentVersionId;
  thread.versions = document.versions;
  thread.updatedAt = new Date().toISOString();
  await mkdir(threadDir(thread.id), { recursive: true });
  await atomicWriteFile(threadJsonPath(thread.id), `${JSON.stringify(thread, null, 2)}\n`);
}

export function currentDocument(thread: ThreadRecord): ThreadDocument {
  normalizeThread(thread);
  const document = thread.documents.find((candidate) => candidate.id === thread.currentDocumentId);
  if (!document) throw new Error(`Current document is missing: ${thread.currentDocumentId}`);
  return document;
}

export function documentById(thread: ThreadRecord, documentId: string): ThreadDocument {
  normalizeThread(thread);
  const safeDocumentId = safeId(documentId);
  const document = thread.documents.find((candidate) => candidate.id === safeDocumentId);
  if (!document) throw new Error(`Document not found: ${documentId}`);
  return document;
}

export function currentVersion(thread: ThreadRecord, document = currentDocument(thread)): FileVersion {
  const version = document.versions.find((candidate) => candidate.id === document.currentVersionId);
  if (!version) throw new Error(`Current version is missing: ${document.currentVersionId}`);
  return version;
}

export function versionById(document: ThreadDocument, versionId: string): FileVersion {
  const safeVersionId = safeId(versionId);
  const version = document.versions.find((candidate) => candidate.id === safeVersionId);
  if (!version) throw new Error(`Version not found: ${versionId}`);
  return version;
}

export function absoluteVersionPath(thread: ThreadRecord, version: FileVersion): string {
  return safeJoin(threadDir(thread.id), version.path);
}

export function relativeToThread(threadId: string, absolutePath: string): string {
  const base = threadDir(threadId);
  const rel = relative(base, absolutePath);
  if (rel.startsWith('..') || rel === '') throw new Error('Path escapes thread workspace');
  return rel;
}

export async function createThreadFromUploads(input: {
  files: UploadedOfficeFile[];
  title?: string;
  ownerUserId: string;
  ownerEmail?: string;
}): Promise<ThreadRecord> {
  if (input.files.length === 0) throw new Error('At least one Office file is required.');

  const id = `thread-${randomUUID()}`;
  const now = new Date().toISOString();
  const documents: ThreadDocument[] = [];
  const thread: ThreadRecord = {
    id,
    ownerUserId: input.ownerUserId,
    ownerEmail: input.ownerEmail,
    title: input.title?.trim() || uploadBaseName(input.files[0].originalName),
    createdAt: now,
    updatedAt: now,
    currentDocumentId: '',
    documents,
  };

  await mkdir(join(threadDir(id), 'tmp'), { recursive: true });
  for (const file of input.files) {
    documents.push(await writeUploadedDocument(thread.id, file, now));
  }
  thread.currentDocumentId = documents[0].id;
  await writeThread(thread);
  return thread;
}

export async function createThreadFromUpload(input: UploadedOfficeFile & { title?: string; ownerUserId: string; ownerEmail?: string }): Promise<ThreadRecord> {
  return createThreadFromUploads({
    files: [{ originalName: input.originalName, bytes: input.bytes }],
    title: input.title,
    ownerUserId: input.ownerUserId,
    ownerEmail: input.ownerEmail,
  });
}

export async function addDocumentsToThread(threadId: string, files: UploadedOfficeFile[], ownerUserId?: string): Promise<ThreadRecord> {
  if (files.length === 0) throw new Error('At least one Office file is required.');
  return withThreadMutation(threadId, async () => {
    const thread = await readThread(threadId, ownerUserId);
    const now = new Date().toISOString();
    const added: ThreadDocument[] = [];

    for (const file of files) {
      const document = await writeUploadedDocument(thread.id, file, now);
      thread.documents.push(document);
      added.push(document);
    }
    thread.currentDocumentId = added[0].id;
    await writeThread(thread);
    return thread;
  });
}

export async function selectDocument(threadId: string, documentId: string, ownerUserId?: string): Promise<ThreadRecord> {
  return withThreadMutation(threadId, async () => {
    const thread = await readThread(threadId, ownerUserId);
    const document = documentById(thread, documentId);
    thread.currentDocumentId = document.id;
    await writeThread(thread);
    return thread;
  });
}

export async function removeDocumentFromThread(threadId: string, documentId: string, ownerUserId?: string): Promise<ThreadRecord> {
  return withThreadMutation(threadId, async () => {
    const thread = await readThread(threadId, ownerUserId);
    const document = documentById(thread, documentId);
    if (thread.documents.length <= 1) {
      throw new Error('A thread must keep at least one document.');
    }

    thread.documents = thread.documents.filter((candidate) => candidate.id !== document.id);
    if (thread.currentDocumentId === document.id) {
      thread.currentDocumentId = thread.documents[0].id;
    }
    await writeThread(thread);

    if (document.id !== 'doc-primary') {
      await rm(documentDir(thread.id, document.id), { recursive: true, force: true });
    }
    return thread;
  });
}

export function nextVersionId(document: ThreadDocument): string {
  return `v${String(document.versions.length + 1).padStart(4, '0')}`;
}

export async function withThreadMutation<T>(threadId: string, fn: () => Promise<T>): Promise<T> {
  const key = safeId(threadId);
  const previous = threadMutationQueues.get(key) ?? Promise.resolve();
  const run = previous.catch(() => undefined).then(fn);
  const done = run.then(
    () => undefined,
    () => undefined,
  );
  threadMutationQueues.set(key, done);
  try {
    return await run;
  } finally {
    if (threadMutationQueues.get(key) === done) {
      threadMutationQueues.delete(key);
    }
  }
}

export function safeJoin(base: string, unsafePath: string): string {
  const resolvedBase = resolve(base);
  const resolved = resolve(resolvedBase, unsafePath);
  if (resolved !== resolvedBase && !resolved.startsWith(`${resolvedBase}/`)) {
    throw new Error('Path escapes workspace');
  }
  return resolved;
}

export function fileUrlFor(threadId: string, documentId: string, versionId: string): string {
  return withAppBasePath(`/api/threads/${encodeURIComponent(threadId)}/documents/${encodeURIComponent(documentId)}/versions/${encodeURIComponent(versionId)}/download`);
}

export function artifactUrl(threadId: string, documentId: string, versionId: string, artifactPath: string): string {
  return withAppBasePath(`/api/threads/${encodeURIComponent(threadId)}/documents/${encodeURIComponent(documentId)}/versions/${encodeURIComponent(versionId)}/artifact?path=${encodeURIComponent(artifactPath)}`);
}

function normalizeThread(thread: ThreadRecord): ThreadRecord {
  if (!Array.isArray(thread.documents) || thread.documents.length === 0) {
    const legacyVersions = Array.isArray(thread.versions) ? thread.versions : [];
    const first = legacyVersions[0];
    const document: ThreadDocument = {
      id: 'doc-primary',
      title: first?.originalName || thread.title || 'Document',
      originalName: first?.originalName || thread.title || 'Document',
      createdAt: first?.createdAt || thread.createdAt || new Date().toISOString(),
      currentVersionId: thread.currentVersionId || first?.id || 'v0001',
      versions: legacyVersions,
    };
    thread.documents = [document];
    thread.currentDocumentId = document.id;
  }

  if (!thread.currentDocumentId || !thread.documents.some((document) => document.id === thread.currentDocumentId)) {
    thread.currentDocumentId = thread.documents[0].id;
  }

  for (const document of thread.documents) {
    document.id = safeId(document.id);
    if (!document.currentVersionId && document.versions[0]) {
      document.currentVersionId = document.versions[0].id;
    }
    document.title ||= document.originalName;
  }

  return thread;
}

function assertThreadOwner(thread: ThreadRecord, ownerUserId: string | undefined): void {
  if (!ownerUserId) return;
  if (thread.ownerUserId !== ownerUserId) {
    throw new Error('Thread not found');
  }
}

// ── Upload safety: refuse zip bombs ──────────────────────────────────────────
// Office files are ZIP (OPC) packages. The upload limits cap the COMPRESSED
// size; a small file can still inflate to many GB and OOM the shared host on the
// first inspect/render. We read uncompressed sizes straight from the ZIP central
// directory (no decompression) and refuse implausible packages before persisting.
const maxUncompressedBytes = Math.max(
  1,
  Math.trunc(Number(process.env.OOXML_UPLOAD_MAX_UNCOMPRESSED_BYTES) || 500 * 1024 * 1024),
);
const maxCompressionRatio = Math.max(
  1,
  Math.trunc(Number(process.env.OOXML_UPLOAD_MAX_COMPRESSION_RATIO) || 300),
);

function assertSafeOoxmlZip(bytes: Uint8Array): void {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const len = bytes.byteLength;
  if (len < 22) throw new Error('Upload is not a valid Office package.');

  // Locate the End Of Central Directory record, scanning back over the optional comment.
  let eocd = -1;
  const lowBound = Math.max(0, len - (22 + 0xffff));
  for (let i = len - 22; i >= lowBound; i--) {
    if (view.getUint32(i, true) === 0x06054b50) {
      eocd = i;
      break;
    }
  }
  if (eocd < 0) throw new Error('Upload is not a valid Office package.');

  const entryCount = view.getUint16(eocd + 10, true);
  const cdOffset = view.getUint32(eocd + 16, true);
  // We do not parse ZIP64; treat its sentinels as untrusted-large and refuse.
  if (cdOffset === 0xffffffff || entryCount === 0xffff) {
    throw new Error('Upload uses an unsupported (zip64) package format or is too large.');
  }

  let totalUncompressed = 0;
  let totalCompressed = 0;
  let seen = 0;
  let p = cdOffset;
  while (p + 46 <= len && view.getUint32(p, true) === 0x02014b50) {
    const compressed = view.getUint32(p + 20, true);
    const uncompressed = view.getUint32(p + 24, true);
    const nameLen = view.getUint16(p + 28, true);
    const extraLen = view.getUint16(p + 30, true);
    const commentLen = view.getUint16(p + 32, true);
    if (uncompressed === 0xffffffff || compressed === 0xffffffff) {
      throw new Error('Upload uses an unsupported (zip64) package format or is too large.');
    }
    totalUncompressed += uncompressed;
    totalCompressed += compressed;
    if (totalUncompressed > maxUncompressedBytes) {
      throw new Error(
        `Upload expands to too much data (>${Math.round(maxUncompressedBytes / 1024 / 1024)} MB uncompressed) and was refused.`,
      );
    }
    if (++seen > 100_000) throw new Error('Upload has too many entries and was refused.');
    p += 46 + nameLen + extraLen + commentLen;
  }

  // Fail closed: a bogus/out-of-bounds central-directory offset (or a forged
  // EOCD) leaves the walk empty or short, which must NOT be accepted as safe.
  if (seen === 0 || seen !== entryCount) {
    throw new Error('Upload is not a valid Office package.');
  }

  if (
    totalCompressed > 0 &&
    totalUncompressed / totalCompressed > maxCompressionRatio &&
    totalUncompressed > 50 * 1024 * 1024
  ) {
    throw new Error('Upload has an implausible compression ratio and was refused.');
  }
}

async function writeUploadedDocument(threadId: string, input: UploadedOfficeFile, createdAt: string): Promise<ThreadDocument> {
  const originalBase = uploadBaseName(input.originalName);
  const ext = extname(originalBase).toLowerCase();
  if (!isUploadExtensionSupported(ext)) {
    throw new Error(`Unsupported Office file extension: ${ext || '(none)'}`);
  }
  assertSafeOoxmlZip(input.bytes);

  const documentId = `doc-${randomUUID()}`;
  const dir = documentDir(threadId, documentId);
  await mkdir(join(dir, 'versions'), { recursive: true });
  await mkdir(join(dir, 'renders'), { recursive: true });
  await mkdir(join(dir, 'tmp'), { recursive: true });

  const versionId = 'v0001';
  const versionPath = join('documents', documentId, 'versions', `${versionId}-original${ext}`);
  await writeFile(join(threadDir(threadId), versionPath), input.bytes);

  return {
    id: documentId,
    title: originalBase,
    originalName: originalBase,
    createdAt,
    currentVersionId: versionId,
    versions: [
      {
        id: versionId,
        originalName: originalBase,
        path: versionPath,
        createdAt,
        note: 'Uploaded original',
      },
    ],
  };
}

function uploadBaseName(originalName: string): string {
  return basename(originalName).replace(/[^\w.\- ()]/g, '_') || 'document';
}

function clampListLimit(value: number | undefined): number {
  if (value === undefined || !Number.isFinite(value)) return 100;
  return Math.max(1, Math.min(200, Math.trunc(value)));
}
