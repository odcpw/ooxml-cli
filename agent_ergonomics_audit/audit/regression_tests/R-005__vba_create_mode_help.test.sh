#!/usr/bin/env bash
set -euo pipefail

BIN="${OOXML_BIN:-target/debug/ooxml}"

help_text="$("$BIN" vba create --help)"
grep -F 'Mode guide:' <<<"$help_text" >/dev/null
grep -F 'Preferred pure Rust mode:' <<<"$help_text" >/dev/null
grep -F 'Legacy Office-COM mode:' <<<"$help_text" >/dev/null
grep -F 'Do not combine --pure with legacy Office-COM flags.' <<<"$help_text" >/dev/null
