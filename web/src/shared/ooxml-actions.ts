import { execFile, spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { basename, extname, join } from 'node:path';
import { createInterface } from 'node:readline';
import { promisify } from 'node:util';
import { isPreviewExtensionSupported, previewUnavailableReasonCopy } from './file-support.ts';
import { runtimeDataRoot } from './runtime-paths.ts';
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
  versionById,
  withThreadMutation,
  type FileVersion,
  type RenderInfo,
  type ThreadDocument,
  type ThreadRecord,
  writeThread,
} from './storage.ts';

const execFileAsync = promisify(execFile);
const OOXML_DEFAULT_BIN = 'ooxml';
const OOXML_DEFAULT_TIMEOUT_MS = 120_000;
const OOXML_DEFAULT_MAX_OUTPUT_BUFFER = 24 * 1024 * 1024;

function resolveOoxmlBin(): string {
  return process.env.OOXML_BIN || OOXML_DEFAULT_BIN;
}

// Remove server filesystem layout from text before it reaches the client. The
// data-dir prefix and any remaining absolute path collapse to a basename, so
// tool diagnostics stay useful without leaking thread/document paths or UUIDs.
function scrubServerPaths(text: string): string {
  if (!text) return text;
  let out = text;
  try {
    const root = runtimeDataRoot();
    if (root) out = out.split(root).join('<data>');
  } catch {
    /* data root unavailable — fall through to the generic path collapse */
  }
  return out.replace(/(?:\/[^\s'"]+)+\/([\w.\-]+)/g, '$1');
}

export async function runOoxml(args: string[], cwd: string): Promise<{ stdout: string; stderr: string }> {
  const bin = resolveOoxmlBin();
  try {
    const result = await execFileAsync(bin, args, {
      cwd,
      maxBuffer: OOXML_DEFAULT_MAX_OUTPUT_BUFFER,
      timeout: OOXML_DEFAULT_TIMEOUT_MS,
    });
    return {
      stdout: result.stdout.toString(),
      stderr: result.stderr.toString(),
    };
  } catch (error) {
    const err = error as Error & { stdout?: Buffer | string; stderr?: Buffer | string; code?: number };
    const stdout = typeof err.stdout === 'string' ? err.stdout : err.stdout?.toString() ?? '';
    const stderr = typeof err.stderr === 'string' ? err.stderr : err.stderr?.toString() ?? '';
    const errorId = randomUUID().slice(0, 8);
    // Full, unscrubbed detail (including absolute paths) stays server-side only.
    console.error('[ooxml-web] ooxml command failed', {
      errorId,
      args: args.join(' '),
      exitCode: err.code,
      stderr: stderr.trim().slice(0, 4000),
    });
    throw new Error(
      [
        `ooxml failed${typeof err.code === 'number' ? ` with exit ${err.code}` : ''} [ref ${errorId}]`,
        scrubServerPaths(stdout.trim()) ? `stdout:\n${scrubServerPaths(stdout.trim())}` : '',
        scrubServerPaths(stderr.trim()) ? `stderr:\n${scrubServerPaths(stderr.trim())}` : '',
        scrubServerPaths(err.message ?? ''),
      ]
        .filter(Boolean)
        .join('\n\n'),
    );
  }
}

async function runOoxmlJson<T>(args: string[], cwd: string): Promise<T> {
  const result = await runOoxml(args, cwd);
  return JSON.parse(result.stdout) as T;
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
  // Flags first, then `--`, then the user-controlled positionals, so a query
  // beginning with '-' is treated as literal text rather than parsed as a flag.
  const args = ['--json', 'find', '--max', '20'];
  if (input.ignoreCase) args.push('--ignore-case');
  args.push('--', input.query, file);
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
  const outPath = newVersionOutputPath(dir, document.id, newVersionId, 'replace', ext);
  const opsPath = join(dir, 'documents', document.id, 'tmp', `${newVersionId}-ops-${randomUUID().slice(0, 8)}.json`);
  await mkdir(join(dir, 'documents', document.id, 'versions'), { recursive: true });
  await mkdir(join(dir, 'documents', document.id, 'tmp'), { recursive: true });

  // Flags first, then `--`, then the user-controlled positionals (query, file),
  // so a query beginning with '-' is treated as literal text, not a flag.
  const findArgs = ['--json', 'find', '--replace', input.replacement, '--to-ops'];
  if (input.ignoreCase) findArgs.push('--ignore-case');
  findArgs.push('--', input.query, file);
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
  const outPath = newVersionOutputPath(dir, document.id, newVersionId, 'slide-text', ext);
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
  targetTextStyles?: boolean;
  targetCharts?: boolean;
}): Promise<Record<string, unknown>> {
  const { thread, document, version } = await currentSelection(input.threadId);
  const { templateDocument, templateVersion } = resolveTemplateSource(thread, document, input.templateDocumentId);
  const file = absoluteVersionPath(thread, version);
  const templateFile = absoluteVersionPath(thread, templateVersion);
  const dir = threadDir(input.threadId);
  const newVersionId = nextVersionId(document);
  const ext = extname(version.path);
  const outPath = newVersionOutputPath(dir, document.id, newVersionId, 'template', ext);
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
  const targetTextStyles = input.targetTextStyles ?? true;
  if (targetTextStyles) args.push('--target-text-styles');
  if (input.targetCharts) args.push('--target-charts');
  const applied = await runOoxml(args, dir);
  const apply = JSON.parse(applied.stdout) as TemplateApplyCliResult;
  if (Number(apply.totalUpdates ?? 0) === 0) {
    await rm(outPath, { force: true }).catch(() => undefined);
    return {
      changed: false,
      reason: 'Template styling produced no transferable updates for the selected document.',
      documentId: document.id,
      currentVersionId: document.currentVersionId,
      apply,
      templateDocumentId: templateDocument.id,
      templateVersionId: templateVersion.id,
      targetTextStyles,
      targetCharts: Boolean(input.targetCharts),
    };
  }

  return publishNewVersion({
    thread,
    document,
    sourceVersion: version,
    versionId: newVersionId,
    outPath,
    note: `Applied transferable template styling from ${templateDocument.title}`,
    apply,
    extra: {
      templateDocumentId: templateDocument.id,
      templateVersionId: templateVersion.id,
      targetTextStyles,
      targetCharts: Boolean(input.targetCharts),
      limitation:
        'Applies transferable design tokens: theme colors, major/minor fonts, representative PPTX level-1 master default text styles by role, plus chart styling when requested. It does not rebuild slide layouts or copy arbitrary shape geometry.',
    },
  });
}

function resolveTemplateSource(
  thread: ThreadRecord,
  document: ThreadDocument,
  templateDocumentId: string,
): { templateDocument: ThreadDocument; templateVersion: FileVersion } {
  const templateDocument = documentById(thread, templateDocumentId);
  if (templateDocument.id === document.id) {
    throw new Error('Choose a different document as the template source.');
  }
  const templateVersion = currentVersion(thread, templateDocument);
  return { templateDocument, templateVersion };
}

export async function createTemplateFormSlideFromCurrent(input: {
  threadId: string;
  templateDocumentId: string;
  sourceSlide?: number;
  templateLayout?: string;
  title?: string;
  subtitle?: string;
  body?: string;
  replaceSourceSlide?: boolean;
  expectedDocumentId?: string;
  expectedVersionId?: string;
}): Promise<Record<string, unknown>> {
  const { thread, document, version } = await currentSelection(input.threadId, {
    documentId: input.expectedDocumentId,
    versionId: input.expectedVersionId,
  });
  if (thread.documents.length > 1 && (!input.expectedDocumentId || !input.expectedVersionId)) {
    throw new Error(
      'Multi-file threads require expectedDocumentId and expectedVersionId for template-form slide creation. Call get_thread_status or inspect_current_with_ooxml, then retry with those guards.',
    );
  }
  assertPresentation(version);

  const { templateDocument, templateVersion } = resolveTemplateSource(thread, document, input.templateDocumentId);
  assertPresentation(templateVersion);

  const sourceSlide = positiveSlideNumber(input.sourceSlide ?? 1, 'sourceSlide');
  const replaceSourceSlide = input.replaceSourceSlide ?? true;
  const dir = threadDir(input.threadId);
  const file = absoluteVersionPath(thread, version);
  const templateFile = absoluteVersionPath(thread, templateVersion);
  const newVersionId = nextVersionId(document);
  const ext = extname(version.path);
  const outPath = newVersionOutputPath(dir, document.id, newVersionId, 'template-form', ext);
  const tmpDir = join(dir, 'documents', document.id, 'tmp');
  await mkdir(join(dir, 'documents', document.id, 'versions'), { recursive: true });
  await mkdir(tmpDir, { recursive: true });

  const importedPath = join(tmpDir, `${newVersionId}-layout-${randomUUID().slice(0, 8)}${ext}`);
  const insertedPath = replaceSourceSlide ? join(tmpDir, `${newVersionId}-inserted-${randomUUID().slice(0, 8)}${ext}`) : outPath;
  const intermediatePaths = replaceSourceSlide ? [importedPath, insertedPath] : [importedPath];

  try {
    const sourceShapeReadback = await runOoxmlJson<PptxShapesShowOutput>(
      ['--json', 'pptx', 'shapes', 'show', file, '--slide', String(sourceSlide), '--include-text', '--include-bounds'],
      dir,
    );
    const extractedText = extractTemplateText(sourceShapeReadback, input);

    const templateLayouts = await runOoxmlJson<PptxLayoutListOutput>(['--json', 'pptx', 'layouts', 'list', templateFile], dir);
    const selectedTemplateLayout = selectTemplateLayout(templateLayouts.layouts ?? [], extractedText, input.templateLayout);
    const targetAssignments = buildTemplateTextAssignments(selectedTemplateLayout, extractedText);

    const imported = await runOoxmlJson<ImportLayoutCliResult>(
      [
        '--json',
        'pptx',
        'layouts',
        'import',
        file,
        '--source',
        templateFile,
        '--layout',
        String(selectedTemplateLayout.number ?? selectedTemplateLayout.name),
        '--theme-policy',
        'import',
        '--out',
        importedPath,
      ],
      dir,
    );
    const importedLayouts = await runOoxmlJson<PptxLayoutListOutput>(['--json', 'pptx', 'layouts', 'list', importedPath], dir);
    const importedLayout = findImportedLayout(importedLayouts.layouts ?? [], imported);
    const newSlideArgs = [
      '--json',
      'pptx',
      'new-slide-from-layout',
      importedPath,
      '--layout',
      String(importedLayout.number),
      '--insert-after',
      String(sourceSlide),
      '--out',
      insertedPath,
    ];
    for (const assignment of targetAssignments) {
      newSlideArgs.push('--set-text', `${assignment.target}=${assignment.text}`);
    }
    const inserted = await runOoxmlJson<NewSlideCliResult>(newSlideArgs, dir);

    let deleted: SlidesDeleteCliResult | undefined;
    if (replaceSourceSlide) {
      deleted = await runOoxmlJson<SlidesDeleteCliResult>(
        ['--json', 'pptx', 'slides', 'delete', insertedPath, String(sourceSlide), '--out', outPath],
        dir,
      );
    }

    return publishNewVersion({
      thread,
      document,
      sourceVersion: version,
      versionId: newVersionId,
      outPath,
      note: `Created template-form slide from slide ${sourceSlide} using ${templateDocument.title}`,
      apply: {
        workflow: 'template-form-slide',
        sourceSlide,
        replaceSourceSlide,
        selectedTemplateLayout,
        importedLayout,
        assignments: targetAssignments.map((assignment) => ({ target: assignment.target, textLength: assignment.text.length })),
        import: imported,
        newSlide: inserted,
        delete: deleted,
      },
      extra: {
        templateDocumentId: templateDocument.id,
        templateVersionId: templateVersion.id,
        sourceSlide,
        newSlideNumber: replaceSourceSlide ? sourceSlide : inserted.newSlideNumber,
        selectedTemplateLayout: {
          number: selectedTemplateLayout.number,
          name: selectedTemplateLayout.name,
          placeholders: selectedTemplateLayout.placeholders,
        },
        textMapping: targetAssignments.map((assignment) => ({
          target: assignment.target,
          textLength: assignment.text.length,
        })),
        limitation:
          'Creates a new slide from an imported template layout and fills text placeholders. It preserves the template layout/master/theme chain for that slide, but it does not automatically map arbitrary freeform shapes, tables, charts, or images into template-specific slots.',
      },
    });
  } catch (error) {
    await rm(outPath, { force: true }).catch(() => undefined);
    throw error;
  } finally {
    await Promise.all(intermediatePaths.map((path) => rm(path, { force: true }).catch(() => undefined)));
  }
}

type TemplateApplyCliResult = {
  changed?: boolean;
  totalUpdates?: number;
  skipped?: string[];
  warnings?: string[];
  output?: string;
};

type PptxShapeInfo = {
  order?: number;
  shapeName?: string;
  targetKind?: string;
  primarySelector?: string;
  textCapable?: boolean;
  textPreview?: string;
  placeholder?: {
    key?: string;
    role?: string;
    index?: number;
  };
};

type PptxShapesShowOutput = {
  shapes?: PptxShapeInfo[];
};

type PptxLayoutEntry = {
  number?: number;
  name?: string;
  partUri?: string;
  placeholderCount?: number;
  placeholders?: string[];
};

type PptxLayoutListOutput = {
  layouts?: PptxLayoutEntry[];
};

type ImportLayoutCliResult = {
  targetLayoutUri?: string;
  targetMasterUri?: string;
  themeUri?: string;
  name?: string;
  imported?: boolean;
  masterImported?: boolean;
};

type NewSlideCliResult = {
  output?: string;
  layout?: string;
  insertAfter?: number;
  newSlideNumber?: number;
  newSlideId?: number;
  newSlideUri?: string;
};

type SlidesDeleteCliResult = {
  output?: string;
  deletedSlide?: number;
  remainingSlides?: number;
};

type TemplateText = {
  title?: string;
  subtitle?: string;
  body?: string;
};

type TemplateTextAssignment = {
  target: string;
  text: string;
};

function positiveSlideNumber(value: number, label: string): number {
  if (!Number.isFinite(value)) {
    throw new Error(`${label} must be a finite slide number.`);
  }
  const normalized = Math.trunc(value);
  if (normalized < 1) {
    throw new Error(`${label} must be 1 or greater.`);
  }
  return normalized;
}

function extractTemplateText(show: PptxShapesShowOutput, overrides: Partial<TemplateText>): TemplateText {
  const sortedShapes = (show.shapes ?? [])
    .filter((shape) => shape.textCapable !== false)
    .sort((a, b) => (a.order ?? 0) - (b.order ?? 0));
  const textShapes = sortedShapes
    .map((shape) => ({ shape, text: cleanSlideText(shape.textPreview ?? '') }))
    .filter((entry) => entry.text);

  const findByRole = (role: string): string | undefined => {
    const found = textShapes.find(({ shape }) => shapeRole(shape) === role);
    return found?.text;
  };

  const title = cleanSlideText(overrides.title ?? '') || findByRole('title') || textShapes[0]?.text;
  const subtitle = cleanSlideText(overrides.subtitle ?? '') || findByRole('subtitle');
  const explicitBody = cleanSlideText(overrides.body ?? '');
  const body =
    explicitBody ||
    textShapes
      .filter(({ text }) => text !== title && text !== subtitle)
      .map(({ text }) => text)
      .join('\n\n');

  if (!title && !subtitle && !body) {
    throw new Error('No source slide text was found to map into the template layout.');
  }
  return {
    title: title || undefined,
    subtitle: subtitle || undefined,
    body: body || undefined,
  };
}

function shapeRole(shape: PptxShapeInfo): string {
  const values = [shape.primarySelector, shape.targetKind, shape.placeholder?.role, shape.placeholder?.key]
    .filter((value): value is string => Boolean(value))
    .map((value) => value.toLowerCase());
  if (values.some((value) => value === 'title' || value.includes('title'))) return 'title';
  if (values.some((value) => value === 'subtitle' || value.includes('subtitle'))) return 'subtitle';
  if (values.some((value) => value === 'body' || value.startsWith('body'))) return 'body';
  return '';
}

function cleanSlideText(text: string): string {
  return text.replace(/\r\n/g, '\n').replace(/\n{3,}/g, '\n\n').trim();
}

function selectTemplateLayout(layouts: PptxLayoutEntry[], text: TemplateText, requestedLayout?: string): PptxLayoutEntry {
  if (layouts.length === 0) {
    throw new Error('The template document has no slide layouts to use.');
  }
  const requested = requestedLayout?.trim();
  if (requested) {
    const match = layouts.find((layout) => String(layout.number) === requested || layout.name === requested);
    if (!match) {
      throw new Error(`Template layout not found: ${requested}`);
    }
    return requireLayoutNumber(match);
  }

  const scored = layouts
    .map((layout) => ({ layout, score: scoreTemplateLayout(layout, text) }))
    .sort((a, b) => b.score - a.score || (a.layout.number ?? Number.MAX_SAFE_INTEGER) - (b.layout.number ?? Number.MAX_SAFE_INTEGER));
  const best = scored[0];
  if (!best || best.score <= 0) {
    throw new Error('Could not find a template layout with usable text placeholders.');
  }
  return requireLayoutNumber(best.layout);
}

function scoreTemplateLayout(layout: PptxLayoutEntry, text: TemplateText): number {
  const placeholders = layoutPlaceholders(layout);
  let score = 0;
  if (text.title && firstPlaceholderForRole(placeholders, 'title')) score += 20;
  if (text.body && firstPlaceholderForRole(placeholders, 'body')) score += 30;
  if (text.subtitle && firstPlaceholderForRole(placeholders, 'subtitle')) score += 8;
  if (firstPlaceholderForRole(placeholders, 'body')) score += 3;
  if (firstPlaceholderForRole(placeholders, 'title')) score += 2;
  if ((layout.placeholderCount ?? placeholders.length) === 0) score -= 100;
  return score;
}

function buildTemplateTextAssignments(layout: PptxLayoutEntry, text: TemplateText): TemplateTextAssignment[] {
  const placeholders = layoutPlaceholders(layout);
  const titleTarget = firstPlaceholderForRole(placeholders, 'title');
  const subtitleTarget = firstPlaceholderForRole(placeholders, 'subtitle');
  const bodyTarget = firstPlaceholderForRole(placeholders, 'body');
  const assignments: TemplateTextAssignment[] = [];

  if (titleTarget && text.title) assignments.push({ target: titleTarget, text: text.title });
  if (subtitleTarget && text.subtitle) assignments.push({ target: subtitleTarget, text: text.subtitle });

  const bodyParts = [!subtitleTarget ? text.subtitle : undefined, text.body].filter((part): part is string => Boolean(part));
  if (bodyTarget && bodyParts.length > 0) {
    assignments.push({ target: bodyTarget, text: bodyParts.join('\n\n') });
  }
  if (assignments.length === 0) {
    const fallback = placeholders[0];
    const fallbackText = [text.title, text.subtitle, text.body].filter((part): part is string => Boolean(part)).join('\n\n');
    if (!fallback || !fallbackText) {
      throw new Error('Could not map source slide text into the selected template layout placeholders.');
    }
    assignments.push({ target: fallback, text: fallbackText });
  }
  return assignments;
}

function layoutPlaceholders(layout: PptxLayoutEntry): string[] {
  return (layout.placeholders ?? []).map((placeholder) => placeholder.trim()).filter(Boolean);
}

function firstPlaceholderForRole(placeholders: string[], role: 'title' | 'subtitle' | 'body'): string | undefined {
  if (role === 'title') {
    return placeholders.find((placeholder) => placeholder === 'title' || placeholder.startsWith('title:'));
  }
  if (role === 'subtitle') {
    return placeholders.find((placeholder) => placeholder === 'subtitle' || placeholder.startsWith('subtitle:'));
  }
  return placeholders.find((placeholder) => placeholder === 'body' || placeholder.startsWith('body:'));
}

function findImportedLayout(layouts: PptxLayoutEntry[], imported: ImportLayoutCliResult): PptxLayoutEntry {
  const byUri = imported.targetLayoutUri ? layouts.find((layout) => layout.partUri === imported.targetLayoutUri) : undefined;
  const byName = imported.name ? layouts.find((layout) => layout.name === imported.name) : undefined;
  const found = byUri ?? byName;
  if (!found) {
    throw new Error('Imported template layout was not discoverable after import.');
  }
  return requireLayoutNumber(found);
}

function requireLayoutNumber(layout: PptxLayoutEntry): PptxLayoutEntry {
  if (!layout.number || layout.number < 1) {
    throw new Error(`Template layout ${layout.name || layout.partUri || '(unnamed)'} did not include a usable layout number.`);
  }
  return layout;
}

export async function inspectCurrentWithOoxml(input: {
  threadId: string;
  command: string;
  argsJson?: string;
}): Promise<string> {
  const { thread, document, version } = await currentSelection(input.threadId);
  const file = absoluteVersionPath(thread, version);
  const responses = await runOoxmlServe(
    serveRequest(1, 'open', { file, dryRun: true }),
    (sessionId) => [
      serveRequest(2, 'inspect', {
        session: sessionId,
        command: normalizeServeCommand(input.command),
        args: parseArgsJson(input.argsJson),
      }),
      serveRequest(3, 'abort', { session: sessionId }),
    ],
    threadDir(input.threadId),
  );
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
  const outPath = newVersionOutputPath(dir, document.id, newVersionId, 'ooxml', ext);
  await mkdir(join(dir, 'documents', document.id, 'versions'), { recursive: true });

  let responses: ServeResponse[];
  try {
    responses = await runOoxmlServe(
      serveRequest(1, 'open', { file, out: outPath }),
      (sessionId) => {
        const requests: ServeRequest[] = [];
        operations.forEach((operation, index) => {
          requests.push(
            serveRequest(index + 2, 'op', {
              session: sessionId,
              command: normalizeServeCommand(operation.command),
              args: operation.args ?? {},
            }),
          );
        });
        requests.push(serveRequest(operations.length + 2, 'validate', { session: sessionId }));
        requests.push(serveRequest(operations.length + 3, 'commit', { session: sessionId }));
        return requests;
      },
      dir,
    );
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

async function countSlidesSafe(file: string, cwd: string): Promise<number> {
  try {
    const listed = await runOoxmlJson<{ slides?: unknown[] }>(['--json', 'pptx', 'slides', 'list', file], cwd);
    return Array.isArray(listed.slides) ? listed.slides.length : 0;
  } catch {
    // Cannot determine the count -> render all (pre-existing behavior). The
    // upload zip-bomb guard already bounds total package size.
    return 0;
  }
}

export async function renderCurrent(threadId: string): Promise<Record<string, unknown>> {
  const { thread, document, version } = await currentSelection(threadId);
  if (!previewSupportedFor(version)) {
    return {
      rendered: false,
      reason: previewUnavailableReasonCopy,
      currentDocumentId: document.id,
      currentVersionId: document.currentVersionId,
      currentExtension: extname(version.path).toLowerCase(),
    };
  }

  const dir = threadDir(threadId);
  const file = absoluteVersionPath(thread, version);
  const renderDir = join(dir, 'documents', document.id, 'renders', `${version.id}-${randomUUID().slice(0, 8)}`);
  await mkdir(renderDir, { recursive: true });

  // Cap how many slides we rasterize so one attacker-controlled many-slide deck
  // (slides compress heavily) cannot fill the shared data volume with PNGs.
  // Normal decks (<= cap) render exactly as before; only oversize decks get a
  // 1..cap range, which pdftoppm rejects unless we know the deck is larger.
  const maxRenderSlides = Math.max(1, Math.trunc(Number(process.env.OOXML_RENDER_MAX_SLIDES) || 200));
  const slideArgs = (await countSlidesSafe(file, dir)) > maxRenderSlides ? ['--slides', `1-${maxRenderSlides}`] : [];
  const rendered = await runOoxml(['--json', 'pptx', 'render', file, '--out', renderDir, '--thumbnails', ...slideArgs], dir);
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
  try {
    await withThreadMutation(threadId, async () => {
      const latestThread = await readThread(threadId);
      const latestDocument = documentById(latestThread, document.id);
      const latestVersion = versionById(latestDocument, version.id);
      latestVersion.render = renderInfo;
      await writeThread(latestThread);
    });
  } catch (error) {
    await rm(renderDir, { recursive: true, force: true }).catch(() => undefined);
    throw error;
  }

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
  try {
    const validate = await runOoxml(['--json', '--strict', 'validate', input.outPath], threadDir(input.thread.id));
    const validateResult = JSON.parse(validate.stdout);
    const published = await withThreadMutation(input.thread.id, async () => {
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
      const newVersion: FileVersion = {
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
      return { latestThread, latestDocument, newVersion };
    });

    return {
      changed: true,
      documentId: published.latestDocument.id,
      version: published.newVersion,
      apply: input.apply,
      validate: validateResult,
      downloadUrl: fileUrlFor(published.latestThread.id, published.latestDocument.id, input.versionId),
      ...input.extra,
    };
  } catch (error) {
    await rm(input.outPath, { force: true }).catch(() => undefined);
    throw error;
  }
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

type ServeResponseId = number | string;

type ServeResponse = {
  jsonrpc: '2.0';
  id: ServeResponseId;
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

type ServeOpenResult = {
  sessionId: string;
  type?: string;
};

function serveRequest(id: number, method: string, params?: Record<string, unknown>): ServeRequest {
  return { jsonrpc: '2.0', id, method, params };
}

function serveResponseKey(id: ServeResponseId): string {
  return String(id);
}

async function runOoxmlServe(
  openRequest: ServeRequest,
  buildFollowUpRequests: (sessionId: string) => ServeRequest[],
  cwd: string,
): Promise<ServeResponse[]> {
  const bin = resolveOoxmlBin();
  const maxServeOutputBytes = positiveIntegerEnv('OOXML_SERVE_MAX_OUTPUT_BYTES', OOXML_DEFAULT_MAX_OUTPUT_BUFFER);
  const child = spawn(bin, ['serve'], {
    cwd,
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  const stdout: Buffer[] = [];
  const stderr: Buffer[] = [];
  const responsesById = new Map<string, ServeResponse>();
  const pendingResponses = new Map<string, (response: ServeResponse) => void>();
  const ignoredLines: string[] = [];
  let stdoutBytes = 0;
  let stderrBytes = 0;
  let outputLimitError: string | null = null;
  let settled = false;
  let followUpRequests: ServeRequest[] = [];

  const timeout = setTimeout(() => {
    if (!settled) child.kill('SIGKILL');
  }, OOXML_DEFAULT_TIMEOUT_MS);

  const stdoutLines = createInterface({ input: child.stdout });
  stdoutLines.on('line', (line) => {
    const trimmed = line.trim();
    if (!trimmed) return;
    let parsed: unknown;
    try {
      parsed = JSON.parse(trimmed);
    } catch {
      ignoredLines.push(trimmed);
      return;
    }
    if (!isServeResponse(parsed)) {
      ignoredLines.push(trimmed);
      return;
    }
    const responseKey = serveResponseKey(parsed.id);
    responsesById.set(responseKey, parsed);
    const resolvePending = pendingResponses.get(responseKey);
    if (resolvePending) {
      pendingResponses.delete(responseKey);
      resolvePending(parsed);
    }
  });

  child.stdout.on('data', (chunk: Buffer) => {
    stdoutBytes += chunk.length;
    if (stdoutBytes > maxServeOutputBytes) {
      outputLimitError = `ooxml serve stdout exceeded ${maxServeOutputBytes} bytes.`;
      child.kill('SIGKILL');
      return;
    }
    stdout.push(chunk);
  });
  child.stderr.on('data', (chunk: Buffer) => {
    stderrBytes += chunk.length;
    if (stderrBytes > maxServeOutputBytes) {
      outputLimitError = `ooxml serve stderr exceeded ${maxServeOutputBytes} bytes.`;
      child.kill('SIGKILL');
      return;
    }
    stderr.push(chunk);
  });

  const closePromise = new Promise<number | null>((resolve, reject) => {
    child.on('error', reject);
    child.on('close', resolve);
  });

  let code: number | null;
  try {
    child.stdin.write(`${JSON.stringify(openRequest)}\n`);
    const openResponse = await Promise.race([
      waitForServeResponse(openRequest, responsesById, pendingResponses),
      closePromise.then(() => undefined),
    ]);
    if (openResponse) {
      const sessionId = sessionIdFromOpenResponse(openResponse);
      followUpRequests = buildFollowUpRequests(sessionId);
      for (const request of followUpRequests) {
        child.stdin.write(`${JSON.stringify(request)}\n`);
      }
      child.stdin.end();
    }
    code = await closePromise;
  } finally {
    settled = true;
    clearTimeout(timeout);
    if (!child.stdin.destroyed && !child.stdin.writableEnded) child.stdin.end();
    if (!child.killed && child.exitCode === null) child.kill('SIGKILL');
    stdoutLines.close();
  }

  const stdoutText = Buffer.concat(stdout).toString();
  const stderrText = Buffer.concat(stderr).toString();
  const allRequests = [openRequest, ...followUpRequests];

  if (outputLimitError) {
    throw new Error(outputLimitError);
  }

  if (code !== 0) {
    const errorId = randomUUID().slice(0, 8);
    console.error('[ooxml-web] ooxml serve failed', { errorId, exitCode: code, stderr: stderrText.trim().slice(0, 4000) });
    throw new Error(
      [
        `ooxml serve failed${typeof code === 'number' ? ` with exit ${code}` : ''} [ref ${errorId}]`,
        scrubServerPaths(stderrText.trim()) ? `stderr:\n${scrubServerPaths(stderrText.trim())}` : '',
        scrubServerPaths(stdoutText.trim()) ? `stdout:\n${scrubServerPaths(stdoutText.trim())}` : '',
      ]
        .filter(Boolean)
        .join('\n\n'),
    );
  }
  return allRequests.map((request) => {
    const response = responsesById.get(serveResponseKey(request.id));
    if (!response) {
      throw new Error(
        [
          `Missing ooxml serve response for request ${request.id} (${request.method})`,
          ignoredLines.length ? `ignored stdout lines:\n${scrubServerPaths(ignoredLines.join('\n'))}` : '',
          scrubServerPaths(stdoutText.trim()) ? `stdout:\n${scrubServerPaths(stdoutText.trim())}` : '',
          scrubServerPaths(stderrText.trim()) ? `stderr:\n${scrubServerPaths(stderrText.trim())}` : '',
        ]
          .filter(Boolean)
          .join('\n\n'),
      );
    }
    return response;
  });
}

function waitForServeResponse(
  request: ServeRequest,
  responsesById: Map<string, ServeResponse>,
  pendingResponses: Map<string, (response: ServeResponse) => void>,
): Promise<ServeResponse> {
  const requestKey = serveResponseKey(request.id);
  const existing = responsesById.get(requestKey);
  if (existing) return Promise.resolve(existing);
  return new Promise<ServeResponse>((resolve) => {
    pendingResponses.set(requestKey, resolve);
  });
}

function isServeResponse(value: unknown): value is ServeResponse {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const candidate = value as Partial<ServeResponse>;
  return candidate.jsonrpc === '2.0' && (typeof candidate.id === 'number' || typeof candidate.id === 'string');
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

function sessionIdFromOpenResponse(response: ServeResponse): string {
  const result = resultOrThrow(response, 'open');
  if (!result || typeof result !== 'object' || Array.isArray(result)) {
    throw new Error('Invalid ooxml serve open response: missing sessionId');
  }
  const { sessionId } = result as Partial<ServeOpenResult>;
  if (typeof sessionId !== 'string' || !sessionId.trim()) {
    throw new Error('Invalid ooxml serve open response: missing sessionId');
  }
  return sessionId;
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
  return isPreviewExtensionSupported(extname(version.path));
}

function newVersionOutputPath(dir: string, documentId: string, versionId: string, label: string, ext: string): string {
  const operationId = randomUUID().slice(0, 8);
  return join(dir, 'documents', documentId, 'versions', `${versionId}-${label}-${operationId}${ext}`);
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

function positiveIntegerEnv(name: string, fallback: number): number {
  const parsed = Number(process.env[name]);
  if (!Number.isFinite(parsed) || parsed <= 0) return fallback;
  return Math.trunc(parsed);
}
