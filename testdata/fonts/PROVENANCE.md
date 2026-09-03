# Font metric data provenance

The TSV files in this directory contain numeric measurements only. No font
program, glyph outline, hinting program, or other font-file content is copied
into this repository.

`metrics-v1.tsv` was extracted on 2026-09-03 from the horizontal advance,
`unitsPerEm`, and horizontal-header tables in these locally installed fonts,
then normalized to 1,000 units per em:

| Source file | SHA-256 |
|---|---|
| LiberationSans-Regular.ttf | `baccc64becc3eb7d104b7c84d99f5314a0a1f896e2b3ea6c2f22fc08d2003bee` |
| LiberationSans-Bold.ttf | `769673c4355020b1e28a14c366a152da410ab6b16239fe883ebc35b73624835b` |
| LiberationSerif-Regular.ttf | `86b9ea1c2f41bed9d7c09ccad4abc2894b33df5de60e5bbbece5d48610911870` |
| LiberationSerif-Bold.ttf | `28f2d4300ee366d1ff9ca95df967a27e77987c87857fad0d9c85034405aae39d` |
| NotoSans-Regular.ttf | `478c558ea716033cd60c03438f628dfa75694dcf6b5f6d505a2f05fd2b4f3823` |
| NotoSans-Bold.ttf | `1df075a380fc7cb898acf64c1f7b3b4dd780de3caa860178bf929de35817a913` |

Each advance vector has 95 entries in Unicode order, U+0020 through U+007E.
The average is the arithmetic mean of A-Z, a-z, and 0-9. Line height is the
font horizontal-header ascent minus descent plus line gap. Regular and bold
are independent measurements.

`families-v1.tsv` makes substitution explicit and reviewable. Proprietary
theme fonts are not installed or redistributed on the Linux calibration host,
so they deliberately select the same open-font profiles LibreOffice uses for
repeatable layout estimates. The selected source profile is returned by the
debug API; callers can therefore distinguish a native metric from a calibrated
substitute. Unknown families use the Noto Sans fallback profile.
