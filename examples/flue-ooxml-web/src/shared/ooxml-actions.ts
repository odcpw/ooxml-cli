import { execFile, spawn } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { basename, extname, join } from 'node:path';
import { promisify } from 'node:util';
import {
  absoluteVersionPath,
  artifactUrl,
  currentDocument,
  currentVersion,
  documentById,
  fileUrlFor,
  nextVersionId,
  readThread,
  relativeToThread,
  safeJoin,
  threadDir,
  type FileVersion,
  type RenderInfo,
  type ThreadDocument,
  type ThreadRecord,
  writeThread,
} from './storage.ts';

const execFileAsync = promisify(execFile);
const maxOutputBuffer = 24 * 1024 * 1024;

export async function runOoxml(args: string[], cwd: string): Promise<{ stdout: string; stderr: string }> {
  const bin = process.env.OOXML_BIN || 'ooxml';
  try {
    const result = await execFileAsync(bin, args, {
      cwd,
      maxBuffer: maxOutputBuffer,
      timeout: 120_000,
    });
    return {
      stdout: result.stdout.toString(),
      stderr: result.stderr.toString(),
    };
  } catch (error) {
    const err = error as Error & { stdout?: Buffer | string; stderr?: Buffer | string; code?: number };
    const stdout = typeof err.stdout === 'string' ? err.stdout : err.stdout?.toString() ?? '';
    const stderr = typeof err.stderr === 'string' ? err.stderr : err.stderr?.toString() ?? '';
    throw new Error(
      [
        `ooxml failed${typeof err.code === 'number' ? ` with exit ${err.code}` : ''}: ${args.join(' ')}`,
        stdout.trim() ? `stdout:\n${stdout.trim()}` : '',
        stderr.trim() ? `stderr:\n${stderr.trim()}` : '',
        err.message,
      ]
        .filter(Boolean)
        .join('\n\n'),
    );
  }
}

type OoxmlCapabilityCommand = {
  path?: string;
  use?: string;
  short?: string;
  subcommands?: unknown;
  opCompatible?: boolean;
  opIneligibleReason?: string;
  targetObjectKinds?: unknown;
  objectKinds?: unknown;
  mutates?: unknown;
  readOnly?: unknown;
};

type OoxmlCapabilities = {
  tool?: string;
  version?: string;
  contractVersion?: string;
  packageTypes?: unknown;
  commands?: OoxmlCapabilityCommand[];
  objectKinds?: unknown;
  objectKindsIndex?: unknown;
  objectKindIndex?: unknown;
  workflows?: unknown;
  notes?: unknown;
  conventions?: unknown;
};

export async function getOoxmlCapabilities(filter?: string, includeDetails = false): Promise<string> {
  const args = ['--json', 'capabilities'];
  const normalizedFilter = filter?.trim();
  if (normalizedFilter) args.push('--for', normalizedFilter);
  const result = await runOoxml(args, process.cwd());
  const parsed = JSON.parse(result.stdout) as OoxmlCapabilities;
  if (includeDetails) return JSON.stringify(parsed, null, 2);
  const commandIndex = normalizedFilter ? (parsed.commands ?? []).map(compactCapabilityCommand) : undefined;
  return JSON.stringify(
    {
      tool: parsed.tool,
      version: parsed.version,
      contractVersion: parsed.contractVersion,
      filter: normalizedFilter || null,
      packageTypes: parsed.packageTypes,
      commandCount: parsed.commands?.length ?? 0,
      objectKinds: parsed.objectKinds,
      objectKindsIndex: parsed.objectKindsIndex ?? parsed.objectKindIndex,
      workflows: parsed.workflows,
      notes: parsed.notes,
      conventions: parsed.conventions,
      commands: commandIndex,
      next: normalizedFilter
        ? 'Use a command path without the leading "ooxml" in inspect_current_with_ooxml or apply_ooxml_ops_to_current. Call get_ooxml_command_help for exact flags. Set includeDetails=true only when this compact index is insufficient.'
        : 'Call get_ooxml_capabilities with a filter such as pptx, xlsx, docx, vba, shape, slide, chart, table, range, style, or package. Then call get_ooxml_command_help for exact flags.',
    },
    null,
    2,
  );
}

function compactCapabilityCommand(command: OoxmlCapabilityCommand): Record<string, unknown> {
  const compact: Record<string, unknown> = {
    path: command.path,
    use: command.use,
    short: command.short,
    opCompatible: command.opCompatible,
  };
  if (command.opIneligibleReason) compact.opIneligibleReason = command.opIneligibleReason;
  if (command.targetObjectKinds) compact.targetObjectKinds = command.targetObjectKinds;
  if (command.objectKinds) compact.objectKinds = command.objectKinds;
  if (command.subcommands) compact.subcommands = command.subcommands;
  if (command.mutates !== undefined) compact.mutates = command.mutates;
  if (command.readOnly !== undefined) compact.readOnly = command.readOnly;
  return compact;
}

export async function getOoxmlCommandHelp(command?: string): Promise<string> {
  const commandWords = normalizeServeCommand(command ?? '');
  const args = commandWords ? [...commandWords.split(' '), '--help'] : ['--help'];
  const result = await runOoxml(args, process.cwd());
  return result.stdout;
}

export function publicThreadSummary(thread: ThreadRecord): Record<string, unknown> {
  const currentDoc = currentDocument(thread);
  const current = currentVersion(thread, currentDoc);
  return {
    id: thread.id,
    title: thread.title,
    createdAt: thread.createdAt,
    updatedAt: thread.updatedAt,
    currentDocumentId: currentDoc.id,
    currentDocumentName: currentDoc.title,
    currentVersionId: currentDoc.currentVersionId,
    currentFile: current.originalName,
    currentExtension: extname(current.path).toLowerCase(),
    previewSupported: previewSupportedFor(current),
    downloadUrl: fileUrlFor(thread.id, currentDoc.id, current.id),
    documents: thread.documents.map((document) => publicDocumentSummary(thread, document)),
    versions: currentDoc.versions.map((version) => publicVersionSummary(thread, currentDoc, version)),
  };
}

export async function inspectCurrent(threadId: string): Promise<string> {
  const { thread, version } = await currentSelection(threadId);
  const file = absoluteVersionPath(thread, version);
  const result = await runOoxml(['--json', 'inspect', file], threadDir(threadId));
  return result.stdout;
}

export async function validateCurrent(threadId: string): Promise<string> {
  const { thread, version } = await currentSelection(threadId);
  const file = absoluteVersionPath(thread, version);
  const result = await runOoxml(['--json', '--strict', 'validate', file], threadDir(threadId));
  return result.stdout;
}

export async function searchCurrent(input: {
  threadId: string;
  query: string;
  ignoreCase?: boolean;
}): Promise<string> {
  const { thread, version } = await currentSelection(input.threadId);
  const file = absoluteVersionPath(thread, version);
  const args = ['--json', 'find', input.query, file, '--max', '20'];
  if (input.ignoreCase) args.push('--ignore-case');
  const result = await runOoxml(args, threadDir(input.threadId));
  return result.stdout;
}

export async function showSlideCurrent(input: {
  threadId: string;
  slide: number;
  includeBounds?: boolean;
}): Promise<string> {
  const { thread, version } = await currentSelection(input.threadId);
  assertPresentation(version);
  const file = absoluteVersionPath(thread, version);
  const args = ['--json', 'pptx', 'slides', 'show', file, '--slide', String(input.slide), '--include-text'];
  if (input.includeBounds ?? true) args.push('--include-bounds');
  const result = await runOoxml(args, threadDir(input.threadId));
  return result.stdout;
}

export async function replaceTextCurrent(input: {
  threadId: string;
  query: string;
  replacement: string;
  ignoreCase?: boolean;
}): Promise<Record<string, unknown>> {
  const { thread, document, version } = await currentSelection(input.threadId);
  const file = absoluteVersionPath(thread, version);
  const dir = threadDir(input.threadId);
  const newVersionId = nextVersionId(document);
  const ext = extname(version.path);
  const outPath = join(dir, 'documents', document.id, 'versions', `${newVersionId}-replace${ext}`);
  const opsPath = join(dir, 'documents', document.id, 'tmp', `${newVersionId}-ops.json`);
  await mkdir(join(dir, 'documents', document.id, 'versions'), { recursive: true });
  await mkdir(join(dir, 'documents', document.id, 'tmp'), { recursive: true });

  const findArgs = ['--json', 'find', input.query, file, '--replace', input.replacement, '--to-ops'];
  if (input.ignoreCase) findArgs.push('--ignore-case');
  const ops = await runOoxml(findArgs, dir);
  const parsedOps = JSON.parse(ops.stdout) as unknown[];
  if (!Array.isArray(parsedOps) || parsedOps.length === 0) {
    return {
      changed: false,
      reason: 'No matching text was found.',
      currentDocumentId: document.id,
      currentVersionId: document.currentVersionId,
    };
  }

  await writeFile(opsPath, `${JSON.stringify(parsedOps, null, 2)}\n`);
  const applied = await runOoxml(['--json', 'apply', file, '--ops', opsPath, '--out', outPath], dir);
  return publishNewVersion({
    thread,
    document,
    sourceVersion: version,
    versionId: newVersionId,
    outPath,
    note: `Replaced "${input.query}" with "${input.replacement}"`,
    apply: JSON.parse(applied.stdout),
    extra: { operationCount: parsedOps.length },
  });
}

export async function setSlideShapeTextCurrent(input: {
  threadId: string;
  slide: number;
  target: string;
  text: string;
}): Promise<Record<string, unknown>> {
  const { thread, document, version } = await currentSelection(input.threadId);
  assertPresentation(version);
  const file = absoluteVersionPath(thread, version);
  const dir = threadDir(input.threadId);
  const newVersionId = nextVersionId(document);
  const ext = extname(version.path);
  const outPath = join(dir, 'documents', document.id, 'versions', `${newVersionId}-slide-text${ext}`);
  await mkdir(join(dir, 'documents', document.id, 'versions'), { recursive: true });
  const applied = await runOoxml(
    [
      '--json',
      'pptx',
      'replace',
      'text',
      file,
      '--slide',
      String(input.slide),
      '--target',
      input.target,
      '--text',
      input.text,
      '--out',
      outPath,
    ],
    dir,
  );
  return publishNewVersion({
    thread,
    document,
    sourceVersion: version,
    versionId: newVersionId,
    outPath,
    note: `Set slide ${input.slide} ${input.target} text`,
    apply: JSON.parse(applied.stdout),
    extra: { slide: input.slide, target: input.target },
  });
}

export async function applyTemplateToCurrentDocument(input: {
  threadId: string;
  templateDocumentId: string;
  targetCharts?: boolean;
}): Promise<Record<string, unknown>> {
  const { thread, document, version } = await currentSelection(input.threadId);
  const templateDocument = documentById(thread, input.templateDocumentId);
  if (templateDocument.id === document.id) {
    throw new Error('Choose a different document as the template source.');
  }
  const templateVersion = currentVersion(thread, templateDocument);
  const file = absoluteVersionPath(thread, version);
  const templateFile = absoluteVersionPath(thread, templateVersion);
  const dir = threadDir(input.threadId);
  const newVersionId = nextVersionId(document);
  const ext = extname(version.path);
  const outPath = join(dir, 'documents', document.id, 'versions', `${newVersionId}-template${ext}`);
  await mkdir(join(dir, 'documents', document.id, 'versions'), { recursive: true });

  const args = [
    '--json',
    'template',
    'apply',
    file,
    '--from',
    templateFile,
    '--target-colors',
    '--target-fonts',
    '--out',
    outPath,
  ];
  if (input.targetCharts) args.push('--target-charts');
  const applied = await runOoxml(args, dir);

  return publishNewVersion({
    thread,
    document,
    sourceVersion: version,
    versionId: newVersionId,
    outPath,
    note: `Applied template colors and fonts from ${templateDocument.title}`,
    apply: JSON.parse(applied.stdout),
    extra: {
      templateDocumentId: templateDocument.id,
      templateVersionId: templateVersion.id,
      targetCharts: Boolean(input.targetCharts),
      limitation:
        'Applies transferable design tokens: theme colors and major/minor fonts, plus chart styling when requested. It does not rebuild slide layouts or copy arbitrary shape geometry.',
    },
  });
}

export async function inspectCurrentWithOoxml(input: {
  threadId: string;
  command: string;
  argsJson?: string;
}): Promise<string> {
  const { thread, document, version } = await currentSelection(input.threadId);
  const file = absoluteVersionPath(thread, version);
  const requests = [
    serveRequest(1, 'open', { file, dryRun: true }),
    serveRequest(2, 'inspect', {
      session: 's1',
      command: normalizeServeCommand(input.command),
      args: parseArgsJson(input.argsJson),
    }),
    serveRequest(3, 'abort', { session: 's1' }),
  ];
  const responses = await runOoxmlServe(requests, threadDir(input.threadId));
  return JSON.stringify(
    {
      currentDocumentId: document.id,
      currentVersionId: version.id,
      result: resultOrThrow(responses[1], 'inspect'),
      next: 'Pass expectedDocumentId and expectedVersionId to apply_ooxml_ops_to_current so the edit fails instead of drifting if the selected document changes mid-turn.',
    },
    null,
    2,
  );
}

export async function applyOoxmlOpsToCurrent(input: {
  threadId: string;
  opsJson: string;
  note?: string;
  expectedDocumentId?: string;
  expectedVersionId?: string;
}): Promise<Record<string, unknown>> {
  const { thread, document, version } = await currentSelection(input.threadId, {
    documentId: input.expectedDocumentId,
    versionId: input.expectedVersionId,
  });
  if (thread.documents.length > 1 && (!input.expectedDocumentId || !input.expectedVersionId)) {
    throw new Error(
      'Multi-file threads require expectedDocumentId and expectedVersionId for generic mutations. Call get_thread_status or inspect_current_with_ooxml, then retry with those guards.',
    );
  }
  const operations = parseOperationsJson(input.opsJson);
  const dir = threadDir(input.threadId);
  const file = absoluteVersionPath(thread, version);
  const newVersionId = nextVersionId(document);
  const ext = extname(version.path);
  const outPath = join(dir, 'documents', document.id, 'versions', `${newVersionId}-ooxml${ext}`);
  await mkdir(join(dir, 'documents', document.id, 'versions'), { recursive: true });

  const requests: ServeRequest[] = [serveRequest(1, 'open', { file, out: outPath })];
  operations.forEach((operation, index) => {
    requests.push(
      serveRequest(index + 2, 'op', {
        session: 's1',
        command: normalizeServeCommand(operation.command),
        args: operation.args ?? {},
      }),
    );
  });
  requests.push(serveRequest(operations.length + 2, 'validate', { session: 's1' }));
  requests.push(serveRequest(operations.length + 3, 'commit', { session: 's1' }));

  let responses: ServeResponse[];
  try {
    responses = await runOoxmlServe(requests, dir);
  } catch (error) {
    throw error;
  }

  const opResults = responses.slice(1, 1 + operations.length).map((response) => resultOrThrow(response, 'op'));
  const validate = resultOrThrow(responses[1 + operations.length], 'validate');
  const commit = resultOrThrow(responses[2 + operations.length], 'commit');

  return publishNewVersion({
    thread,
    document,
    sourceVersion: version,
    versionId: newVersionId,
    outPath,
    note: input.note?.trim() || `Applied ${operations.length} OOXML operation${operations.length === 1 ? '' : 's'}`,
    apply: commit,
    extra: {
      operations,
      opResults,
      serveValidate: validate,
    },
  });
}

export async function renderCurrent(threadId: string): Promise<Record<string, unknown>> {
  const { thread, document, version } = await currentSelection(threadId);
  if (!previewSupportedFor(version)) {
    return {
      rendered: false,
      reason: 'Preview rendering is currently wired for PPTX/PPTM only.',
      currentDocumentId: document.id,
      currentVersionId: document.currentVersionId,
      currentExtension: extname(version.path).toLowerCase(),
    };
  }

  const dir = threadDir(threadId);
  const file = absoluteVersionPath(thread, version);
  const renderDir = join(dir, 'documents', document.id, 'renders', version.id);
  await mkdir(renderDir, { recursive: true });

  const rendered = await runOoxml(['--json', 'pptx', 'render', file, '--out', renderDir, '--thumbnails'], dir);
  const parsed = JSON.parse(rendered.stdout) as {
    pdfPath?: string;
    thumbnails?: Array<{ index?: number; slide?: number; path?: string; imagePath?: string; width?: number; height?: number }>;
  };
  const thumbnails = (parsed.thumbnails ?? []).map((thumb, index) => {
    const rawPath = thumb.path ?? thumb.imagePath;
    if (!rawPath) throw new Error('Render manifest did not include a thumbnail path');
    return {
      index: thumb.index ?? thumb.slide ?? index + 1,
      path: relativeToThread(threadId, rawPath),
      width: thumb.width,
      height: thumb.height,
    };
  });

  const renderInfo: RenderInfo = {
    dir: relativeToThread(threadId, renderDir),
    pdfPath: parsed.pdfPath ? relativeToThread(threadId, parsed.pdfPath) : undefined,
    manifestPath: relativeToThread(threadId, join(renderDir, 'thumbnails-manifest.json')),
    thumbnails,
  };
  version.render = renderInfo;
  await writeThread(thread);

  return {
    rendered: true,
    currentDocumentId: document.id,
    currentVersionId: version.id,
    thumbnails: thumbnails.map((thumb) => ({
      ...thumb,
      url: artifactUrl(threadId, document.id, version.id, thumb.path),
    })),
  };
}

export async function readVersionRenderArtifact(input: {
  thread: ThreadRecord;
  document: ThreadDocument;
  version: FileVersion;
  path: string;
}): Promise<{
  bytes: Buffer;
  filename: string;
}> {
  const decoded = decodeArtifactPath(input.path);
  const allowed = allowedRenderArtifactPaths(input.version);
  if (!allowed.has(decoded)) {
    throw new Error('Artifact is not registered as a render output for this version.');
  }
  const file = safeJoin(threadDir(input.thread.id), decoded);
  const bytes = await readFile(file);
  return { bytes, filename: basename(file) };
}

function currentSelection(threadId: string): Promise<{
  thread: ThreadRecord;
  document: ThreadDocument;
  version: FileVersion;
}>;
function currentSelection(
  threadId: string,
  expected?: {
    documentId?: string;
    versionId?: string;
  },
): Promise<{
  thread: ThreadRecord;
  document: ThreadDocument;
  version: FileVersion;
}>;
async function currentSelection(
  threadId: string,
  expected?: {
    documentId?: string;
    versionId?: string;
  },
): Promise<{
  thread: ThreadRecord;
  document: ThreadDocument;
  version: FileVersion;
}> {
  const thread = await readThread(threadId);
  const document = currentDocument(thread);
  const version = currentVersion(thread, document);
  assertExpectedSelection({ document, version, expected });
  return { thread, document, version };
}

function publicDocumentSummary(thread: ThreadRecord, document: ThreadDocument): Record<string, unknown> {
  const version = currentVersion(thread, document);
  return {
    id: document.id,
    title: document.title,
    originalName: document.originalName,
    createdAt: document.createdAt,
    currentVersionId: document.currentVersionId,
    currentFile: version.originalName,
    currentExtension: extname(version.path).toLowerCase(),
    previewSupported: previewSupportedFor(version),
    downloadUrl: fileUrlFor(thread.id, document.id, version.id),
    versions: document.versions.map((candidate) => publicVersionSummary(thread, document, candidate)),
  };
}

function publicVersionSummary(thread: ThreadRecord, document: ThreadDocument, version: FileVersion): Record<string, unknown> {
  return {
    id: version.id,
    originalName: version.originalName,
    createdAt: version.createdAt,
    note: version.note,
    extension: extname(version.path).toLowerCase(),
    previewSupported: previewSupportedFor(version),
    downloadUrl: fileUrlFor(thread.id, document.id, version.id),
    render: version.render
      ? {
          pdfUrl: version.render.pdfPath ? artifactUrl(thread.id, document.id, version.id, version.render.pdfPath) : undefined,
          thumbnails: version.render.thumbnails.map((thumb) => ({
            ...thumb,
            url: artifactUrl(thread.id, document.id, version.id, thumb.path),
          })),
        }
      : undefined,
  };
}

async function publishNewVersion(input: {
  thread: ThreadRecord;
  document: ThreadDocument;
  sourceVersion: FileVersion;
  versionId: string;
  outPath: string;
  note: string;
  apply: unknown;
  extra?: Record<string, unknown>;
}): Promise<Record<string, unknown>> {
  const validate = await runOoxml(['--json', '--strict', 'validate', input.outPath], threadDir(input.thread.id));
  const latestThread = await readThread(input.thread.id);
  const latestDocument = documentById(latestThread, input.document.id);
  const latestVersion = currentVersion(latestThread, latestDocument);
  if (latestThread.currentDocumentId !== input.document.id || latestVersion.id !== input.sourceVersion.id) {
    throw new Error(
      [
        'Thread changed while the agent was working, so the edit was not published.',
        `Expected current document/version ${input.document.id}/${input.sourceVersion.id}.`,
        `Actual current document/version ${latestThread.currentDocumentId}/${latestVersion.id}.`,
        'Select the intended document and retry the edit.',
      ].join(' '),
    );
  }
  const ext = extname(input.sourceVersion.path);
  const now = new Date().toISOString();
  const newVersion = {
    id: input.versionId,
    originalName: `${basename(input.document.originalName, ext)}-${input.versionId}${ext}`,
    path: relativeToThread(input.thread.id, input.outPath),
    createdAt: now,
    note: input.note,
  };
  latestDocument.versions.push(newVersion);
  latestDocument.currentVersionId = input.versionId;
  latestThread.currentDocumentId = latestDocument.id;
  await writeThread(latestThread);

  return {
    changed: true,
    documentId: latestDocument.id,
    version: newVersion,
    apply: input.apply,
    validate: JSON.parse(validate.stdout),
    downloadUrl: fileUrlFor(latestThread.id, latestDocument.id, input.versionId),
    ...input.extra,
  };
}

function assertExpectedSelection(input: {
  document: ThreadDocument;
  version: FileVersion;
  expected?: {
    documentId?: string;
    versionId?: string;
  };
}): void {
  const expectedDocumentId = input.expected?.documentId?.trim();
  const expectedVersionId = input.expected?.versionId?.trim();
  if (expectedDocumentId && expectedDocumentId !== input.document.id) {
    throw new Error(`Selected document changed before the tool ran. Expected ${expectedDocumentId}, got ${input.document.id}.`);
  }
  if (expectedVersionId && expectedVersionId !== input.version.id) {
    throw new Error(`Selected version changed before the tool ran. Expected ${expectedVersionId}, got ${input.version.id}.`);
  }
}

function assertPresentation(version: FileVersion): void {
  const ext = extname(version.path).toLowerCase();
  if (ext !== '.pptx' && ext !== '.pptm') {
    throw new Error('This tool is available for PPTX/PPTM files only.');
  }
}

type ServeRequest = {
  jsonrpc: '2.0';
  id: number;
  method: string;
  params?: Record<string, unknown>;
};

type ServeResponse = {
  jsonrpc: '2.0';
  id: number;
  result?: unknown;
  error?: {
    code: number;
    message: string;
    data?: unknown;
  };
};

type OoxmlOperation = {
  command: string;
  args?: Record<string, unknown>;
};

function serveRequest(id: number, method: string, params?: Record<string, unknown>): ServeRequest {
  return { jsonrpc: '2.0', id, method, params };
}

async function runOoxmlServe(requests: ServeRequest[], cwd: string): Promise<ServeResponse[]> {
  const bin = process.env.OOXML_BIN || 'ooxml';
  const child = spawn(bin, ['serve'], {
    cwd,
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  const stdout: Buffer[] = [];
  const stderr: Buffer[] = [];
  let settled = false;

  const timeout = setTimeout(() => {
    if (!settled) child.kill('SIGKILL');
  }, 120_000);

  child.stdout.on('data', (chunk: Buffer) => stdout.push(chunk));
  child.stderr.on('data', (chunk: Buffer) => stderr.push(chunk));
  for (const request of requests) {
    child.stdin.write(`${JSON.stringify(request)}\n`);
  }
  child.stdin.end();

  let code: number | null;
  try {
    code = await new Promise<number | null>((resolve, reject) => {
      child.on('error', reject);
      child.on('close', resolve);
    });
  } finally {
    settled = true;
    clearTimeout(timeout);
  }

  const stdoutText = Buffer.concat(stdout).toString();
  const stderrText = Buffer.concat(stderr).toString();
  const parsed = parseServeResponses(stdoutText);

  if (code !== 0) {
    throw new Error(
      [
        `ooxml serve failed${typeof code === 'number' ? ` with exit ${code}` : ''}`,
        stderrText.trim() ? `stderr:\n${stderrText.trim()}` : '',
        stdoutText.trim() ? `stdout:\n${stdoutText.trim()}` : '',
      ]
        .filter(Boolean)
        .join('\n\n'),
    );
  }
  return requests.map((request) => {
    const response = parsed.byId.get(request.id);
    if (!response) {
      throw new Error(
        [
          `Missing ooxml serve response for request ${request.id} (${request.method})`,
          parsed.ignoredLines.length ? `ignored stdout lines:\n${parsed.ignoredLines.join('\n')}` : '',
          stdoutText.trim() ? `stdout:\n${stdoutText.trim()}` : '',
          stderrText.trim() ? `stderr:\n${stderrText.trim()}` : '',
        ]
          .filter(Boolean)
          .join('\n\n'),
      );
    }
    return response;
  });
}

function parseServeResponses(stdoutText: string): {
  byId: Map<number, ServeResponse>;
  ignoredLines: string[];
} {
  const byId = new Map<number, ServeResponse>();
  const ignoredLines: string[] = [];
  for (const line of stdoutText.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    let parsed: unknown;
    try {
      parsed = JSON.parse(trimmed);
    } catch {
      ignoredLines.push(trimmed);
      continue;
    }
    if (!isServeResponse(parsed)) {
      ignoredLines.push(trimmed);
      continue;
    }
    byId.set(parsed.id, parsed);
  }
  return { byId, ignoredLines };
}

function isServeResponse(value: unknown): value is ServeResponse {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const candidate = value as Partial<ServeResponse>;
  return candidate.jsonrpc === '2.0' && typeof candidate.id === 'number';
}

function resultOrThrow(response: ServeResponse | undefined, label: string): unknown {
  if (!response) throw new Error(`Missing ooxml serve response for ${label}`);
  if (response.error) {
    throw new Error(
      JSON.stringify(
        {
          error: response.error.message,
          code: response.error.code,
          data: response.error.data,
        },
        null,
        2,
      ),
    );
  }
  return response.result;
}

function parseArgsJson(raw: string | undefined): Record<string, unknown> {
  if (!raw?.trim()) return {};
  const parsed = JSON.parse(raw) as unknown;
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new Error('argsJson must be a JSON object.');
  }
  return parsed as Record<string, unknown>;
}

function parseOperationsJson(raw: string): OoxmlOperation[] {
  const parsed = JSON.parse(raw) as unknown;
  if (!Array.isArray(parsed) || parsed.length === 0) {
    throw new Error('opsJson must be a non-empty JSON array.');
  }
  return parsed.map((item, index) => {
    if (!item || typeof item !== 'object' || Array.isArray(item)) {
      throw new Error(`Operation ${index + 1} must be a JSON object.`);
    }
    const operation = item as Partial<OoxmlOperation>;
    if (typeof operation.command !== 'string' || !operation.command.trim()) {
      throw new Error(`Operation ${index + 1} must include a command string.`);
    }
    if (operation.args !== undefined && (!operation.args || typeof operation.args !== 'object' || Array.isArray(operation.args))) {
      throw new Error(`Operation ${index + 1} args must be a JSON object.`);
    }
    return {
      command: normalizeServeCommand(operation.command),
      args: operation.args ?? {},
    };
  });
}

function normalizeServeCommand(command: string): string {
  const normalized = command.trim().replace(/\s+/g, ' ').replace(/^ooxml\s+/, '');
  if (!normalized) return '';
  for (const word of normalized.split(' ')) {
    if (word.startsWith('-')) {
      throw new Error('Command must contain only command words. Put flags in argsJson or opsJson args.');
    }
  }
  return normalized;
}

function previewSupportedFor(version: FileVersion): boolean {
  const ext = extname(version.path).toLowerCase();
  return ext === '.pptx' || ext === '.pptm';
}

function allowedRenderArtifactPaths(version: FileVersion): Set<string> {
  const allowed = new Set<string>();
  if (!version.render) return allowed;
  if (version.render.pdfPath) allowed.add(version.render.pdfPath);
  if (version.render.manifestPath) allowed.add(version.render.manifestPath);
  for (const thumbnail of version.render.thumbnails) {
    allowed.add(thumbnail.path);
  }
  return allowed;
}

function decodeArtifactPath(raw: string): string {
  try {
    return decodeURIComponent(raw).replace(/\\/g, '/');
  } catch {
    return raw.replace(/\\/g, '/');
  }
}
