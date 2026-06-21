#!/usr/bin/env bash
set -euo pipefail

BIN="${OOXML_BIN:-target/debug/ooxml}"

triage="$("$BIN" agent-triage)"
jq -e '
  .contractVersion == "ooxml-cli.agent-triage.v1"
  and .readOnly == true
  and (.dataHash | startswith("sha256:"))
  and any(.quickRef.topCommands[]; . == "ooxml --json capabilities --for <filter>")
  and any(.commands[]; .action == "doctor-health" and .command == "ooxml --json doctor health")
  and (.health.summary | type == "object")
' <<<"$triage" >/dev/null

alias_triage="$("$BIN" agent triage)"
jq -e '.contractVersion == "ooxml-cli.agent-triage.v1"' <<<"$alias_triage" >/dev/null
