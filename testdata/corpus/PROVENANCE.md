# Independent producer corpus

These six packages contain original, self-authored test content distributed under
this repository's license. They were created on 2026-09-05 using the adjacent
`generate.py`, without using ooxml-cli as the package serializer. Python packages
and LibreOffice are needed only for deliberate regeneration; CI reads the committed
bytes and needs no network, Python packages, LibreOffice, or credentials.

| File | Actual final producer | Content | Office2019 SDK input baseline |
| --- | --- | --- | --- |
| `python-pptx/review.pptx` | python-pptx 1.0.2 | Two slides, placeholders, Unicode, table | Clean |
| `python-docx/report.docx` | python-docx 1.2.0 | Headings, bold runs, list, table, header | Clean |
| `xlsxwriter/sales.xlsx` | XlsxWriter 3.2.5 | Shared strings, table, formula, chart, frozen row | Clean |
| `libreoffice/review.pptx` | LibreOffice Impress 26.2.5.2 | Actual headless re-export of the first file | Clean |
| `libreoffice/report.docx` | LibreOffice Writer 26.2.5.2 | Actual headless re-export of the second file | Invalid `start` justification; ooxml-8bq |
| `libreoffice/sales.xlsx` | LibreOffice Calc 26.2.5.2 | Actual headless re-export of the third file | Two chart-style errors; ooxml-ojc |

This is four independent serializers and three Office families, with two files
per family. It does not include Microsoft Office or Google exports. No public
sample with established redistribution provenance or credential-free Google
export was used. In particular, the older `testdata/pptx/producers` simulated
Google/PowerPoint files are not counted as those producers.

Regenerate in an isolated virtual environment with the versions in the generator,
then review binary/semantic changes and update the SHA256 list deliberately.
LibreOffice exports may contain creation metadata; committed bytes, not generated
byte identity, define this corpus. No regeneration runs in tests.

The Rust test runs outline, check, design-check, a representative mutation,
strict validation and semantic readback for every file. It verifies fixture hashes,
covers every corpus file, and compares the decompressed bytes of every unrelated
package part before and after mutation. SDK proof is checked
when available and required when `OOXML_REQUIRE_OPENXML_SDK=1`. Clean inputs must
produce clean outputs. Upstream-invalid inputs remain classified as invalid;
their unrelated schema problems are not silently repaired by the mutation.
Each process has a 30-second timeout and the six-case suite has a 180-second
budget. Tool availability and native paths are never golden data.

Findings:

- ooxml-yfv: table text contrast used the slide background. The product fix pairs
  each cell's text with its own direct fill and excludes border colors. A minimal
  two-cell XML regression retains both readable and unreadable cases. The real
  export still has a legitimate contrast finding (~4:1), now with the correct
  blue background rather than a false 1:1 white background.
- ooxml-8bq: LibreOffice's DOCX has `w:jc` and `w:lvlJc` values of `start` rejected
  by the configured SDK. The input file remains the real export.
- ooxml-ojc: LibreOffice's XLSX chart style has children in a leaf element and a
  chart-style namespace color where DrawingML is expected. The real input is
  retained; this is not an ooxml cell-write regression.

Schema and LibreOffice export evidence do not establish desktop Office compatibility.
