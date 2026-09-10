'use agent';

import { type AgentProps, useModel, useInstruction, useSkill, useTool } from '@flue/runtime';
import type { MiddlewareHandler } from 'hono';
import ooxmlSkill from '../../../skills/ooxml/SKILL.md';
import { createOoxmlTools } from '../shared/ooxml-tools.ts';
import { requireAuthUser } from '../shared/auth.ts';
import { readThread } from '../shared/storage.ts';
import '../shared/model-provider.ts';
import { editorInstructions } from '../shared/codex-instructions.ts';

export const route: MiddlewareHandler = async (c, next) => {
  const threadId = c.req.param('id');
  if (!threadId) {
    return c.json({ error: 'Thread id is required' }, 400);
  }
  const user = requireAuthUser(c);
  try {
    await readThread(threadId, user.id);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    // Only a genuine missing thread or owner mismatch is a 404. A transient
    // read error (e.g. a concurrent partial write) must not masquerade as
    // "not found" for the legitimate owner; let it surface as a 500.
    if (message === 'Thread not found' || (error as { code?: string }).code === 'ENOENT') {
      return c.json({ error: 'Thread not found' }, 404);
    }
    throw error;
  }
  await next();
};

export function OoxmlEditor({ id }: AgentProps) {
  useModel(process.env.OOXML_FLUE_MODEL || 'openai/gpt-6-astra', {
    thinkingLevel: 'low',
    compaction: { keepRecentTokens: 6000 },
  });
  useSkill(ooxmlSkill);
  for (const tool of createOoxmlTools(id)) useTool(tool);
  useInstruction(editorInstructions);
}

OoxmlEditor.agentName = 'ooxml-editor';
