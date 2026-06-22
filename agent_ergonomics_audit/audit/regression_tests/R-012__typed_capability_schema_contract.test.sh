#!/usr/bin/env bash
set -euo pipefail

BIN="${OOXML_BIN:-target/debug/ooxml}"

caps="$("$BIN" --json capabilities)"
strict_caps="$("$BIN" --json capabilities --strict)"
empty_caps="$("$BIN" --json capabilities --for slidez)"
triage="$("$BIN" --json agent-triage)"

jq -e '
  (has("filter") | not)
  and all(.globalFlags[]; (.default | type) == "string")
  and all(.filterAliases[]; ((keys | sort) == ["alias", "canonical"] and (.alias | type) == "string" and (.canonical | type) == "string"))
  and all(.commands[];
    ((keys - ["flagConstraints", "localFlags", "opCompatible", "opIneligibleReason", "path", "short", "targetObjectKinds", "use"]) | length) == 0
    and (.path | type) == "string"
    and (.use | type) == "string"
    and (.short | type) == "string"
    and (.targetObjectKinds | type) == "array"
    and (.localFlags | type) == "array"
    and (.opCompatible | type) == "boolean"
    and ((has("opIneligibleReason") | not) or (.opIneligibleReason | type) == "string")
    and ((has("flagConstraints") | not) or (.flagConstraints | type) == "object")
    and all(.localFlags[];
      ((keys | sort) == ["argName", "description", "name", "type"])
      and (.argName | type) == "string"
      and (.description | type) == "string"
      and (.name | type) == "string"
      and (.type | type) == "string"
    )
  )
' <<<"$caps" >/dev/null

caps_count="$(jq '.commands | length' <<<"$caps")"
strict_caps_count="$(jq '.commands | length' <<<"$strict_caps")"
test "$caps_count" = "$strict_caps_count"

jq -e '
  . as $root
  |
  (.commands | length) == 0
  and (.filter.requested == "slidez")
  and (.filter.normalized == "slidez")
  and ((.filter.suggestions | length) > 0)
  and all($root.objectKinds[]; (. as $kind | ($root.objectKindsIndex[$kind] | type) == "array" and ($root.objectKindsIndex[$kind] | length) == 0))
' <<<"$empty_caps" >/dev/null

jq -e '
  all(.discovery.filterAliases[]; type == "string")
' <<<"$triage" >/dev/null
