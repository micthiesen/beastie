#!/usr/bin/env bash
set -euo pipefail

repo="$(git rev-parse --show-toplevel)"
sdk="${MOONSHINE_ROOT:-$repo/target/stt-bakeoff/moonshine-runtime/lib}"
build="$repo/target/stt-bakeoff/engine-build"
destination="$repo/target/debug/beastie-moonshine-engine"

if [ ! -f "$sdk/include/moonshine-cpp.h" ] || [ ! -f "$sdk/lib/libmoonshine.a" ]; then
  echo "Moonshine SDK not present at $sdk; skipping optional native engine build."
  echo "Set MOONSHINE_ROOT to an extracted Moonshine Voice v0.1.2 native SDK to enable it."
  exit 0
fi

cmake -S "$repo/native/moonshine-stt" -B "$build" \
  -DMOONSHINE_ROOT="$sdk" -DCMAKE_BUILD_TYPE=Release
cmake --build "$build" --config Release

source_binary="$build/beastie-moonshine-engine"
if [ ! -x "$source_binary" ] && [ -x "$build/Release/beastie-moonshine-engine.exe" ]; then
  source_binary="$build/Release/beastie-moonshine-engine.exe"
  destination="$repo/target/debug/beastie-moonshine-engine.exe"
fi
if [ ! -x "$source_binary" ]; then
  echo "error: native engine build completed without the expected executable" >&2
  exit 1
fi

cp "$source_binary" "$destination"
echo "Built $destination"
