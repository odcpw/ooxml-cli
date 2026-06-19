# Known Rust Port Contract Divergences

Every mismatch against the Go oracle must be fixed or classified here before it
is accepted in a Rust milestone.

## DISC-001: `xlsx ranges set` Partial Direct CLI Surface

- Review date: 2026-06-19
- Status: open, not accepted as final parity
- Affected surface: `ooxml xlsx ranges set`
- Affected tests: `xlsx_ranges_set_matches_go_oracle_and_saved_output`,
  `xlsx_ranges_set_delimited_and_stdin_match_go_oracle`
- Impact: Rust currently proves parity for the direct CLI JSON-matrix path with
  `--out`, `--in-place`, `--backup`, and `--dry-run`, including CSV/TSV input,
  `--values-file -` stdin, saved-output readback, formula-cell readback, null
  skipping, formula-overwrite rejection, merged-cell rejection, untouched
  shared-string/style/formula-cache cell XML preservation, and generated
  readback-command shape. The Go oracle also supports full formula
  recalculation and calc-chain invalidation around formula writes/overwrites,
  and the serve/MCP operation route. Those subfeatures remain open work before
  this command can be counted as full command parity.
- Current handling: Rust advertises the command as `opCompatible: false` and the
  status doc calls out direct CLI support only. Do not use this entry to claim
  full parity for `xlsx ranges set`.

## DISC-002: `xlsx ranges set-format` Partial Direct CLI Surface

- Review date: 2026-06-19
- Status: open, not accepted as final parity
- Affected surface: `ooxml xlsx ranges set-format`
- Affected tests: `xlsx_ranges_set_format_matches_go_oracle_and_saved_output`,
  `xlsx_ranges_set_format_range_edges_match_go_oracle`,
  `capabilities_advertise_supported_web_agent_surface`,
  `rust_capability_inventory_is_go_oracle_subset`
- Impact: Rust currently proves parity for the direct CLI number-format mutation
  path with `--out` and `--dry-run`, including built-in and custom style
  creation, style readback on blank cells, saved-output readback via the Go
  oracle, malformed range rejection, reversed range handling, generated
  readback-command shape, and dry-run non-mutation. The Go oracle also exposes
  the broader command surface through agent/server operation routing. That
  serve/MCP operation route remains open work before this command can be counted
  as full command parity.
- Current handling: Rust advertises the command as `opCompatible: false` and the
  status doc calls out direct CLI support only. Do not use this entry to claim
  full parity for `xlsx ranges set-format`.
