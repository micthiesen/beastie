#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: scripts/assemble-moonshine-notices.sh MOONSHINE_CHECKOUT OUTPUT" >&2
  exit 2
fi

moonshine="$1"
output="$2"
licenses=(
  "core/third-party/doctest/LICENSE.txt"
  "core/third-party/Eigen/COPYING.MPL2"
  "core/third-party/kaldi-native-fbank/LICENSE"
  "core/third-party/kissfft/COPYING"
  "core/third-party/nlohmann/LICENSE.MIT"
  "core/third-party/onnxruntime/LICENSE.txt"
  "core/third-party/utf-8/LICENSE.txt"
  "core/third-party/utf8proc/LICENSE.md"
)

for license in "${licenses[@]}"; do
  if [ ! -f "$moonshine/$license" ]; then
    echo "error: missing $moonshine/$license" >&2
    exit 1
  fi
done

mkdir -p "$(dirname "$output")"
{
  echo "Moonshine Voice third-party notices"
  echo
  echo "Generated from the exact vendored license files at Moonshine Voice v0.1.2."
  for license in "${licenses[@]}"; do
    echo
    echo "============================================================================="
    echo "$license"
    echo "============================================================================="
    cat "$moonshine/$license"
  done
} > "$output"

echo "Wrote $output"
