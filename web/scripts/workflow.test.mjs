import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import vm from 'node:vm';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { workbenchHtml } from '../src/page.ts';
import { validateWorkflow } from '../src/shared/workflow.ts';
import { createThreadFromUploads, readThread, saveWorkflow, selectDocument, removeDocumentFromThread } from '../src/shared/storage.ts';
import { absoluteVersionPath } from '../src/shared/storage.ts';
import { createTemplateFormSlideFromCurrent } from '../src/shared/ooxml-actions.ts';

const settings = { mode: 'translate', sourceDocumentId: 'de', templateDocumentId: '', referenceDocumentIds: ['fr'], language: 'Italian', glossary: 'Arbeitssicherheit → sicurezza sul lavoro' };

test('chat displays readable download links without enabling executable URLs or HTML', () => {
  const script = workbenchHtml().match(/<script>([\s\S]*?)<\/script>/)[1];
  const functions = script.slice(script.indexOf('function renderMarkdown('), script.indexOf('function toolTraceText('));
  const render = vm.runInNewContext(functions + '; renderMarkdown', { appUrl: url => url });
  const rendered = render('[Download Italian deck](/api/threads/example/download)');
  assert.match(rendered, /href="\/api\/threads\/example\/download"/);
  assert.match(rendered, />Download Italian deck<\/a>/);
  const mounted = render('[Download Italian deck]( /ooxml/api/threads/example/download )');
  assert.match(mounted, /href="\/ooxml\/api\/threads\/example\/download"/);
  assert.match(mounted, />Download Italian deck<\/a>/);
  assert.doesNotMatch(mounted, /\[Download/);
  assert.doesNotMatch(render('[bad](javascript:alert(1))'), /<a /);
  assert.doesNotMatch(render('<img src=x onerror=alert(1)>'), /<img/);
});

test('roles reject missing documents, conflicting assignments and invalid terms', () => {
  assert.deepEqual(validateWorkflow(settings, ['de', 'fr']), settings);
  assert.throws(() => validateWorkflow(settings, ['de']), /no longer available/);
  assert.throws(() => validateWorkflow({ ...settings, templateDocumentId: 'de' }, ['de', 'fr']), /different files/);
  assert.throws(() => validateWorkflow({ ...settings, referenceDocumentIds: ['de'] }, ['de', 'fr']), /different files/);
  assert.throws(() => validateWorkflow({ ...settings, glossary: 'x'.repeat(50001) }, ['de', 'fr']), /glossary/);
});

test('saved roles survive reopening and source targeting is independent of reference selection', async () => {
  const previous = process.env.OOXML_WEB_DATA_DIR;
  const dir = await mkdtemp(join(tmpdir(), 'ooxml-workflow-'));
  process.env.OOXML_WEB_DATA_DIR = dir;
  try {
    const bytes = await readFile(new URL('../../testdata/pptx/minimal-title/presentation.pptx', import.meta.url));
    const thread = await createThreadFromUploads({ files: [{ originalName: 'German.pptx', bytes }, { originalName: 'French.pptx', bytes }], ownerUserId: 'owner' });
    const [source, reference] = thread.documents;
    const workflow = { ...settings, sourceDocumentId: source.id, referenceDocumentIds: [reference.id] };
    await saveWorkflow(thread.id, workflow, 'owner');
    await selectDocument(thread.id, reference.id, 'owner');
    assert.deepEqual((await readThread(thread.id, 'owner')).workflow, workflow);
    const restored = await saveWorkflow(thread.id, workflow, 'owner');
    assert.equal(restored.currentDocumentId, source.id);
    await assert.rejects(saveWorkflow(thread.id, workflow, 'another-user'), /Thread not found/);
    assert.deepEqual((await readThread(thread.id, 'owner')).workflow, workflow);
    await removeDocumentFromThread(thread.id, reference.id, 'owner');
    assert.deepEqual((await readThread(thread.id, 'owner')).workflow.referenceDocumentIds, []);
  } finally {
    if (previous === undefined) delete process.env.OOXML_WEB_DATA_DIR;
    else process.env.OOXML_WEB_DATA_DIR = previous;
    await rm(dir, { recursive: true, force: true });
  }
});

test('real template tool maps centered titles, subtitles and content placeholders without dropping text', { skip: !process.env.OOXML_BIN }, async () => {
  const previous = process.env.OOXML_WEB_DATA_DIR;
  const dir = await mkdtemp(join(tmpdir(), 'ooxml-template-workflow-'));
  process.env.OOXML_WEB_DATA_DIR = dir;
  const run = promisify(execFile);
  const slideText = async (file, slide) => {
    const { stdout } = await run(process.env.OOXML_BIN, ['--json', 'pptx', 'shapes', 'show', file, '--slide', String(slide), '--include-text']);
    return JSON.parse(stdout).shapes.map(shape => shape.textPreview || '').filter(Boolean).join('\n');
  };
  try {
    const thread = await createThreadFromUploads({ ownerUserId: 'owner', files: [
      { originalName: 'source.pptx', bytes: await readFile(new URL('../../testdata/pptx/title-content/presentation.pptx', import.meta.url)) },
      { originalName: 'template.pptx', bytes: await readFile(new URL('../../testdata/pptx/template-branded/presentation.pptx', import.meta.url)) },
    ] });
    const [source, template] = thread.documents;
    const original = absoluteVersionPath(thread, source.versions[0]);
    for (const slide of [1, 2]) {
      const current = await readThread(thread.id, 'owner');
      await createTemplateFormSlideFromCurrent({ threadId: thread.id, templateDocumentId: template.id, sourceSlide: slide,
        expectedDocumentId: source.id, expectedVersionId: current.documents[0].currentVersionId });
      const after = await readThread(thread.id, 'owner');
      const file = absoluteVersionPath(after, after.documents[0].versions.at(-1));
      assert.equal(await slideText(file, slide), await slideText(original, slide));
      assert.equal(after.documents[1].versions.length, 1);
    }
  } finally {
    if (previous === undefined) delete process.env.OOXML_WEB_DATA_DIR;
    else process.env.OOXML_WEB_DATA_DIR = previous;
    await rm(dir, { recursive: true, force: true });
  }
});
