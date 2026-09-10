import assert from 'node:assert/strict';
import test from 'node:test';
import vm from 'node:vm';
import { agentUpdateUrl, handleAgentEvent } from './smoke-agent-edit.mjs';
import { workbenchHtml } from '../src/page.ts';

const submissionId = 'current-submission';
const state = () => ({ submissionId, toolNames: new Set(), text: '', appendText(text) { this.text += text; } });

test('Flue 2 live read selects updates and retains mounted paths and cursor', () => {
  const url = agentUpdateUrl({ streamUrl: '/office/flue/agents/editor/thread', offset: '0001_0002' }, 'http://localhost:3583/office');
  assert.equal(url.pathname, '/office/flue/agents/editor/thread');
  assert.equal(url.searchParams.get('view'), 'updates');
  assert.equal(url.searchParams.get('live'), 'sse');
  assert.equal(url.searchParams.get('offset'), '0001_0002');
});

test('Flue 2 projected tools and text populate the smoke evidence', () => {
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

test('rendered browser stream consumes Flue 2 text and matching settlement', async () => {
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
    URL, EventSource: Stream, Date, setInterval, clearInterval, setTimeout, clearTimeout, AbortController, AbortSignal,
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

for (const ending of ['closed', 'failed']) {
  test('browser rejects ' + ending + ' before successful settlement', async () => {
    const script = workbenchHtml().match(/<script>([\s\S]*?)<\/script>/)[1];
    const start = script.indexOf('async function streamAgentEvents(admission)');
    const end = script.indexOf('function renderMarkdown(', start);
    class Stream {
      addEventListener(name, listener) {
        if (name === 'data') queueMicrotask(() => {
          listener({ data: JSON.stringify([{ type: 'message-delta', kind: 'text', delta: 'Working' }]) });
          if (ending === 'disconnect') this.onerror();
          if (ending === 'failed') listener({ data: JSON.stringify([{ type: 'submission-settled', submissionId, outcome: 'failed', error: { message: 'provider failed' } }]) });
        });
        if (name === 'control' && ending === 'closed') queueMicrotask(() => listener({ data: JSON.stringify({ streamClosed: true }) }));
      }
      close() {}
    }
    const context = vm.createContext({
      URL, EventSource: Stream, Date, setInterval, clearInterval, setTimeout, clearTimeout, AbortController, AbortSignal,
      state: {}, chat: { scrollTop: 0, scrollHeight: 1 },
      normalizedEventStreamUrl: value => new URL(value, 'http://localhost:3583'),
      renderMarkdown: value => value, readableError: value => value?.message || String(value),
      addMessage: () => ({ textContent: '', innerHTML: '' }),
    });
    vm.runInContext(script.slice(start, end), context);
    await assert.rejects(context.streamAgentEvents({ streamUrl: '/flue/agents/editor/thread', offset: '0', submissionId }), /before completion|submission failed/);
  });
}

test('browser recovers a blocked stream through saved updates without duplicate text or resubmitting work', async () => {
  const script = workbenchHtml().match(/<script>([\s\S]*?)<\/script>/)[1];
  const start = script.indexOf('async function streamAgentEvents(admission)');
  const end = script.indexOf('function renderMarkdown(', start);
  const first = { type: 'message-delta', messageId: 'm', kind: 'text', delta: 'Translated ', position: { batch: 1, index: 0 } };
  const messages = []; let polls = 0;
  class Stream {
    addEventListener(name, listener) { if (name === 'data') queueMicrotask(() => { listener({ data: JSON.stringify([first]) }); this.onerror(); }); }
    close() {}
  }
  const context = vm.createContext({
    URL, EventSource: Stream, Date, setInterval, clearInterval, setTimeout, clearTimeout, AbortController, AbortSignal,
    state: {}, chat: { scrollTop: 0, scrollHeight: 1 },
    normalizedEventStreamUrl: value => new URL(value, 'https://example.test'), renderMarkdown: value => value,
    addMessage: (kind, text) => { const node = { kind, textContent: text, innerHTML: '' }; messages.push(node); return node; },
    apiFetch: async url => { polls++; assert(!url.includes('live=')); return new Response(JSON.stringify([first, { ...first, delta: 'slides.', position: { batch: 1, index: 1 } }, { type: 'submission-settled', submissionId, outcome: 'completed' }]), { headers: { 'stream-next-offset': '0_3', 'stream-up-to-date': 'true' } }); },
  });
  vm.runInContext(script.slice(start, end), context);
  await context.streamAgentEvents({ streamUrl: '/flue/agents/editor/thread', offset: '0_0', submissionId });
  assert.equal(polls, 1); assert.equal(messages.find(m => m.kind === 'assistant').innerHTML, 'Translated slides.');
});

for (const streamUrl of ['/flue/agents/editor/thread', '/ooxml/flue/agents/editor/thread', 'http://127.0.0.1:3594/flue/agents/editor/thread']) {
  test('public mounted stream normalizes ' + streamUrl, () => {
    const url = agentUpdateUrl({ streamUrl, offset: 'cursor' }, 'https://ss.odc.pw/ooxml');
    assert.equal(url.origin, 'https://ss.odc.pw');
    assert.equal(url.pathname, '/ooxml/flue/agents/editor/thread');
    assert.equal(url.searchParams.get('offset'), 'cursor');
    assert.equal(url.searchParams.get('live'), 'sse');
  });
}
