#!/usr/bin/env bash
set -euo pipefail

BIN="${OOXML_BIN:-target/debug/ooxml}"

cf_help="$("$BIN" xlsx cf --help)"
grep -F 'Commands for conditional-formatting expression rules.' <<<"$cf_help" >/dev/null
grep -F 'conditional-formats, conditional-format, conditional-formatting, cf' <<<"$cf_help" >/dev/null

dv_help="$("$BIN" xlsx dv --help)"
grep -F 'Commands for data-validation rules.' <<<"$dv_help" >/dev/null
grep -F 'data-validations, data-validation, dv' <<<"$dv_help" >/dev/null
