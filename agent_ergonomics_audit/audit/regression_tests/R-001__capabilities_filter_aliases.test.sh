#!/usr/bin/env bash
set -euo pipefail

BIN="${OOXML_BIN:-target/debug/ooxml}"

slides="$("$BIN" --json capabilities --for slides)"
jq -e '
  .filter.requested == "slides"
  and .filter.normalized == "slide"
  and .filter.matchedCommands > 0
  and any(.filterAliases[]; .alias == "slides" and .canonical == "slide")
  and any(.commands[]; .path == "ooxml pptx slides list")
' <<<"$slides" >/dev/null

conditional_formats="$("$BIN" --json capabilities --for conditional-formats)"
jq -e '
  .filter.requested == "conditional-formats"
  and .filter.normalized == "conditional-format"
  and .filter.matchedCommands == 5
  and any(.filterAliases[]; .alias == "cf" and .canonical == "conditional-format")
  and any(.commands[]; .path == "ooxml xlsx conditional-formats add")
' <<<"$conditional_formats" >/dev/null
