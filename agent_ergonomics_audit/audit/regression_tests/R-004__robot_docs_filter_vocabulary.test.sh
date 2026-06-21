#!/usr/bin/env bash
set -euo pipefail

BIN="${OOXML_BIN:-target/debug/ooxml}"

guide="$("$BIN" --json robot-docs guide)"
jq -e '
  any(.sections[]; .name == "Discovery"
    and any(.filters[]; . == "conditional-format")
    and any(.filters[]; . == "module")
    and any(.filterAliases[]; . == "conditional-formats -> conditional-format")
    and any(.filterAliases[]; . == "modules -> module"))
' <<<"$guide" >/dev/null
