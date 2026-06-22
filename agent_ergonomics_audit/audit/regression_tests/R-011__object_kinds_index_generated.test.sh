#!/usr/bin/env bash
set -euo pipefail

BIN="${OOXML_BIN:-target/debug/ooxml}"

jq_filter='
  . as $caps
  | reduce ($caps.objectKinds[]) as $kind ({};
      .[$kind] = ([ $caps.commands[]
        | select(((.targetObjectKinds // []) | index($kind)) != null)
        | .path
      ] | sort)
    ) as $generated
  | (($caps.objectKindsIndex | keys | sort) == ($caps.objectKinds | sort))
    and all($caps.objectKinds[];
      (($caps.objectKindsIndex[.] // []) | sort) == (($generated[.] // []) | sort)
    )
'

all_caps="$("$BIN" --json capabilities)"
jq -e "$jq_filter" <<<"$all_caps" >/dev/null

package_caps="$("$BIN" --json capabilities --for package)"
jq -e "$jq_filter" <<<"$package_caps" >/dev/null
