import { defineTool } from '@flue/runtime';
import * as v from 'valibot';
import {
  applyOoxmlOpsToCurrent,
  applyTemplateToCurrentDocument,
  getOoxmlCapabilities,
  getOoxmlCommandHelp,
  inspectCurrent,
  inspectCurrentWithOoxml,
  publicThreadSummary,
  renderCurrent,
  replaceTextCurrent,
  searchCurrent,
  setSlideShapeTextCurrent,
  showSlideCurrent,
  validateCurrent,
} from './ooxml-actions.ts';
import { readThread, selectDocument } from './storage.ts';

const emptyParameters = v.object({});

function describedString(description: string) {
  return v.pipe(v.string(), v.description(description));
}

function describedNumber(description: string) {
  return v.pipe(v.number(), v.description(description));
}

function describedBoolean(description: string) {
  return v.pipe(v.boolean(), v.description(description));
}

export function createOoxmlTools(threadId: string) {
  return [
    defineTool({
      name: 'get_thread_status',
      description: 'Show the uploaded Office document library, selected document, current version, previous versions, and preview artifacts for this thread.',
      parameters: emptyParameters,
      execute: async () => JSON.stringify(publicThreadSummary(await readThread(threadId)), null, 2),
    }),
    defineTool({
      name: 'select_document',
      description: 'Select which uploaded document in this thread is current. All later OOXML tools operate on the selected document.',
      parameters: v.object({
        documentId: describedString('Document id from get_thread_status.'),
      }),
      execute: async ({ documentId }) => JSON.stringify(publicThreadSummary(await selectDocument(threadId, String(documentId))), null, 2),
    }),
    defineTool({
      name: 'get_ooxml_capabilities',
      description:
        'Read the live ooxml capabilities contract as a compact command index. Pass a filter such as pptx, xlsx, docx, vba, shape, slide, chart, table, range, style, or package. Use get_ooxml_command_help for exact flags. Set includeDetails only when the compact index is insufficient.',
      parameters: v.object({
        filter: v.optional(describedString('Optional command family or object kind filter.')),
        includeDetails: v.optional(describedBoolean('Return the full raw capabilities JSON. Use sparingly; it can be large.')),
      }),
      execute: async ({ filter, includeDetails }) => getOoxmlCapabilities(typeof filter === 'string' ? filter : undefined, Boolean(includeDetails)),
    }),
    defineTool({
      name: 'get_ooxml_command_help',
      description:
        'Read live --help output for an ooxml command. Use command words without flags, for example "pptx slides show", "xlsx ranges export", or "template apply".',
      parameters: v.object({
        command: v.optional(describedString('Optional command words. Omit for top-level ooxml help.')),
      }),
      execute: async ({ command }) => getOoxmlCommandHelp(typeof command === 'string' ? command : undefined),
    }),
    defineTool({
      name: 'inspect_current_with_ooxml',
      description:
        'Run any serve-allowed read-only ooxml command against the selected document. The app supplies the current file. Put command words in command and flags in argsJson, for example command="pptx slides show", argsJson={"slide":1,"include-text":true}.',
      parameters: v.object({
        command: describedString('OOXML command words, with or without leading "ooxml", and without flags.'),
        argsJson: v.optional(describedString('JSON object of command flags/args. Use flag names without leading --.')),
      }),
      execute: async ({ command, argsJson }) =>
        inspectCurrentWithOoxml({
          threadId,
          command: String(command),
          argsJson: typeof argsJson === 'string' ? argsJson : undefined,
        }),
    }),
    defineTool({
      name: 'apply_ooxml_ops_to_current',
      description:
        'Apply one or more generic ooxml serve/MCP-compatible mutation operations to the selected document and publish a new immutable version. Use commands from get_ooxml_capabilities where opCompatible=true. Do not include file/out/in-place/dry-run/no-validate args; the app owns the file and output path.',
      parameters: v.object({
        opsJson: describedString(
          'JSON array of operations, e.g. [{"command":"pptx replace text","args":{"slide":1,"target":"title","text":"New title"}}].',
        ),
        note: v.optional(describedString('Short version note for the published output.')),
        expectedDocumentId: v.optional(describedString('Current document id from inspect_current_with_ooxml or get_thread_status. Guards against editing the wrong file if selection changes.')),
        expectedVersionId: v.optional(describedString('Current version id from inspect_current_with_ooxml or get_thread_status. Guards against stale edits.')),
      }),
      execute: async ({ opsJson, note, expectedDocumentId, expectedVersionId }) =>
        JSON.stringify(
          await applyOoxmlOpsToCurrent({
            threadId,
            opsJson: String(opsJson),
            note: typeof note === 'string' ? note : undefined,
            expectedDocumentId: typeof expectedDocumentId === 'string' ? expectedDocumentId : undefined,
            expectedVersionId: typeof expectedVersionId === 'string' ? expectedVersionId : undefined,
          }),
          null,
          2,
        ),
    }),
    defineTool({
      name: 'inspect_current_document',
      description: 'Run ooxml inspect on the current Office file and return machine-readable package information.',
      parameters: emptyParameters,
      execute: async () => inspectCurrent(threadId),
    }),
    defineTool({
      name: 'validate_current_document',
      description: 'Run strict OOXML validation on the current Office file.',
      parameters: emptyParameters,
      execute: async () => validateCurrent(threadId),
    }),
    defineTool({
      name: 'search_current_document_text',
      description: 'Search the current Office file for text, formulas, or defined names. Use this before replacing text.',
      parameters: v.object({
        query: describedString('Exact text or search query.'),
        ignoreCase: v.optional(describedBoolean('Match case-insensitively.')),
      }),
      execute: async ({ query, ignoreCase }) =>
        searchCurrent({
          threadId,
          query: String(query),
          ignoreCase: Boolean(ignoreCase),
        }),
    }),
    defineTool({
      name: 'show_current_presentation_slide',
      description: 'Read text, selectors, and bounds for one slide in the selected PPTX/PPTM. Use this before translating or targeted slide edits.',
      parameters: v.object({
        slide: describedNumber('One-based slide number.'),
        includeBounds: v.optional(describedBoolean('Include shape bounds; defaults to true.')),
      }),
      execute: async ({ slide, includeBounds }) =>
        showSlideCurrent({
          threadId,
          slide: Number(slide),
          includeBounds: includeBounds === undefined ? true : Boolean(includeBounds),
        }),
    }),
    defineTool({
      name: 'replace_text_in_current_document',
      description:
        'Replace matching text in the current Office file using ooxml find-generated ops and publish a new immutable version. Search first unless the user gives exact text.',
      parameters: v.object({
        query: describedString('Exact text to replace.'),
        replacement: describedString('Replacement text.'),
        ignoreCase: v.optional(describedBoolean('Match case-insensitively.')),
      }),
      execute: async ({ query, replacement, ignoreCase }) =>
        JSON.stringify(
          await replaceTextCurrent({
            threadId,
            query: String(query),
            replacement: String(replacement),
            ignoreCase: Boolean(ignoreCase),
          }),
          null,
          2,
        ),
    }),
    defineTool({
      name: 'set_current_presentation_slide_shape_text',
      description:
        'Set one text shape on one slide in the selected PPTX/PPTM and publish a new immutable version. Use selectors from show_current_presentation_slide, such as title, body, shape:2, or returned placeholder selectors.',
      parameters: v.object({
        slide: describedNumber('One-based slide number.'),
        target: describedString('Shape target selector from show_current_presentation_slide.'),
        text: describedString('Replacement text for the whole target shape.'),
      }),
      execute: async ({ slide, target, text }) =>
        JSON.stringify(
          await setSlideShapeTextCurrent({
            threadId,
            slide: Number(slide),
            target: String(target),
            text: String(text),
          }),
          null,
          2,
        ),
    }),
    defineTool({
      name: 'apply_template_to_current_document',
      description:
        'Apply transferable design tokens from another uploaded document in this thread to the selected document. This uses ooxml template apply for theme colors and major/minor fonts; chart styling is optional. It does not rebuild slide layouts or copy arbitrary shape geometry.',
      parameters: v.object({
        templateDocumentId: describedString('Document id of the uploaded template or booklet from get_thread_status.'),
        targetCharts: v.optional(describedBoolean('Also apply chart styling when the document contains charts.')),
      }),
      execute: async ({ templateDocumentId, targetCharts }) =>
        JSON.stringify(
          await applyTemplateToCurrentDocument({
            threadId,
            templateDocumentId: String(templateDocumentId),
            targetCharts: Boolean(targetCharts),
          }),
          null,
          2,
        ),
    }),
    defineTool({
      name: 'render_current_presentation_preview',
      description: 'Render the current PPTX/PPTM version to PDF and PNG thumbnails for the browser preview. DOCX/XLSX render is not wired yet.',
      parameters: emptyParameters,
      execute: async () => JSON.stringify(await renderCurrent(threadId), null, 2),
    }),
  ];
}
