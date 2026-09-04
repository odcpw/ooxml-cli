import { defineTool } from '@flue/runtime';
import * as v from 'valibot';
import { previewSupportedLabel } from './file-support.ts';
import {
  applyOoxmlOpsToCurrent,
  applyTemplateToCurrentDocument,
  buildCurrentWithTypedMcp,
  checkCurrentWithTypedMcp,
  createTemplateFormSlideFromCurrent,
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

function structured(value: unknown) {
  return JSON.parse(JSON.stringify(value));
}

export function createOoxmlTools(threadId: string) {
  return [
    defineTool({
      name: 'get_thread_status',
      description: 'Show the uploaded Office document library, selected document, current version, previous versions, and preview artifacts for this thread.',
      input: emptyParameters,
      run: async () => structured(publicThreadSummary(await readThread(threadId))),
    }),
    defineTool({
      name: 'select_document',
      description: 'Select which uploaded document in this thread is current. All later OOXML tools operate on the selected document.',
      input: v.object({
        documentId: describedString('Document id from get_thread_status.'),
      }),
      run: async ({ input: { documentId } }) =>
        structured(publicThreadSummary(await selectDocument(threadId, String(documentId)))),
    }),
    defineTool({
      name: 'get_ooxml_capabilities',
      description:
        'Read the live ooxml capabilities contract as a compact command index. Pass a filter such as pptx, xlsx, docx, vba, shape, slide, chart, table, range, style, or package. Use get_ooxml_command_help for exact flags. Set includeDetails only when the compact index is insufficient.',
      input: v.object({
        filter: v.optional(describedString('Optional command family or object kind filter.')),
        includeDetails: v.optional(describedBoolean('Return the full raw capabilities JSON. Use sparingly; it can be large.')),
      }),
      run: async ({ input: { filter, includeDetails } }) =>
        JSON.parse(
          await getOoxmlCapabilities(
            typeof filter === 'string' ? filter : undefined,
            Boolean(includeDetails),
          ),
        ),
    }),
    defineTool({
      name: 'get_ooxml_command_help',
      description:
        'Read live --help output for an ooxml command. Use command words without flags, for example "pptx slides show", "xlsx ranges export", or "template apply".',
      input: v.object({
        command: v.optional(describedString('Optional command words. Omit for top-level ooxml help.')),
      }),
      run: async ({ input: { command } }) =>
        getOoxmlCommandHelp(typeof command === 'string' ? command : undefined),
    }),
    defineTool({
      name: 'build_presentation',
      description:
        'Build and strictly validate a complete PPTX from the published pptx-build specification through the typed MCP tool, then publish it as a new immutable version of the selected PPTX.',
      input: v.object({
        specJson: describedString('JSON object conforming to resource://schema/pptx-build.'),
        note: v.optional(describedString('Short version note for the published presentation.')),
      }),
      run: async ({ input: { specJson, note } }) =>
        structured(
          await buildCurrentWithTypedMcp({
            threadId,
            family: 'pptx',
            specJson: String(specJson),
            note: typeof note === 'string' ? note : undefined,
          }),
        ),
    }),
    defineTool({
      name: 'build_workbook',
      description:
        'Build and strictly validate a complete XLSX from the published xlsx-build specification through the typed MCP tool, then publish it as a new immutable version of the selected XLSX.',
      input: v.object({
        specJson: describedString('JSON object conforming to resource://schema/xlsx-build.'),
        note: v.optional(describedString('Short version note for the published workbook.')),
      }),
      run: async ({ input: { specJson, note } }) =>
        structured(
          await buildCurrentWithTypedMcp({
            threadId,
            family: 'xlsx',
            specJson: String(specJson),
            note: typeof note === 'string' ? note : undefined,
          }),
        ),
    }),
    defineTool({
      name: 'build_document',
      description:
        'Build and strictly validate a complete DOCX from the published docx-build specification through the typed MCP tool, then publish it as a new immutable version of the selected DOCX.',
      input: v.object({
        specJson: describedString('JSON object conforming to resource://schema/docx-build.'),
        note: v.optional(describedString('Short version note for the published document.')),
      }),
      run: async ({ input: { specJson, note } }) =>
        structured(
          await buildCurrentWithTypedMcp({
            threadId,
            family: 'docx',
            specJson: String(specJson),
            note: typeof note === 'string' ? note : undefined,
          }),
        ),
    }),
    defineTool({
      name: 'check_package',
      description:
        'Run the typed MCP proof recipe on the selected package: structural, strict, schema, design, references, and optional visual render findings with executable fix commands.',
      input: v.object({
        openXmlSdk: v.optional(describedString('Schema policy: auto, require, or skip. Defaults to auto.')),
        failOn: v.optional(describedString('Finding threshold: error or warning. Defaults to error.')),
        render: v.optional(describedBoolean('Include the shared visual renderer proof pass.')),
      }),
      run: async ({ input: { openXmlSdk, failOn, render } }) =>
        JSON.parse(
          await checkCurrentWithTypedMcp({
            threadId,
            openXmlSdk: openXmlSdk === 'require' || openXmlSdk === 'skip' ? openXmlSdk : 'auto',
            failOn: failOn === 'warning' ? 'warning' : 'error',
            render: Boolean(render),
          }),
        ),
    }),
    defineTool({
      name: 'inspect_current_with_ooxml',
      description:
        'Run any serve-allowed read-only ooxml command against the selected document. The app supplies the current file. Put command words in command and flags in argsJson, for example command="pptx slides show", argsJson={"slide":1,"include-text":true}.',
      input: v.object({
        command: describedString('OOXML command words, with or without leading "ooxml", and without flags.'),
        argsJson: v.optional(describedString('JSON object of command flags/args. Use flag names without leading --.')),
      }),
      run: async ({ input: { command, argsJson } }) =>
        JSON.parse(
          await inspectCurrentWithOoxml({
            threadId,
            command: String(command),
            argsJson: typeof argsJson === 'string' ? argsJson : undefined,
          }),
        ),
    }),
    defineTool({
      name: 'apply_ooxml_ops_to_current',
      description:
        'Apply one or more generic ooxml serve/MCP-compatible mutation operations to the selected document and publish a new immutable version. Use commands from get_ooxml_capabilities where opCompatible=true. Do not include file/out/in-place/dry-run/no-validate args; the app owns the file and output path.',
      input: v.object({
        opsJson: describedString(
          'JSON array of operations, e.g. [{"command":"pptx replace text","args":{"slide":1,"target":"title","text":"New title"}}].',
        ),
        note: v.optional(describedString('Short version note for the published output.')),
        expectedDocumentId: v.optional(describedString('Current document id from inspect_current_with_ooxml or get_thread_status. Guards against editing the wrong file if selection changes.')),
        expectedVersionId: v.optional(describedString('Current version id from inspect_current_with_ooxml or get_thread_status. Guards against stale edits.')),
      }),
      run: async ({ input: { opsJson, note, expectedDocumentId, expectedVersionId } }) =>
        structured(
          await applyOoxmlOpsToCurrent({
            threadId,
            opsJson: String(opsJson),
            note: typeof note === 'string' ? note : undefined,
            expectedDocumentId: typeof expectedDocumentId === 'string' ? expectedDocumentId : undefined,
            expectedVersionId: typeof expectedVersionId === 'string' ? expectedVersionId : undefined,
          }),
        ),
    }),
    defineTool({
      name: 'inspect_current_document',
      description: 'Run ooxml inspect on the current Office file and return machine-readable package information.',
      input: emptyParameters,
      run: async () => JSON.parse(await inspectCurrent(threadId)),
    }),
    defineTool({
      name: 'validate_current_document',
      description: 'Run strict OOXML validation on the current Office file.',
      input: emptyParameters,
      run: async () => JSON.parse(await validateCurrent(threadId)),
    }),
    defineTool({
      name: 'search_current_document_text',
      description: 'Search the current Office file for text, formulas, or defined names. Use this before replacing text.',
      input: v.object({
        query: describedString('Exact text or search query.'),
        ignoreCase: v.optional(describedBoolean('Match case-insensitively.')),
      }),
      run: async ({ input: { query, ignoreCase } }) =>
        JSON.parse(
          await searchCurrent({
            threadId,
            query: String(query),
            ignoreCase: Boolean(ignoreCase),
          }),
        ),
    }),
    defineTool({
      name: 'show_current_presentation_slide',
      description: 'Read text, selectors, and bounds for one slide in the selected PPTX/PPTM. Use this before translating or targeted slide edits.',
      input: v.object({
        slide: describedNumber('One-based slide number.'),
        includeBounds: v.optional(describedBoolean('Include shape bounds; defaults to true.')),
      }),
      run: async ({ input: { slide, includeBounds } }) =>
        JSON.parse(
          await showSlideCurrent({
            threadId,
            slide: Number(slide),
            includeBounds: includeBounds === undefined ? true : Boolean(includeBounds),
          }),
        ),
    }),
    defineTool({
      name: 'replace_text_in_current_document',
      description:
        'Replace matching text in the current Office file using ooxml find-generated ops and publish a new immutable version. Search first unless the user gives exact text.',
      input: v.object({
        query: describedString('Exact text to replace.'),
        replacement: describedString('Replacement text.'),
        ignoreCase: v.optional(describedBoolean('Match case-insensitively.')),
      }),
      run: async ({ input: { query, replacement, ignoreCase } }) =>
        structured(
          await replaceTextCurrent({
            threadId,
            query: String(query),
            replacement: String(replacement),
            ignoreCase: Boolean(ignoreCase),
          }),
        ),
    }),
    defineTool({
      name: 'set_current_presentation_slide_shape_text',
      description:
        'Set one text shape on one slide in the selected PPTX/PPTM and publish a new immutable version. Use selectors from show_current_presentation_slide, such as title, body, shape:2, or returned placeholder selectors.',
      input: v.object({
        slide: describedNumber('One-based slide number.'),
        target: describedString('Shape target selector from show_current_presentation_slide.'),
        text: describedString('Replacement text for the whole target shape.'),
      }),
      run: async ({ input: { slide, target, text } }) =>
        structured(
          await setSlideShapeTextCurrent({
            threadId,
            slide: Number(slide),
            target: String(target),
            text: String(text),
          }),
        ),
    }),
    defineTool({
      name: 'apply_template_to_current_document',
      description:
        'Apply transferable design tokens from another uploaded document in this thread to the selected document. This uses ooxml template apply for theme colors, major/minor fonts, and representative PPTX level-1 master default text styles by role; chart styling is optional. It does not rebuild slide layouts or copy arbitrary shape geometry.',
      input: v.object({
        templateDocumentId: describedString('Document id of the uploaded template or booklet from get_thread_status.'),
        targetTextStyles: v.optional(describedBoolean('Apply PPTX master default text styles by role. Defaults to true.')),
        targetCharts: v.optional(describedBoolean('Also apply chart styling when the document contains charts.')),
      }),
      run: async ({ input: { templateDocumentId, targetTextStyles, targetCharts } }) =>
        structured(
          await applyTemplateToCurrentDocument({
            threadId,
            templateDocumentId: String(templateDocumentId),
            targetTextStyles: targetTextStyles === undefined ? undefined : Boolean(targetTextStyles),
            targetCharts: Boolean(targetCharts),
          }),
        ),
    }),
    defineTool({
      name: 'create_template_form_slide_from_current',
      description:
        'Create a template-form version of one PPTX/PPTM slide using another uploaded presentation as the booklet/template source. It imports the chosen template layout, creates a new slide from that layout, fills title/subtitle/body text placeholders, and by default replaces the source slide position with the new template-form slide.',
      input: v.object({
        templateDocumentId: describedString('Document id of the uploaded template or booklet from get_thread_status.'),
        sourceSlide: v.optional(describedNumber('One-based source slide number to rebuild. Defaults to 1.')),
        templateLayout: v.optional(describedString('Optional exact template layout number or name. Omit to pick the best title/body layout.')),
        title: v.optional(describedString('Optional final title text to place in the template layout. If omitted, extracted from the source slide.')),
        subtitle: v.optional(describedString('Optional final subtitle text to place in the template layout. If omitted, extracted when present.')),
        body: v.optional(describedString('Optional final body text to place in the template layout. If omitted, extracted from non-title source text.')),
        replaceSourceSlide: v.optional(describedBoolean('Replace the source slide position with the template-form slide. Defaults to true.')),
        expectedDocumentId: v.optional(describedString('Current document id from inspect_current_with_ooxml or get_thread_status. Guards against editing the wrong file if selection changes.')),
        expectedVersionId: v.optional(describedString('Current version id from inspect_current_with_ooxml or get_thread_status. Guards against stale edits.')),
      }),
      run: async ({
        input: {
          templateDocumentId,
          sourceSlide,
          templateLayout,
          title,
          subtitle,
          body,
          replaceSourceSlide,
          expectedDocumentId,
          expectedVersionId,
        },
      }) =>
        structured(
          await createTemplateFormSlideFromCurrent({
            threadId,
            templateDocumentId: String(templateDocumentId),
            sourceSlide: sourceSlide === undefined ? undefined : Number(sourceSlide),
            templateLayout: typeof templateLayout === 'string' ? templateLayout : undefined,
            title: typeof title === 'string' ? title : undefined,
            subtitle: typeof subtitle === 'string' ? subtitle : undefined,
            body: typeof body === 'string' ? body : undefined,
            replaceSourceSlide: replaceSourceSlide === undefined ? undefined : Boolean(replaceSourceSlide),
            expectedDocumentId: typeof expectedDocumentId === 'string' ? expectedDocumentId : undefined,
            expectedVersionId: typeof expectedVersionId === 'string' ? expectedVersionId : undefined,
          }),
        ),
    }),
    defineTool({
      name: 'render_current_presentation_preview',
      description: `Render the current ${previewSupportedLabel} version to PDF and PNG thumbnails for the browser preview. DOCX/XLSX render is not wired yet.`,
      input: emptyParameters,
      run: async () => structured(await renderCurrent(threadId)),
    }),
  ];
}
