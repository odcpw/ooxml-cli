import { createHash, randomUUID } from 'node:crypto';
import { constants, createReadStream } from 'node:fs';
import { copyFile, mkdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { basename, extname, join } from 'node:path';
import { atomicWriteFile } from './fs-atomic.ts';
import { assertUploadSizes } from './upload-limits.ts';
import { absoluteVersionPath, assertSafeOoxmlZip, dataRoot, documentById, relativeToThread, threadDir, readThread, versionById, withThreadMutation, writeThread, type ThreadRecord, type UploadedOfficeFile } from './storage.ts';

type Folder = { id: string; name: string };
export type LibraryDeck = { id: string; name: string; originalName: string; sizeBytes: number; createdAt: string; folderId: string };
type Library = { folders: Folder[]; decks: LibraryDeck[] };
const queues = new Map<string, Promise<void>>();
function root(owner: string): string {
  if (!owner) throw Error('Sign in to use the library.');
  return join(dataRoot(), 'library', createHash('sha256').update(owner).digest('hex'));
}
function deckPath(owner: string, deck: LibraryDeck): string {
  if (!/^deck-[a-f0-9]{64}$/.test(deck.id) || !/^\.ppt[xm]$/i.test(extname(deck.originalName))) throw Error('Invalid library deck.');
  return join(root(owner), deck.id + extname(deck.originalName).toLowerCase());
}
export async function readLibrary(owner: string): Promise<Library> {
  try { return JSON.parse(await readFile(join(root(owner), 'index.json'), 'utf8')); }
  catch (error) { if ((error as NodeJS.ErrnoException).code === 'ENOENT') return { folders: [], decks: [] }; throw error; }
}
async function mutate<T>(owner: string, action: (library: Library) => Promise<T>, afterCommit?: (result: T) => Promise<void>): Promise<T> {
  const key = root(owner);
  const run = (queues.get(key) ?? Promise.resolve()).then(async () => {
    const library = await readLibrary(owner);
    const result = await action(library);
    await mkdir(key, { recursive: true });
    await atomicWriteFile(join(key, 'index.json'), JSON.stringify(library));
    if (afterCommit) await afterCommit(result);
    return result;
  });
  const done = run.then(() => undefined, () => undefined); queues.set(key, done);
  try { return await run; } finally { if (queues.get(key) === done) queues.delete(key); }
}
function folderExists(library: Library, folderId: string): void {
  if (folderId && !library.folders.some(folder => folder.id === folderId)) throw Error('Folder no longer exists.');
}
function label(value: unknown): string {
  if (typeof value !== 'string' || !value.trim() || value.trim().length > 160 || /[\x00-\x1f]/.test(value)) throw Error('Enter a name between 1 and 160 characters.');
  return value.trim();
}
function findDeck(library: Library, id: string): LibraryDeck {
  const deck = library.decks.find(deck => deck.id === id);
  if (!deck) throw Error('Deck not found in your library.');
  return deck;
}
async function save(owner: string, originalName: string, sizeBytes: number, digest: string, folderId: string, write: (path: string) => Promise<void>): Promise<LibraryDeck> {
  if (!/^\.ppt[xm]$/i.test(extname(originalName))) throw Error('Choose a PowerPoint file (.pptx or .pptm).');
  return mutate(owner, async library => {
    folderExists(library, folderId);
    const id = 'deck-' + digest;
    const existing = library.decks.find(deck => deck.id === id);
    if (existing) { existing.folderId = folderId; return existing; }
    const deck = { id, originalName: basename(originalName), name: basename(originalName), sizeBytes, createdAt: new Date().toISOString(), folderId };
    await mkdir(root(owner), { recursive: true });
    await write(deckPath(owner, deck));
    library.decks.unshift(deck);
    return deck;
  });
}
export async function uploadLibraryDeck(owner: string, file: UploadedOfficeFile, folderId = ''): Promise<LibraryDeck> {
  assertUploadSizes([{ size: file.bytes.byteLength }]); assertSafeOoxmlZip(file.bytes);
  return save(owner, file.originalName, file.bytes.byteLength, createHash('sha256').update(file.bytes).digest('hex'), folderId, path => writeFile(path, file.bytes));
}
export async function saveJobDeck(owner: string, threadId: string, documentId: string, versionId: string, folderId = ''): Promise<LibraryDeck> {
  return withThreadMutation(threadId, async () => {
    const thread = await readThread(threadId, owner);
    const version = versionById(documentById(thread, documentId), versionId);
    const path = absoluteVersionPath(thread, version);
    const hash = createHash('sha256'); for await (const chunk of createReadStream(path)) hash.update(chunk);
    return save(owner, version.originalName, (await stat(path)).size, hash.digest('hex'), folderId, target => copyFile(path, target, constants.COPYFILE_FICLONE));
  });
}
export async function libraryDownload(owner: string, id: string): Promise<{ deck: LibraryDeck; path: string }> {
  const deck = findDeck(await readLibrary(owner), id); return { deck, path: deckPath(owner, deck) };
}
export async function useLibraryDeck(owner: string, id: string, threadId?: string, ownerEmail?: string): Promise<ThreadRecord> {
  // Always take the thread lock before the library lock, including save-from-job.
  const targetId = threadId || 'thread-' + randomUUID();
  return withThreadMutation(targetId, () => mutate(owner, async library => {
    const deck = findDeck(library, id); const now = new Date().toISOString();
    const thread: ThreadRecord = threadId ? await readThread(threadId, owner) : { id: targetId, ownerUserId: owner, ownerEmail, title: deck.name, createdAt: now, updatedAt: now, currentDocumentId: '', documents: [] };
    const documentId = 'doc-' + randomUUID(); const dir = join(threadDir(targetId), 'documents', documentId);
    await mkdir(join(dir, 'versions'), { recursive: true }); await mkdir(join(dir, 'tmp'), { recursive: true }); await mkdir(join(dir, 'renders'), { recursive: true }); await mkdir(join(threadDir(targetId), 'tmp'), { recursive: true });
    const path = join(dir, 'versions', 'v0001-original' + extname(deck.originalName).toLowerCase());
    // Reflink when available, normal disk copy otherwise; never a writable hard link.
    await copyFile(deckPath(owner, deck), path, constants.COPYFILE_FICLONE);
    thread.documents.push({ id: documentId, title: deck.name, originalName: deck.originalName, createdAt: now, currentVersionId: 'v0001', versions: [{ id: 'v0001', originalName: deck.originalName, path: relativeToThread(targetId, path), createdAt: now, note: 'Copied from deck library', sizeBytes: deck.sizeBytes }] });
    if (!thread.currentDocumentId) thread.currentDocumentId = documentId;
    await writeThread(thread); return thread;
  }));
}
export async function createLibraryFolder(owner: string, name: unknown): Promise<Folder> {
  return mutate(owner, async library => {
    const clean = label(name); if (library.folders.some(folder => folder.name.toLowerCase() === clean.toLowerCase())) throw Error('A folder with that name already exists.');
    const folder = { id: 'folder-' + randomUUID(), name: clean }; library.folders.push(folder); return folder;
  });
}
export async function updateLibraryItem(owner: string, kind: 'decks' | 'folders', id: string, changes: { name?: unknown; folderId?: unknown }): Promise<void> {
  return mutate(owner, async library => {
    const item = library[kind].find(item => item.id === id); if (!item) throw Error('Library item no longer exists.');
    if (changes.name !== undefined) {
      const name = label(changes.name);
      if (kind === 'folders' && library.folders.some(folder => folder.id !== id && folder.name.toLowerCase() === name.toLowerCase())) throw Error('A folder with that name already exists.');
      item.name = name;
    }
    if (changes.folderId !== undefined) {
      if (kind !== 'decks' || typeof changes.folderId !== 'string') throw Error('Invalid folder.');
      folderExists(library, changes.folderId); (item as LibraryDeck).folderId = changes.folderId;
    }
  });
}
export async function removeLibraryItem(owner: string, kind: 'decks' | 'folders', id: string): Promise<void> {
  await mutate(owner, async library => {
    if (kind === 'folders') {
      if (!library.folders.some(folder => folder.id === id)) throw Error('Folder no longer exists.');
      library.folders = library.folders.filter(folder => folder.id !== id);
      for (const deck of library.decks) if (deck.folderId === id) deck.folderId = '';
    } else {
      const deck = findDeck(library, id);
      library.decks = library.decks.filter(deck => deck.id !== id);
      return deckPath(owner, deck);
    }
  }, async path => { if (path) await rm(path, { force: true }); });
}
