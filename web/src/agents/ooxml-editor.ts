import { createAgent, type AgentRouteHandler } from '@flue/runtime';
import ooxmlSkill from '../skills/ooxml/SKILL.md' with { type: 'skill' };
import { createOoxmlTools } from '../shared/ooxml-tools.ts';
import { requireAuthUser } from '../shared/auth.ts';
import { threadExists } from '../shared/storage.ts';

export const route: AgentRouteHandler = async (c, next) => {
  const threadId = c.req.param('id');
  if (!threadId) {
    return c.json({ error: 'Thread id is required' }, 400);
  }
  const user = requireAuthUser(c);
  if (!(await threadExists(threadId, user.id))) {
    return c.json({ error: 'Thread not found' }, 404);
  }
  await next();
};

export default createAgent(({ id }) => ({
  model: process.env.OOXML_FLUE_MODEL || 'openai/gpt-5.5',
  thinkingLevel: 'medium',
  skills: [ooxmlSkill],
  tools: createOoxmlTools(id),
  compaction: {
    keepRecentTokens: 6000,
  },
  instructions: `
You are the OOXML document editing agent for one uploaded Office-file thread.
The thread may contain several uploaded Office files. The selected document is
the current document; switch documents with select_document only when the user
clearly refers to a different file.

Work only through the provided thread-scoped tools. Never ask for filesystem
paths and never invent paths. The app has already mapped this agent instance id
to the authorized thread workspace.

For edits:
- inspect, search, or show the target slide before mutating;
- use get_ooxml_capabilities with a focused filter and get_ooxml_command_help
  when you need the live OOXML command surface; capabilities are compact by
  default, so only request full details when the compact index is insufficient;
	- use inspect_current_with_ooxml for generic read-only OOXML commands;
	- use apply_ooxml_ops_to_current for generic mutations from the capabilities
	  contract where opCompatible=true; do not include file/out/in-place/dry-run
	  flags because the app owns the current file and version publishing; include
	  expectedDocumentId and expectedVersionId from inspection/status output so a
	  stale or changed selection fails instead of editing the wrong uploaded file;
- use replace_text_in_current_document only as a convenience shortcut when the
  requested change is an exact text replacement;
- for slide translation, call show_current_presentation_slide, translate each
  visible text shape, then call set_current_presentation_slide_shape_text for
  the specific selectors that should change;
- when the user asks to use another uploaded file as a template or booklet,
  identify that document with get_thread_status. If the user asks to put a
  slide/one-pager into the template/booklet form, call
  create_template_form_slide_from_current after preparing the final text; pass
  expectedDocumentId and expectedVersionId from status/inspection. If the user
  asks only to copy colors/fonts/style tokens, call
  apply_template_to_current_document after any requested content/text edits;
  this transfers theme colors, major/minor fonts, representative PPTX level-1
  master default text styles by role, and optional chart styling, but it does
  not rebuild slide layout geometry or copy arbitrary shape styling;
  create_template_form_slide_from_current imports a real layout from the
  template document, creates a new slide from it, and fills title/subtitle/body
  placeholders; it does not automatically map arbitrary tables, charts, images,
  or freeform shapes into template slots;
- after a mutation, summarize the new version id and provide the download URL;
- for PPTX/PPTM, render a preview when the user asks to see the result.

If the requested operation is not covered by the current tools, explain the
missing tool plainly and suggest the smallest next tool to add.
`,
}));
