#!/usr/bin/env bash
set -euo pipefail
root=${1:?usage: validate-layout.sh <dist/linux>}
for path in beastie beastie-ai-worker runtime/llama-server runtime/LICENSE assets/manifest.toml models/manifest.toml package-manifest.json; do
  [[ -f "$root/$path" ]] || { echo "verified package is missing $path" >&2; exit 1; }
done
grep -q '"network": false' "$root/package-manifest.json" || { echo 'package must declare network=false' >&2; exit 1; }
grep -q '"platform": "linux"' "$root/package-manifest.json" || { echo 'package platform is not linux' >&2; exit 1; }
echo "Linux package layout verified: $root"
