# Mutation validation and publication seam

Package validation is an observation until its report is asserted. A mutation
must therefore use this transaction:

`input -> destination-local stage -> strict assertion/readback -> publish`

`--no-validate` is the one explicit bypass. A rejected stage is removed; the
input and any pre-existing `--out` remain unchanged. A successful in-place
mutation creates its requested backup only after validation succeeds.

## Authorities

- `validate` computes a report and remains usable by read-only inspection.
- `validate_mutation_output` asserts strict acceptance for a borrowed artifact.
- `validate_owned_mutation_output` asserts strict acceptance and removes a
  rejected caller-owned stage.
- `mutation_staging_path` creates a private stage beside the intended
  destination.
- `finish_mutation_output` is the publication authority. It uses
  same-directory rename and a restore path on platforms where rename cannot
  directly replace an existing file; it never falls back to a partial copy.

Normal DOCX, PPTX, XLSX, VBA, template, import, translation, authoring, repair,
and serve-commit paths use this boundary. Repair keeps its additional policy:
an invalid repair is not promoted. Serve validates its working copy without
deleting it, then publishes through the shared authority.

## Evidence and limits

The executable observation plan is
[`mutation-validation.observe.json`](mutation-validation.observe.json), its
compact receipt is
[`mutation-validation.evidence.json`](mutation-validation.evidence.json), and
the finite intent/factor models are the adjacent semantic-model and factor
JSON files. The observed corpus includes all four package families, a
pre-existing-output sentinel, repair, committed-path chart readback, and an
explicit bypass negative control.

This does not claim crash consistency under power loss, exhaustive filesystem
coverage, or Office interoperability. Operation-specific readback errors after
validation can still leave a private stage; an RAII stage guard is the next
resource-lifecycle improvement if leak evidence appears.
