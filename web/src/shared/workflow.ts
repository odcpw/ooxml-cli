export type SlideWorkflow = {
  mode: 'template' | 'translate';
  sourceDocumentId: string;
  templateDocumentId: string;
  referenceDocumentIds: string[];
  language: string;
  glossary: string;
};

export function validateWorkflow(value: unknown, documentIds: string[]): SlideWorkflow {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Choose a slide task.');
  const data = value as Record<string, unknown>;
  if (data.mode !== 'template' && data.mode !== 'translate') throw new Error('Choose Change template or Translate.');
  const text = (key: string, max: number) => {
    if (typeof data[key] !== 'string' || data[key].length > max) throw new Error(`Invalid ${key}.`);
    return data[key].trim();
  };
  const sourceDocumentId = text('sourceDocumentId', 120);
  const templateDocumentId = text('templateDocumentId', 120);
  if (!Array.isArray(data.referenceDocumentIds) || data.referenceDocumentIds.some(id => typeof id !== 'string')) {
    throw new Error('Choose reference decks from your uploaded files.');
  }
  const referenceDocumentIds = [...new Set(data.referenceDocumentIds as string[])];
  const assigned = [sourceDocumentId, templateDocumentId, ...referenceDocumentIds].filter(Boolean);
  if (assigned.some(id => !documentIds.includes(id))) throw new Error('An assigned file is no longer available. Please choose it again.');
  if (new Set(assigned).size !== assigned.length) throw new Error('The source deck, template deck, and reference decks must be different files.');
  return { mode: data.mode, sourceDocumentId, templateDocumentId, referenceDocumentIds, language: text('language', 80), glossary: text('glossary', 50000) };
}
