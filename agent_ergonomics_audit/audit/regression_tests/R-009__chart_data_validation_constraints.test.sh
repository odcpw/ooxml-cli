#!/usr/bin/env bash
set -euo pipefail

BIN="${OOXML_BIN:-target/debug/ooxml}"

caps="$("$BIN" --json capabilities)"

jq -e '
  (.commands[] | select(.path == "ooxml xlsx data-validations create") | .flagConstraints)
  | .modeFlag == "--type"
    and any(.modes[]; .value == "list" and .oneOf == ["--list-values", "--list-range"])
    and any(.modes[]; .value == "textLength" and any(.aliases[]; . == "text-length"))
    and any(.rules[]; contains("between and notBetween require --formula2"))
' <<<"$caps" >/dev/null

jq -e '
  (.commands[] | select(.path == "ooxml xlsx charts create") | .flagConstraints)
  | .modeFlag == "--type"
    and any(.sourceModes[]; .name == "range" and .required == ["--sheet", "--range"])
    and any(.sourceModes[]; .name == "table" and .required == ["--table"] and .conflictsWith == ["--range"])
' <<<"$caps" >/dev/null

jq -e '
  (.commands[] | select(.path == "ooxml pptx charts create") | .flagConstraints)
  | .modeFlag == "--type"
    and any(.sourceModes[]; .name == "inline-json" and any(.conflictsWith[]; . == "--source-file"))
    and any(.sourceModes[]; .name == "external-xlsx" and .required == ["--source-file", "--source-range"])
' <<<"$caps" >/dev/null
