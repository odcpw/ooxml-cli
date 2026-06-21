#!/usr/bin/env bash
set -euo pipefail

BIN="${OOXML_BIN:-target/debug/ooxml}"

set +e
health="$("$BIN" --json doctor health --only openxml-sdk-validator)"
code=$?
set -e

if [[ "$code" != "0" && "$code" != "1" ]]; then
  echo "unexpected doctor health exit code: $code" >&2
  exit 1
fi

jq -e --argjson code "$code" '
  .tool == "ooxml"
  and .summary.total == 1
  and .exitCode == $code
  and (.healthy == ($code == 0))
' <<<"$health" >/dev/null
