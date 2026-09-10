import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { uploadLibraryDeck, readLibrary, createLibraryFolder, updateLibraryItem, removeLibraryItem, useLibraryDeck, saveJobDeck, libraryDownload } from '../src/shared/deck-library.ts';
import { absoluteVersionPath, createThreadFromUploads } from '../src/shared/storage.ts';

test('library reuse is independent, owner isolated, deduplicated, and survives folder removal', async () => {
  const previous = process.env.OOXML_WEB_DATA_DIR;
  const dir = await mkdtemp(join(tmpdir(), 'ooxml-library-')); process.env.OOXML_WEB_DATA_DIR = dir;
  try {
    const bytes = await readFile(new URL('../../testdata/pptx/minimal-title/presentation.pptx', import.meta.url));
    const folder = await createLibraryFolder('owner', 'References');
    const deck = await uploadLibraryDeck('owner', { originalName: 'French.pptx', bytes }, folder.id);
    assert.equal((await uploadLibraryDeck('owner', { originalName: 'Again.pptx', bytes })).id, deck.id);
    assert.equal((await readLibrary('owner')).decks.length, 1);
    assert.deepEqual(await readLibrary('outsider'), { folders: [], decks: [] });
    await assert.rejects(useLibraryDeck('outsider', deck.id), /not found/);
    await assert.rejects(libraryDownload('outsider', deck.id), /not found/);
    await assert.rejects(updateLibraryItem('outsider', 'decks', deck.id, { name: 'No' }), /no longer exists/);
    const thread = await useLibraryDeck('owner', deck.id);
    const path = absoluteVersionPath(thread, thread.documents[0].versions[0]);
    assert.deepEqual(await readFile(path), bytes);
    await writeFile(path, 'changed job bytes');
    assert.deepEqual(await readFile((await libraryDownload('owner', deck.id)).path), bytes);
    const foreign = await createThreadFromUploads({ files: [{ originalName: 'Other.pptx', bytes }], ownerUserId: 'outsider' });
    await assert.rejects(useLibraryDeck('owner', deck.id, foreign.id), /Thread not found/);
    await assert.rejects(saveJobDeck('outsider', thread.id, thread.documents[0].id, 'v0001'), /Thread not found/);
    await updateLibraryItem('owner', 'decks', deck.id, { name: 'French reference', folderId: '' });
    await updateLibraryItem('owner', 'decks', deck.id, { folderId: folder.id });
    await removeLibraryItem('owner', 'folders', folder.id);
    assert.equal((await readLibrary('owner')).decks[0].folderId, '');
    assert.equal((await readLibrary('owner')).decks[0].name, 'French reference');
    const second = await useLibraryDeck('owner', deck.id);
    const secondPath = absoluteVersionPath(second, second.documents[0].versions[0]);
    await removeLibraryItem('owner', 'decks', deck.id);
    assert.equal((await readLibrary('owner')).decks.length, 0);
    assert.deepEqual(await readFile(secondPath), bytes);
    const saved = await saveJobDeck('owner', second.id, second.documents[0].id, 'v0001');
    assert.deepEqual(await readFile((await libraryDownload('owner', saved.id)).path), bytes);
    await assert.rejects(updateLibraryItem('owner', 'decks', saved.id, { folderId: '../escape' }), /Folder no longer exists/);
    await assert.rejects(createLibraryFolder('owner', ' '), /Enter a name/);
    await assert.rejects(uploadLibraryDeck('owner', { originalName: 'bad.pptx', bytes: Buffer.from('not zip') }), /Office package/);
  } finally { if (previous === undefined) delete process.env.OOXML_WEB_DATA_DIR; else process.env.OOXML_WEB_DATA_DIR = previous; await rm(dir, { recursive: true, force: true }); }
});

test('concurrent library moves and renames preserve both changes', async () => {
  const previous = process.env.OOXML_WEB_DATA_DIR; const dir = await mkdtemp(join(tmpdir(), 'ooxml-library-queue-')); process.env.OOXML_WEB_DATA_DIR = dir;
  try {
    const bytes = await readFile(new URL('../../testdata/pptx/minimal-title/presentation.pptx', import.meta.url));
    const deck = await uploadLibraryDeck('owner', { originalName: 'Source.pptx', bytes }); const folder = await createLibraryFolder('owner', 'Training');
    await Promise.all([updateLibraryItem('owner', 'decks', deck.id, { folderId: folder.id }), updateLibraryItem('owner', 'decks', deck.id, { name: 'Safety training' })]);
    const library = await readLibrary('owner'); assert.equal(library.decks[0].folderId, folder.id); assert.equal(library.decks[0].name, 'Safety training');
    await assert.rejects(createLibraryFolder('owner', 'training'), /already exists/);
  } finally { if (previous === undefined) delete process.env.OOXML_WEB_DATA_DIR; else process.env.OOXML_WEB_DATA_DIR = previous; await rm(dir, { recursive: true, force: true }); }
});
