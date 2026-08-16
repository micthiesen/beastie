#!/usr/bin/env bash
set -euo pipefail
root=${1:?usage: validate-layout.sh <dist/linux>}
for path in \
  beastie beastie-ai-worker beastie-tts beastie-stt \
  runtime/llama-server runtime/LICENSE runtime/espeak-ng runtime/espeak-ng-COPYING \
  runtime/espeak-ng-1.52.0.tar.gz \
  assets/manifest.toml models/manifest.toml models/LICENSE models/README.md \
  models/Qwen3.5-0.8B-Q4_0.gguf \
  models/parakeet-tdt-0.6b-v3-int8/config.json \
  models/parakeet-tdt-0.6b-v3-int8/decoder_joint-model.int8.onnx \
  models/parakeet-tdt-0.6b-v3-int8/encoder-model.int8.onnx \
  models/parakeet-tdt-0.6b-v3-int8/nemo128.onnx \
  models/parakeet-tdt-0.6b-v3-int8/vocab.txt \
  models/parakeet-tdt-0.6b-v3-int8/LICENSE \
  models/parakeet-tdt-0.6b-v3-int8/README.md \
  THIRD_PARTY_NOTICES LICENSE package-manifest.json
do
  [[ -f "$root/$path" ]] || { echo "verified package is missing $path" >&2; exit 1; }
done
[[ -d "$root/runtime/espeak-ng-data" ]] || {
  echo 'verified package is missing runtime/espeak-ng-data' >&2
  exit 1
}
grep -q '"network": false' "$root/package-manifest.json" || { echo 'package must declare network=false' >&2; exit 1; }
grep -q '"platform": "linux"' "$root/package-manifest.json" || { echo 'package platform is not linux' >&2; exit 1; }
grep -q '"release_complete": true' "$root/package-manifest.json" || { echo 'package is not release-complete' >&2; exit 1; }
echo "Linux package layout verified: $root"
