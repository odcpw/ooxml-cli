#!/usr/bin/env node
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as v from 'valibot';

const scriptDir = fileURLToPath(new URL('.', import.meta.url));
const fixture = resolve(scriptDir, '../../testdata/pptx/minimal-title/presentation.pptx');
const dataDir = await mkdtemp(join(tmpdir(), 'ooxml-flue-tools-'));
process.env.OOXML_WEB_DATA_DIR = dataDir;

try {
  const [{ createOoxmlTools }, { createThreadFromUpload }] = await Promise.all([
    import('../src/shared/ooxml-tools.ts'),
    import('../src/shared/storage.ts'),
  ]);
  const thread = await createThreadFromUpload({
    title: 'Flue tool qualification',
    ownerUserId: 'qualification-user',
    originalName: basename(fixture),
    bytes: await readFile(fixture),
  });
  const tools = createOoxmlTools(thread.id);
  const status = await invoke(tools, 'get_thread_status', {});
  const capabilities = await invoke(tools, 'get_ooxml_capabilities', { filter: 'check' });
  const args = { slide: 1, 'include-text': true };
  const objectInspection = await invoke(tools, 'inspect_current_with_ooxml', { command: 'pptx slides show', argsJson: args });
  const stringInspection = await invoke(tools, 'inspect_current_with_ooxml', { command: 'pptx slides show', argsJson: JSON.stringify(args) });
  assert(comparableInspection(objectInspection) === comparableInspection(stringInspection), 'Object and encoded args produced different inspection results');
  const marker = 'Flue structured input qualification';
  await invoke(tools, 'apply_ooxml_ops_to_current', {
    opsJson: [{ command: 'pptx replace text', args: { slide: 1, target: 'title', text: marker } }],
    expectedDocumentId: status.currentDocumentId,
    expectedVersionId: status.currentVersionId,
  });
  const edited = await invoke(tools, 'inspect_current_with_ooxml', { command: 'pptx slides show', argsJson: args });
  assert(JSON.stringify(edited).includes(marker), 'Structured operations did not publish the requested title');
  const proof = await invoke(tools, 'check_package', {
    openXmlSdk: 'skip',
    failOn: 'error',
    render: false,
  });

  assert(status.currentDocumentId, 'get_thread_status did not return a structured document id');
  assert(capabilities.contractVersion, 'get_ooxml_capabilities did not return structured capabilities');
  assert(proof.proofLevel?.strict === 'passed', 'check_package did not pass strict proof');
  assert(Number(proof.summary?.errors ?? 0) === 0, 'check_package reported errors');
  console.log(
    JSON.stringify(
      {
        ok: true,
        toolCount: tools.length,
        exercised: ['get_thread_status', 'get_ooxml_capabilities', 'inspect_current_with_ooxml', 'apply_ooxml_ops_to_current', 'check_package'],
        proofLevel: proof.proofLevel,
      },
      null,
      2,
    ),
  );
} finally {
  await rm(dataDir, { recursive: true, force: true });
}

async function invoke(tools, name, input) {
  const tool = tools.find((candidate) => candidate.name === name);
  assert(tool, `missing Flue tool ${name}`);
  const result = await tool.run({ data: v.parse(tool.input, input) });
  assert(result && Object.hasOwn(result, 'output'), `missing result envelope for ${name}`);
  return result.output;
}

function comparableInspection(inspection) {
  const workingFile = inspection.result?.file;
  assert(typeof workingFile === 'string' && workingFile, 'Inspection did not report its working file');
  // Each serve process uses a fresh scratch path. Keep all content, selectors,
  // geometry, and command flags; normalize only that path in file/command fields.
  return JSON.stringify(inspection, (key, value) =>
    typeof value === 'string' && (key === 'file' || key.endsWith('Command'))
      ? value.replaceAll(workingFile, '<working-file>')
      : value,
  );
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
