import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { workbenchHtml } from '../src/page.ts';
import { compactAgentStatus, compactSlideList } from '../src/shared/ooxml-tools.ts';

test('agent inventory omits preview URLs while preserving workflow, version guards and slide handles', () => {
  const version = { id: 'v1', downloadUrl: '/download', render: { thumbnails: Array.from({ length: 143 }, (_, i) => ({ url: '/preview/' + i })) } };
  const workflow = { sourceDocumentId: 'd', referenceDocumentIds: ['r'], language: 'French' };
  const summary = compactAgentStatus({ workflow, currentVersionId: 'v1', versions: [version], documents: [{ id: 'd', currentVersionId: 'v1', versions: [version] }] });
  assert.deepEqual(summary.workflow, workflow); assert.equal(summary.documents[0].versions[0].id, 'v1'); assert(!JSON.stringify(summary).includes('/preview/'));
  const compact = compactSlideList({ currentVersionId: 'v1', result: { slides: [{ number: 1, handle: 'H:pptx/s:1', textShapes: 3, readbackCommand: 'long command', selectors: ['1'], layout: 'Title' }] } }, 'pptx slides list');
  assert.equal(compact.result.slides[0].handle, 'H:pptx/s:1'); assert.equal(compact.result.slides[0].textShapes, 3); assert.equal(compact.currentVersionId, 'v1'); assert(!('readbackCommand' in compact.result.slides[0]));
});

test('preview waits never lock setup or overwrite a different job', async () => {
  const script = workbenchHtml().match(/<script>([\s\S]*?)<\/script>/)[1];
  const source = script.slice(script.indexOf('async function ensurePreview('), script.indexOf("$('previewDocument').onchange="));
  let release; const gate = new Promise(resolve => { release = resolve; });
  const state = { busy: false, thread: { id: 'first', documents: [{ id: 'source', versions: [{ id: 'v1' }] }] } };
  const context = vm.createContext({ state, previewLoads: new Set(), previewErrors: new Map(), renderPreview() {},
    previewSelection: () => ({ doc: { id: 'source', previewSupported: true }, version: { id: 'v1' }, key: 'source:v1' }),
    apiFetch: async () => { await gate; return {}; }, readApiJson: async () => ({ documents: [{ id: 'source', versions: [{ id: 'v1', render: { thumbnails: [1] } }] }] }),
    setBusy() { assert.fail('Preview must not lock the rest of the interface'); },
  });
  vm.runInContext(source, context); const pending = context.ensurePreview();
  assert.equal(state.busy, false); assert.equal(context.previewLoads.size, 1);
  state.thread = { id: 'second', documents: [] }; release(); await pending;
  assert.equal(state.thread.id, 'second'); assert.equal(state.thread.documents.length, 0); assert.equal(context.previewLoads.size, 0);
});

test('translation uses a four-language selector and consistent source labels', () => {
  const html = workbenchHtml();
  assert.match(html, /<label for="sourceSelect">Source deck<\/label>/);
  assert.match(html, /<label for="languageInput">Translate to<\/label><select/);
  const select = html.match(/<select id="languageInput">(.*?)<\/select>/)[1];
  assert.equal((select.match(/<option /g) || []).length, 4);
  for (const code of ['DE','FR','EN','IT']) assert(select.includes('('+code+')'));
});
