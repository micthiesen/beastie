#!/usr/bin/env bash
set -euo pipefail

# Run from the repository root. Extra arguments go to `cargo xtask dev`.
if [[ ! -f crates/xtask/Cargo.toml || ! -f models/manifest.toml ]]; then
  echo 'error: run ./scripts/play.sh from the Beastie repository root.' >&2
  exit 1
fi

export BEASTIE_AI_BACKEND=llama-server
export BEASTIE_AI_MODEL="${BEASTIE_AI_MODEL:-$PWD/models/Qwen3.5-0.8B-Q4_0.gguf}"
export BEASTIE_LLAMA_SERVER="${BEASTIE_LLAMA_SERVER:-$PWD/target/runtime/llama-cpp/llama-server}"
export BEASTIE_AI_THREADS="${BEASTIE_AI_THREADS:-4}"
# Allow the first launch to compile Metal shaders before inference begins.
export BEASTIE_AI_TIMEOUT_MS="${BEASTIE_AI_TIMEOUT_MS:-120000}"
export BEASTIE_TTS_BACKEND=espeak
stt_model_dir="${BEASTIE_STT_MODEL_DIR:-$PWD/target/stt/parakeet-tdt-0.6b-v3-int8}"
tts_cache_dir="${BEASTIE_TTS_CACHE_DIR:-$PWD/target/tts-cache}"
espeak="${BEASTIE_ESPEAK_NG:-espeak-ng}"

missing=0
require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'Missing executable: %s\n  %s\n' "$1" "$2" >&2
    missing=1
  fi
}

require_file() {
  if [[ ! -r "$1" || ! -s "$1" ]]; then
    printf 'Missing, empty, or unreadable file: %s\n  %s\n' "$1" "$2" >&2
    missing=1
  fi
}

require_tool cargo 'Install Rust with Cargo, then reopen your terminal.'
require_tool rustc 'Install Rust with Cargo, then reopen your terminal.'
require_tool "$BEASTIE_LLAMA_SERVER" \
  'Install the llama.cpp runtime listed in models/manifest.toml under target/runtime/llama-cpp (including its libraries), or set BEASTIE_LLAMA_SERVER.'
require_tool "$espeak" \
  'Install eSpeak NG (macOS: brew install espeak-ng; Ubuntu: sudo apt install espeak-ng), or set BEASTIE_ESPEAK_NG.'
require_file "$BEASTIE_AI_MODEL" \
  'Download Qwen3.5-0.8B-Q4_0.gguf using the source, revision and checksum in models/manifest.toml, or set BEASTIE_AI_MODEL.'
for component in config.json decoder_joint-model.int8.onnx encoder-model.int8.onnx nemo128.onnx vocab.txt; do
  require_file "$stt_model_dir/$component" \
    'Run cargo xtask stt setup to install Parakeet, or set BEASTIE_STT_MODEL_DIR to a complete model directory.'
done

if (( missing )); then
  echo 'Cannot start with real dialogue and speech until these dependencies are installed.' >&2
  exit 1
fi

if [[ "${1:-}" == --check ]]; then
  if (( $# != 1 )); then
    echo 'error: use --check on its own, or pass launch arguments without --check.' >&2
    exit 1
  fi
  echo 'Local model files and executables are present. Runtime loading and microphone access are checked when playing.'
  exit 0
fi

exec cargo xtask dev \
  --stt-backend parakeet \
  --stt-model-dir "$stt_model_dir" \
  --tts-cache-dir "$tts_cache_dir" \
  --tts-espeak "$espeak" \
  "$@"
