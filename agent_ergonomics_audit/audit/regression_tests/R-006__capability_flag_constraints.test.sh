#!/usr/bin/env bash
set -euo pipefail

BIN="${OOXML_BIN:-target/debug/ooxml}"

caps="$("$BIN" --json capabilities)"
jq -e '
  (.commands[] | select(.path == "ooxml xlsx conditional-formats add") | .flagConstraints)
  | .modeFlag == "--type"
    and .defaultMode == "expression"
    and any(.modes[]; .value == "color-scale" and .repeat["--cfvo"] == "2 or 3")
    and any(.modes[]; .value == "icon-set" and any(.forbidden[]; . == "--color"))
' <<<"$caps" >/dev/null

jq -e '
  (.commands[] | select(.path == "ooxml vba create") | .flagConstraints)
  | any(.modes[]; .name == "pure" and any(.conflictsWith[]; . == "--office-create-script"))
    and any(.modes[]; .name == "legacy-office-com" and any(.allowedFlags[]; . == "--office-create-script"))
    and any(.rules[]; contains("--pure cannot"))
' <<<"$caps" >/dev/null
