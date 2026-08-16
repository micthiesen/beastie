# Beastie Moonshine engine

This small native sidecar keeps Moonshine Voice loaded while Beastie's bounded Rust STT worker
owns validation, timeouts, restart, and fallback policy. It targets Moonshine Voice `v0.1.2`
Tiny Streaming English (architecture `2`). The model directory contains exactly the seven files
returned by Moonshine's dependency catalog. The optional word-alignment model is deliberately not
shipped: it added tens of seconds to a short utterance in the native bake-off. The adapter instead
maps VAD plus a completed streaming endpoint to a conservative usability score. That score is not
presented as a probability that every word is correct.

The build consumes Moonshine's published C++ header and native library:

```sh
cmake -S native/moonshine-stt -B target/moonshine-engine \
  -DMOONSHINE_ROOT="$MOONSHINE_ROOT" -DCMAKE_BUILD_TYPE=Release
cmake --build target/moonshine-engine --config Release
```

On the development Mac, the repeatable host preflight is:

```sh
scripts/build-moonshine-stt.sh
cargo build -p beastie-ai-worker --bin beastie-stt
cargo xtask stt eval \
  --worker target/debug/beastie-stt \
  --model-dir target/stt-bakeoff/moonshine-runtime/tiny-streaming-en \
  --moonshine-engine target/debug/beastie-moonshine-engine \
  --label moonshine-host
```

The build helper exits successfully with an explicit skip message when the optional SDK is absent,
so it is safe in a fresh model-free checkout.

The process accepts the same compact `RecognitionRequest` JSONL as `beastie-stt` and emits one
validated-shape `RecognitionReply` per input. Audio remains in the worker-owned content-addressed
directory. One second of silence is appended internally so a push-to-talk release reliably closes
the streaming endpoint without modifying captured audio.
