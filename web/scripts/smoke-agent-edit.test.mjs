import assert from 'node:assert/strict';
import test from 'node:test';
import vm from 'node:vm';
import { agentUpdateUrl, handleAgentEvent } from './smoke-agent-edit.mjs';
import { workbenchHtml } from '../src/page.ts';

const submissionId = 'current-submission';
const state = () => ({ submissionId, toolNames: new Set(), text: '', appendText(text) { this.text += text; } });

test('beta.9 live read selects updates and retains mounted paths and cursor', () => {
  const url = agentUpdateUrl({ streamUrl: '/office/flue/agents/editor/thread', offset: '0001_0002' }, 'http://localhost:3583');
  assert.equal(url.pathname, '/office/flue/agents/editor/thread');
  assert.equal(url.searchParams.get('view'), 'updates');
  assert.equal(url.searchParams.get('live'), 'sse');
  assert.equal(url.searchParams.get('offset'), '0001_0002');
});

test('beta.9 projected tools and text populate the smoke evidence', () => {
  const current = state();
  for (const name of ['get_ooxml_capabilities', 'inspect_current_with_ooxml', 'apply_ooxml_ops_to_current', 'check_package']) {
    assert.equal(handleAgentEvent({ type: 'tool-input', toolName: name }, current), false);
  }
  handleAgentEvent({ type: 'message-delta', kind: 'reasoning', delta: 'private reasoning' }, current);
  handleAgentEvent({ type: 'message-delta', kind: 'text', delta: 'Edited' }, current);
  assert.equal(current.toolNames.size, 4);
  assert.equal(current.text, 'Edited');
});

test('only the admitted submission can complete the stream', () => {
  const current = state();
  assert.equal(handleAgentEvent({ type: 'submission-settled', submissionId: 'other', outcome: 'completed' }, current), false);
  assert.equal(handleAgentEvent({ type: 'submission-settled', submissionId, outcome: 'completed' }, current), true);
  assert.throws(() => handleAgentEvent({ type: 'submission-settled', submissionId, outcome: 'failed', error: { message: 'provider rejected' } }, current), /provider rejected/);
  assert.throws(() => handleAgentEvent({ type: 'tool-output-error', errorText: 'mutation rejected' }, current), /mutation rejected/);
});

test('rendered browser stream consumes beta.9 text and matching settlement', async () => {
  const html = workbenchHtml();
  const script = html.match(/<script>([\s\S]*?)<\/script>/)[1];
  new vm.Script(script); // The shipped inline JavaScript must still parse.
  const start = script.indexOf('async function streamAgentEvents(admission)');
  const end = script.indexOf('function renderMarkdown(', start);
  const streamFunction = script.slice(start, end);
  const messages = [];
  let openedUrl;
  let closed = false;
  class Stream {
    constructor(url) { openedUrl = new URL(url); }
    addEventListener(name, listener) {
      if (name !== 'data') return;
      queueMicrotask(() => listener({ data: JSON.stringify([
        { type: 'message-delta', kind: 'reasoning', delta: 'hidden' },
        { type: 'tool-input', toolName: 'check_package' },
        { type: 'message-delta', kind: 'text', delta: 'Edited title' },
        { type: 'submission-settled', submissionId, outcome: 'completed' },
      ]) }));
    }
    close() { closed = true; }
  }
  const context = vm.createContext({
    URL, EventSource: Stream, Date, setInterval, clearInterval,
    state: {}, chat: { scrollTop: 0, scrollHeight: 1 },
    normalizedEventStreamUrl: value => new URL(value, 'http://localhost:3583'),
    renderMarkdown: value => value,
    addMessage: (kind, text) => { const message = { kind, textContent: text, innerHTML: '' }; messages.push(message); return message; },
  });
  vm.runInContext(streamFunction, context);
  await context.streamAgentEvents({ streamUrl: '/flue/agents/editor/thread', offset: '0', submissionId });
  assert.equal(openedUrl.searchParams.get('view'), 'updates');
  assert.equal(messages.find(message => message.kind === 'assistant').innerHTML, 'Edited title');
  assert(messages.some(message => message.textContent === 'tool started · check_package'));
  assert(closed);
});
