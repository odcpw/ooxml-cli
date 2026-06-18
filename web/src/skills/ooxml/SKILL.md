---
name: ooxml
description: Use when inspecting, validating, rendering, or editing uploaded Office Open XML documents through the thread-scoped OOXML tools.
---

# OOXML Thread Editing

You edit only the current document version in this thread. A thread can contain
several uploaded Office files; use `get_thread_status` to see the library and
`select_document` when the user asks to work on a different file. Use the
provided tools rather than shell commands or arbitrary file paths.

## Workflow

1. Call `get_thread_status` to understand the current file and version.
2. Use `get_ooxml_capabilities` with a focused filter such as `pptx`, `xlsx`,
   `docx`, `chart`, `slide`, `shape`, `table`, `range`, or `style` when you
   need the live command contract. This returns a compact index by default; do
   not request full details unless the compact index is insufficient. Use
   `get_ooxml_command_help` for exact flag syntax. Do not relearn the whole CLI
   every turn; use focused discovery.
3. For reads, prefer `inspect_current_with_ooxml` with command words and a JSON
   flag object. The app supplies the current file through `ooxml serve`.
4. For mutations, prefer `apply_ooxml_ops_to_current` with an ops JSON array
   using commands where `opCompatible=true`. Do not include file, out,
   in-place, dry-run, or no-validate args; the app owns those.
5. Convenience tools such as `inspect_current_document`,
   `show_current_presentation_slide`, `replace_text_in_current_document`,
   `set_current_presentation_slide_shape_text`, and
   `apply_template_to_current_document` are fast paths, not the capability
   boundary. `apply_template_to_current_document` transfers theme colors,
   major/minor fonts, representative PPTX level-1 master default text styles by
   role, and optional chart styling; it does not rebuild slide layouts or copy
   arbitrary shape geometry.
6. Call `validate_current_document` after any change if the mutation result did
   not already include strict validation.
7. For PPTX/PPTM previews, call `render_current_presentation_preview`.

## Boundaries

- Do not edit in place. Every change must publish a new immutable version.
- Do not claim DOCX/XLSX visual preview support; the current preview renderer is
  PPTX/PPTM only.
- If the user wants a structural edit that no tool supports, state the missing
  capability instead of improvising.
- Skills guide the workflow; executable power comes from the provided tools and
  `ooxml serve` capability contract.
