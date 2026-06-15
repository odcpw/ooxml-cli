# Office Repair Conformance Goal

Use this goal when `ooxml-cli` creates PPTX/XLSX files that open, but Microsoft
Office reports that the file needs repair.

## Short Copy-Paste Goal

Tiny version:

```text
/goal Read AGENTS.md and OFFICE_REPAIR_CONFORMANCE_GOAL.md. Use testing-conformance-harnesses and testing-golden-artifacts to continue the Linux-first PPTX/XLSX Office-repair work: reproduce repair-causing outputs, add focused invariant/golden regressions, fix confirmed OOXML package/XML writer bugs, keep go test ./... green, then commit and push a clean milestone.
```

Detailed version:

```text
/goal Read AGENTS.md, GOAL.md, and OFFICE_REPAIR_CONFORMANCE_GOAL.md. Use testing-conformance-harnesses and testing-golden-artifacts to build/fix ooxml-cli's Linux-first PPTX/XLSX Office-repair conformance harness: reproduce repair-causing outputs, add focused package/XML invariant regressions, fix confirmed writer bugs, and keep `GOCACHE=/tmp/ooxml-go-build go test ./...` green before committing. Use strict validation and LibreOffice/soffice open checks as local evidence; treat real Microsoft Office repair prompts on Windows/macOS as the later final oracle, not a blocker for this repo-local slice.
```

## Objective

Make the tool stop producing normal business PPTX/XLSX files that Microsoft
Office repairs on open.

This is not a quest for every obscure OOXML edge case. Focus on the practical
parts that commonly break generated Office files:

- OPC/package structure, relationships, and content types.
- XML root elements, required attributes, and repair-sensitive child order.
- PPTX presentation, slide, layout, master, media, chart, table, and drawing
  references.
- XLSX workbook, worksheet, table, drawing, chart, pivot, shared-string, style,
  and defined-name references.
- Chart/table/pivot parts that were authored or edited by this CLI.

## Operating Mode

- Work locally on Linux first.
- Prefer repo-generated fixtures and small real failing files over speculation.
- If the user provides a repair-prompting file, copy it into ignored local
  scratch/testdata, reproduce the diagnostic, then reduce it to the smallest
  regression fixture that can be committed safely.
- If no failing file is available, generate representative PPTX/XLSX files
  through the current CLI and run the harness against those.
- Do not refactor broadly until the confirmed repair-sensitive bug is fixed and
  covered.
- Keep stdout machine-readable where commands support JSON; progress and
  diagnostics belong on stderr.

## Harness Layers

The conformance harness should give an agent useful evidence before it ever
opens Microsoft Office:

1. Package open and OPC sanity checks.
2. Existing `ooxml validate --strict` coverage.
3. Repair-sensitive invariants for relationships, content types, XML roots,
   element order, counts, and cross-part references.
4. Golden fixtures for known-good and known-bad PPTX/XLSX packages.
5. Optional local `soffice`/LibreOffice open or conversion smoke checks.
6. A machine-readable coverage/provenance report so agents can see which repair
   classes are covered and which still require external Office confirmation.
7. Later Microsoft Office open checks on Windows/macOS as the final oracle for
   user-facing confidence.

## Current High-Value Checks

Keep these covered and extend them when new repair cases appear:

- Worksheet relationship references: drawings, legacy drawings, table parts,
  pivot tables, chart references, missing IDs, unresolved IDs, wrong relationship
  types, and illegal external targets.
- Shared string count drift: validate `sst@count` and `sst@uniqueCount` when
  present; do not require optional counters when absent.
- Styles sanity: validate `cellXfs@count`, `numFmts@count`, and worksheet cell
  style indices against the available style table.
- Content type coverage: required defaults/overrides, duplicate overrides,
  missing override targets, unmapped parts, and content type mismatches.
- Presentation/workbook references: slides, layouts, masters, sheets, tables,
  pivots, drawings, charts, and media targets.
- Schema-order hotspots: worksheet children, slide children, drawing anchors,
  and common chart part trees.
- ZIP/package metadata that Office is known to dislike, such as invalid part
  timestamps.

## Next Practical Slices

1. Add or keep current a machine-readable conformance coverage report exposed
   from the package and CLI, for example `ooxml conformance coverage --json`.
   It should list covered repair classes, fixture evidence, local Office-open
   evidence, and limitations.
2. Run the harness against the newest generated PPTX/XLSX outputs that have
   caused Office repair prompts, if available.
3. For every confirmed failure, add the smallest focused invariant or golden
   regression, then fix the writer that produced the bad part.
4. Add local LibreOffice/soffice evidence where useful, but do not pretend this
   is the same as Microsoft Office repair proof.
5. Commit only green milestones with focused tests and `go test ./...`.

## Done Means

- The repo has a repeatable command/test path that catches the known
  repair-causing PPTX/XLSX package/XML problems.
- Known generated business files pass strict validation and local open smoke
  checks where available.
- Every fixed repair bug has a focused regression test or golden fixture.
- The coverage report makes current proof and current blind spots obvious to an
  agent.
- `GOCACHE=/tmp/ooxml-go-build go test ./...` is green before each commit.
