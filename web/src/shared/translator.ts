import OpenAI from 'openai';

export type TranslationSegment = { id: string; sourceText: string };
export type TranslationRequest = { sourceLang: string; targetLang: string };
export type Translator = (
  segments: TranslationSegment[],
  request: TranslationRequest,
) => Promise<Map<string, string>>;

export type TranslatorMode = 'model' | 'identity';

/**
 * `identity` copies every source segment unchanged so the export -> apply ->
 * validate plumbing can be proved without model credentials. Anything else is
 * the real model path.
 */
export function translatorMode(): TranslatorMode {
  return process.env.OOXML_TRANSLATE_MODE === 'identity' ? 'identity' : 'model';
}

/**
 * `OOXML_TRANSLATE_MODEL` (falling back to the agent's `OOXML_FLUE_MODEL`) uses
 * Flue's `provider/model` spelling. Only the OpenAI provider is wired for the
 * direct translation call; a bare model name is treated as an OpenAI model.
 */
export function resolveTranslateModel(
  raw: string = process.env.OOXML_TRANSLATE_MODEL || process.env.OOXML_FLUE_MODEL || 'openai/gpt-5.5',
): string {
  const value = raw.trim();
  const slash = value.indexOf('/');
  if (slash === -1) return value;
  const provider = value.slice(0, slash);
  if (provider !== 'openai') {
    throw new Error(
      `Translation needs an openai/<model> id; got ${JSON.stringify(value)}. Set OOXML_TRANSLATE_MODEL.`,
    );
  }
  return value.slice(slash + 1);
}

export const identityTranslator: Translator = async (segments) =>
  new Map(segments.map((segment) => [segment.id, segment.sourceText]));

export function createTranslator(): Translator {
  return translatorMode() === 'identity' ? identityTranslator : createModelTranslator();
}

export function createModelTranslator(
  options: { apiKey?: string; model?: string; batchSize?: number } = {},
): Translator {
  const apiKey = options.apiKey ?? process.env.OPENAI_API_KEY;
  if (!apiKey) {
    throw new Error(
      'OPENAI_API_KEY is not configured for translation. Set it, or set OOXML_TRANSLATE_MODE=identity for plumbing-only runs.',
    );
  }
  const model = options.model ?? resolveTranslateModel();
  const batchSize = options.batchSize ?? positiveInteger(process.env.OOXML_TRANSLATE_BATCH_SIZE, 40);
  const client = new OpenAI({ apiKey });
  return async (segments, request) => {
    const translations = new Map<string, string>();
    for (let index = 0; index < segments.length; index += batchSize) {
      const batch = segments.slice(index, index + batchSize);
      const result = await translateBatch(client, model, batch, request);
      for (const [id, text] of result) translations.set(id, text);
    }
    return translations;
  };
}

async function translateBatch(
  client: OpenAI,
  model: string,
  batch: TranslationSegment[],
  request: TranslationRequest,
): Promise<Map<string, string>> {
  const source =
    request.sourceLang && request.sourceLang !== 'auto' ? request.sourceLang : 'the detected source language';
  const system = [
    `You translate presentation slide text from ${source} into ${request.targetLang}.`,
    'Translate every segment faithfully. Keep numbers, product names, URLs, placeholders such as {name} or %s, and line breaks unchanged.',
    'Keep each translation about as long as its source so it still fits the slide.',
    'Reply with JSON only: {"translations":[{"id":"<id>","text":"<translation>"}]} containing exactly one item per input id.',
  ].join(' ');
  const user = JSON.stringify({
    targetLanguage: request.targetLang,
    segments: batch.map((segment) => ({ id: segment.id, text: segment.sourceText })),
  });
  const completion = await client.chat.completions.create({
    model,
    response_format: { type: 'json_object' },
    messages: [
      { role: 'system', content: system },
      { role: 'user', content: user },
    ],
  });
  const content = completion.choices[0]?.message?.content ?? '';
  return parseTranslations(content, batch);
}

export function parseTranslations(content: string, batch: TranslationSegment[]): Map<string, string> {
  let parsed: unknown;
  try {
    parsed = JSON.parse(content);
  } catch (error) {
    throw new Error(
      `The translation model did not return JSON: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  const items = (parsed as { translations?: unknown } | null)?.translations;
  if (!Array.isArray(items)) {
    throw new Error('The translation model response has no "translations" array.');
  }
  const expected = new Set(batch.map((segment) => segment.id));
  const translations = new Map<string, string>();
  for (const item of items) {
    const id = (item as { id?: unknown } | null)?.id;
    const text = (item as { text?: unknown } | null)?.text;
    if (typeof id !== 'string' || typeof text !== 'string') continue;
    if (expected.has(id)) translations.set(id, text);
  }
  const missing = batch.filter((segment) => !translations.has(segment.id)).map((segment) => segment.id);
  if (missing.length > 0) {
    throw new Error(
      `The translation model skipped ${missing.length} of ${batch.length} segment(s): ${missing.slice(0, 5).join(', ')}${missing.length > 5 ? ', ...' : ''}`,
    );
  }
  return translations;
}

/** Accept BCP 47 shaped tags only; the tag is handed to the CLI as a flag value. */
export function normalizeLanguageTag(value: string): string {
  const tag = value.trim();
  if (!/^[A-Za-z]{2,3}(-[A-Za-z0-9]{2,8})*$/.test(tag)) {
    throw new Error(`Language must be a BCP 47 tag such as "de" or "pt-BR"; got ${JSON.stringify(value)}.`);
  }
  return tag;
}

function positiveInteger(value: string | undefined, fallback: number): number {
  const parsed = Number(value);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : fallback;
}
