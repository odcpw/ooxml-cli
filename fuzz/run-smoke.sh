#!/usr/bin/env bash
set -euo pipefail

seconds="${FUZZ_SECONDS_PER_TARGET:-5}"
targets=(build-spec markdown brand refs image)

for target in "${targets[@]}"; do
  case "$target" in
    build-spec) max_len=$((512 * 1024)); dictionary="fuzz/dictionaries/json.dict" ;;
    markdown) max_len=$((256 * 1024)); dictionary="fuzz/dictionaries/markdown.dict" ;;
    brand|refs) max_len=$((256 * 1024)); dictionary="fuzz/dictionaries/json.dict" ;;
    image) max_len=$((4 * 1024 * 1024)); dictionary="fuzz/dictionaries/image.dict" ;;
  esac
  corpus="fuzz/corpus/${target//-/_}"
  mkdir -p "fuzz/artifacts/$target"
  cargo +nightly fuzz run "$target" "$corpus" -- \
    -max_total_time="$seconds" \
    -timeout=5 \
    -rss_limit_mb=1024 \
    -max_len="$max_len" \
    -dict="$dictionary" \
    -artifact_prefix="fuzz/artifacts/$target/"
done
