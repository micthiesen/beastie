#!/usr/bin/env bash
set -euo pipefail
root=${1:?usage: validate-layout.sh <dist/linux>}
for path in beastie beastie-ai-worker beastie-stt runtime/llama-server runtime/LICENSE runtime/stt/beastie-moonshine-engine runtime/stt/LICENSE runtime/stt/THIRD_PARTY_NOTICES assets/manifest.toml models/manifest.toml models/moonshine-tiny-streaming-en/LICENSE models/moonshine-tiny-streaming-en/README.md package-manifest.json; do
  [[ -f "$root/$path" ]] || { echo "verified package is missing $path" >&2; exit 1; }
done
grep -q '"network": false' "$root/package-manifest.json" || { echo 'package must declare network=false' >&2; exit 1; }
grep -q '"platform": "linux"' "$root/package-manifest.json" || { echo 'package platform is not linux' >&2; exit 1; }
echo "Linux package layout verified: $root"
