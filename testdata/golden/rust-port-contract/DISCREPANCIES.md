# Known Rust Port Contract Divergences

Every mismatch against the Go oracle must be fixed or classified here before it
is accepted in a Rust milestone.

## DISC-001: `xlsx ranges set` Partial Direct CLI Surface

- Review date: 2026-06-19
- Status: open, not accepted as final parity
- Affected surface: `ooxml xlsx ranges set`
- Affected tests: `xlsx_ranges_set_matches_go_oracle_and_saved_output`,
  `xlsx_ranges_set_delimited_and_stdin_match_go_oracle`,
  `serve_op_supports_xlsx_ranges_set`
- Impact: Rust currently proves parity for the direct CLI JSON-matrix path with
  `--out`, `--in-place`, `--backup`, and `--dry-run`, including CSV/TSV input,
  `--values-file -` stdin, saved-output readback, formula-cell readback, null
  skipping, formula-overwrite rejection, merged-cell rejection, untouched
  shared-string/style/formula-cache cell XML preservation, generated
  readback-command shape, and the serve/MCP operation route. The Go oracle also
  supports full formula recalculation and calc-chain invalidation around formula
  writes/overwrites. Those formula subfeatures remain open work before this
  command can be counted as full command parity.
- Current handling: Rust advertises the command as `opCompatible: true` for the
  proven direct and session operation routes. Do not use this entry to claim
  full parity for `xlsx ranges set` until formula recalculation/calc-chain
  behavior is proven or separately classified.
