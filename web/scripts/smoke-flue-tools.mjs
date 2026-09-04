#!/usr/bin/env node
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = fileURLToPath(new URL('.', import.meta.url));
const fixture = resolve(scriptDir, '../../testdata/xlsx/minimal-workbook/workbook.xlsx');
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
        exercised: ['get_thread_status', 'get_ooxml_capabilities', 'check_package'],
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
  return tool.run({ input });
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
