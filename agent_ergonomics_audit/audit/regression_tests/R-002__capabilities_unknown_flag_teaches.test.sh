#!/usr/bin/env bash
set -euo pipefail

BIN="${OOXML_BIN:-target/debug/ooxml}"

set +e
out="$("$BIN" capabilities --fr slides 2>&1)"
code=$?
set -e

[[ "$code" -eq 2 ]]
grep -F 'unknown flag: --fr; did you mean --for?' <<<"$out" >/dev/null
grep -F 'Try: ooxml --json capabilities --for <filter>' <<<"$out" >/dev/null
