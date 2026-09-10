import assert from 'node:assert/strict';
import test from 'node:test';
import * as v from 'valibot';
import { createOoxmlTools } from '../src/shared/ooxml-tools.ts';

const tools = createOoxmlTools('schema-validation-only');
function parse(name, input) { return v.parse(tools.find(tool => tool.name === name).input, input); }

test('inspection accepts native flags and encoded flags without dropping values', () => {
  const args = { slide: 1, 'include-text': true };
  for (const argsJson of [args, JSON.stringify(args)]) {
    assert.deepEqual(parse('inspect_current_with_ooxml', { command: 'pptx slides show', argsJson }).argsJson, argsJson);
  }
  for (const argsJson of [null, [], 1, true]) assert.throws(() => parse('inspect_current_with_ooxml', { command: 'inspect', argsJson }));
});

test('mutations accept operation arrays and preserve stale-version guards', () => {
  const ops = [{ command: 'pptx replace text', args: { slide: 1, target: 'title', text: 'A "quoted" title' } }];
  for (const opsJson of [ops, JSON.stringify(ops)]) {
    const input = { opsJson, expectedDocumentId: 'doc', expectedVersionId: 'v0001' };
    assert.deepEqual(parse('apply_ooxml_ops_to_current', input), input);
  }
  for (const opsJson of [{ command: 'inspect' }, [1], null]) assert.throws(() => parse('apply_ooxml_ops_to_current', { opsJson }));
});

test('all builders accept native or encoded specification objects', () => {
  const spec = { title: 'Unicode café', slides: [{ title: 'A' }] };
  for (const name of ['build_presentation', 'build_workbook', 'build_document']) {
    for (const specJson of [spec, JSON.stringify(spec)]) assert.deepEqual(parse(name, { specJson }).specJson, specJson);
    assert.throws(() => parse(name, { specJson: [] }));
  }
});
