#!/usr/bin/env bash
set -euo pipefail

BIN="${OOXML_BIN:-target/debug/ooxml}"

caps="$("$BIN" --json capabilities)"
guide="$("$BIN" --json robot-docs guide)"

jq -e '
  any(.filterAliases[]; .alias == "dv" and .canonical == "data-validation")
  and any(.filterAliases[]; .alias == "cf" and .canonical == "conditional-format")
' <<<"$caps" >/dev/null

jq -e '
  any(.sections[]; .name == "Discovery"
    and any(.filterAliases[]; . == "dv -> data-validation")
    and any(.filterAliases[]; . == "cf -> conditional-format"))
' <<<"$guide" >/dev/null

dv_help="$("$BIN" xlsx dv --help)"
grep -q "data-validation" <<<"$dv_help"
grep -q "dv" <<<"$dv_help"

cf_help="$("$BIN" xlsx cf --help)"
grep -q "conditional-format" <<<"$cf_help"
grep -q "conditional-formatting" <<<"$cf_help"
grep -q "cf" <<<"$cf_help"
