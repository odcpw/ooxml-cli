# Pass 6 fresh-agent session

Date: 2026-09-04

Agent context: isolated subagent with no prior conversation

Elapsed wall time: not recorded; no estimate is substituted

Repository mutations: none

Artifact workspace: `$HOME/.cache/ooxml-cli-target/CrimsonOsprey-e-gu5-8-simulator`

## Allowed discovery surfaces

The simulator could inspect only:

- `README.md`
- `skills/ooxml/SKILL.md`
- `ooxml --help`
- `ooxml --json capabilities`

It was prohibited from reading source, tests, prior audit artifacts, git history,
or the reviewed command golden. Named input fixtures were supplied as task
inputs. The simulator did not edit or commit repository files.

## Exact CLI transcript

Paths below are intentionally retained so the session can be audited. Commands
1-4 are discovery. Command 5 is the sole wrong invocation; its diagnostic is
recorded verbatim below. All remaining commands exited zero.

1. `ooxml --help` — exit 0; exposed the root command menu.
2. `ooxml --json capabilities` — exit 0; returned 331 command entries.
3. `ooxml --json capabilities` — exit 0; caller filtered build, edit, proof,
   design, and render entries.
4. `ooxml --json capabilities --schema pptx-build` — exit 0; inspected the
   public PPTX build schema.
5. `ooxml --json pptx build --spec $WORK/q3-review-branded.json --out $WORK/branded-deck.pptx --check --force`
   — exit 2; the first workspace copy put the brand path in `theme`.
6. `ooxml --json xlsx build --spec $REPO/testdata/xlsx/build-spec/sales.json --out $WORK/sales.xlsx --check --force`
   — exit 0; workbook published and embedded checks passed.
7. `ooxml --json docx build --spec $REPO/testdata/docx/build-spec/quarterly-report.json --out $WORK/quarterly-report.docx --check --force`
   — exit 0; document published and embedded checks passed.
8. `ooxml --json pptx build --from-markdown $REPO/testdata/markdown/q3-review.md --out $WORK/markdown-deck.pptx --check --force`
   — exit 0; five-slide deck published. The converter reported one ordered-list
   item preserved as a bullet.
9. `ooxml --json capabilities --schema pptx-build` — exit 0; found the separate
   top-level `brand` property.
10. `ooxml --json capabilities --schema pptx-build` — exit 0; inspected
    `$defs.brandRef`, which accepts a string or `{name,path}`.
11. `ooxml --json pptx build --spec $WORK/q3-review-branded.json --out $WORK/branded-deck.pptx --check --force`
    — exit 0; corrected five-slide branded deck published.
12. `ooxml --json pptx replace text $WORK/branded-deck.pptx --slide 1 --target title --text "Q3 Review — Northwind" --out $WORK/edited-branded-deck.pptx`
    — exit 0; semantic title edit published.
13. `ooxml --json pptx shapes get $WORK/edited-branded-deck.pptx --slide 1 --target title --include-text --include-bounds`
    — exit 0; caller's first `jq` projection selected nonexistent fields.
14. `ooxml --json pptx shapes get $WORK/edited-branded-deck.pptx --slide 1 --target title --include-text --include-bounds`
    — exit 0; raw response confirmed `Q3 Review — Northwind`.
15. `ooxml --json validate --strict $WORK/branded-deck.pptx` — exit 0; zero
    errors and warnings.
16. `ooxml --json validate --strict $WORK/sales.xlsx` — exit 0; zero errors and
    warnings.
17. `ooxml --json validate --strict $WORK/quarterly-report.docx` — exit 0; zero
    errors and warnings.
18. `ooxml --json validate --strict $WORK/markdown-deck.pptx` — exit 0; zero
    errors and warnings.
19. `ooxml --json validate --strict $WORK/edited-branded-deck.pptx` — exit 0;
    zero errors and warnings.
20. `ooxml --json design-check $WORK/edited-branded-deck.pptx` — exit 0; zero
    errors and four warnings (two empty placeholders, source-image upscale, and
    one footer safe-margin finding).
21. `ooxml --json pptx render $WORK/edited-branded-deck.pptx --out $WORK/edited-branded-render`
    — exit 0; LibreOffice produced one PDF and five PNG slides.
22. `ooxml --json check $WORK/branded-deck.pptx --openxml-sdk require --fail-on error`
    — exit 0; strict, structural, conformance, design, and SDK schema checks
    passed with zero errors.
23. `ooxml --json check $WORK/sales.xlsx --openxml-sdk require --fail-on error`
    — exit 0; strict, structural, conformance, reference, design, and SDK schema
    checks passed with zero errors.
24. `ooxml --json check $WORK/quarterly-report.docx --openxml-sdk require --fail-on error`
    — exit 0; strict, structural, conformance, design, and SDK schema checks
    passed with zero errors.
25. `ooxml --json check $WORK/markdown-deck.pptx --openxml-sdk require --fail-on error`
    — exit 0; strict, structural, conformance, layout, design, and SDK schema
    checks passed with zero errors.
26. `ooxml --json check $WORK/edited-branded-deck.pptx --openxml-sdk require --fail-on error`
    — exit 0; strict, structural, conformance, design, and SDK schema checks
    passed with zero errors.
27. `ooxml --json outline $WORK/branded-deck.pptx --depth 2` — exit 0;
    confirmed five slides and the Northwind theme (accent `316F8A`, Arial
    headings, Liberation Sans body).
28. `ooxml --json xlsx charts list $WORK/sales.xlsx --sheet Sales` — exit 0;
    confirmed chart `Units by Region` with North/Central/South/West categories
    and values 12/9/15/11.

`$REPO` is `/home/oliver/Projects/odcpw/ooxml-cli`; `$WORK` is the artifact
workspace named above. The executable for every command was
`/home/oliver/.cache/ooxml-cli-target/2/debug/ooxml`.

## Wrong invocation and error quality

The failed branded build produced this diagnostic:

```text
op 0 (pptx scaffold) failed: unknown PPTX theme "/home/oliver/projects/odcpw/ooxml-cli/testdata/brand/northwind.json"; expected neutral, corporate, warm, or dark
```

Classification: **useful error**. It identified the rejected value and listed
the valid built-in themes. That prompted a lookup of the public `pptx-build`
schema, which exposed the correct `brand` property. There were zero useless
errors. The failed caller-side `jq` projection at step 13 was not a CLI error;
the command itself succeeded and raw readback worked without changing argv.

## Independent schema proof

After strict validation, each of the five packages was also passed directly to:

```text
$HOME/dotnet/dotnet $REPO/tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll <package>
```

All five validator invocations exited 0 with `OPENXML-VALIDATOR: 0 errors
(clean)`. LibreOffice rendering proves local renderability; it is not a claim
of desktop Microsoft Office compatibility.
