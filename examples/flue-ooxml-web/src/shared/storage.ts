import { randomUUID } from 'node:crypto';
import { mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { basename, extname, join, relative, resolve } from 'node:path';
import { runtimeDataRoot } from './runtime-paths.ts';

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

const allowedExtensions = new Set(['.pptx', '.pptm', '.docx', '.xlsx', '.xlsm']);

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

export async function listThreads(ownerUserId?: string): Promise<ThreadRecord[]> {
  await ensureDataRoot();
  const entries = await readdir(threadsRoot(), { withFileTypes: true });
  const threads = await Promise.all(
    entries
      .filter((entry) => entry.isDirectory())
      .map(async (entry) => {
        try {
          return await readThread(entry.name);
        } catch {
          return undefined;
        }
      }),
  );
  return threads
    .filter((thread): thread is ThreadRecord => Boolean(thread))
    .filter((thread) => !ownerUserId || thread.ownerUserId === ownerUserId)
    .sort((a, b) => Date.parse(b.updatedAt) - Date.parse(a.updatedAt));
}

export async function writeThread(thread: ThreadRecord): Promise<void> {
  normalizeThread(thread);
  const document = currentDocument(thread);
  thread.currentVersionId = document.currentVersionId;
  thread.versions = document.versions;
  thread.updatedAt = new Date().toISOString();
  await mkdir(threadDir(thread.id), { recursive: true });
  await writeFile(threadJsonPath(thread.id), `${JSON.stringify(thread, null, 2)}\n`);
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
}

export async function selectDocument(threadId: string, documentId: string, ownerUserId?: string): Promise<ThreadRecord> {
  const thread = await readThread(threadId, ownerUserId);
  const document = documentById(thread, documentId);
  thread.currentDocumentId = document.id;
  await writeThread(thread);
  return thread;
}

export async function removeDocumentFromThread(threadId: string, documentId: string, ownerUserId?: string): Promise<ThreadRecord> {
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
}

export function nextVersionId(document: ThreadDocument): string {
  return `v${String(document.versions.length + 1).padStart(4, '0')}`;
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
  return `/api/threads/${encodeURIComponent(threadId)}/documents/${encodeURIComponent(documentId)}/versions/${encodeURIComponent(versionId)}/download`;
}

export function artifactUrl(threadId: string, documentId: string, versionId: string, artifactPath: string): string {
  return `/api/threads/${encodeURIComponent(threadId)}/documents/${encodeURIComponent(documentId)}/versions/${encodeURIComponent(versionId)}/artifact?path=${encodeURIComponent(artifactPath)}`;
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

async function writeUploadedDocument(threadId: string, input: UploadedOfficeFile, createdAt: string): Promise<ThreadDocument> {
  const originalBase = uploadBaseName(input.originalName);
  const ext = extname(originalBase).toLowerCase();
  if (!allowedExtensions.has(ext)) {
    throw new Error(`Unsupported Office file extension: ${ext || '(none)'}`);
  }

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
