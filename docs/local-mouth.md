# Local mouth status

Stage 4 established the replaceable offline dialogue and speech boundaries. The deterministic
fixture backend remains the default for development and `cargo xtask verify`. Real model weights
remain ignored, local, and described by hashes in `models/manifest.toml`.

## Provisional dialogue model

Qwen3.5 0.8B Q4 is the provisional dialogue candidate. It was selected with the version 1 corpus
in `evals/dialogue/corpus.json`, not a single attractive response:

| Candidate | Passed | Protocol valid | Grounded | Fallbacks | Permitted sharpness | Prohibited content | Median |
|---|---:|---:|---:|---:|---:|---:|---:|
| Qwen3.5 0.8B Q4 | 11/12 | 12/12 | 11/12 | 1 | 3/3 | 3/3 | 889 ms |
| Qwen3 0.6B Q8 | 5/12 | 12/12 | 11/12 | 6 | 0/3 | 3/3 | 1,790 ms |

The Qwen3.5 miss was a bell-memory request that exhausted both attempts and used the authored
fallback. Known food and toy nouns are now anchored from authoritative projected memory facts.
Rust selects the response lane, allowed gesture, and recalled-memory ID; the model only phrases the
short `say` field. A final deterministic filter rejects near-verbatim player echoes, a narrow
prohibited lexicon, invalid IDs and gestures, empty or control-bearing text, replies above 512 UTF-8
bytes, and replies over the request's word limit.

The measurements above used `llama.cpp` 10310 (`cb26014d9`) on the M5 Max through Metal. Every
case launched a new `llama-cli`, so the median includes process startup and model loading. The OS
filesystem cache was warm after the first case. This is neither a first-boot disk-cold measurement
nor evidence of a production warm runtime.

Reproduce a real-model run after placing an ignored GGUF at the recorded path:

```sh
cargo build --package beastie-ai-worker
cargo xtask dialogue eval \
  --worker target/debug/beastie-ai-worker \
  --model models/Qwen3.5-0.8B-Q4_0.gguf \
  --timeout-ms 30000 \
  --label qwen35-local
```

Add `--cpu-only` to exercise the portable fallback. On this Mac, genuine CPU-only llama.cpp needs
all of `--device none --no-op-offload -ngl 0`; the worker supplies them together.

## Process boundary

The game owns one long-lived `DialogueManager` thread and reuses one Beastie worker process across
requests. Only one talk may be outstanding. The outer JSONL transport has bounded lines, validates
both sides of the protocol, has a watchdog covering both model attempts plus grace, returns authored
fallback on failure, and restarts a failed worker on the next request.

Worker output is also bounded at the channel boundary, unsolicited replies force a restart, and
shutdown interrupts generation before joining the manager thread. On macOS and Linux, each worker
uses its own process group so a timeout or quit reaps descendant inference processes as well. The
equivalent Windows Job Object containment remains a Stage 5 release requirement.

Inside that persistent worker, the provisional llama.cpp adapter still launches a new `llama-cli`
for each attempt. It bounds time and stdout, disables reasoning, retries once, and falls back safely,
but reloads the GGUF for every utterance. llama.cpp 10310 interactive mode did not provide a safe
framed machine protocol, and its JSON-schema sampler failed to initialize in the tested build. A
persistent native runtime or separately framed long-lived service remains Stage 5 work.

## Native speech spike

The optional Kitten nano v0.8 int8 adapter is native Rust and has no Python runtime dependency. It
loads the model once per TTS process, accepts bounded versioned JSONL, synthesizes 24 kHz mono PCM,
and reuses a deterministic content-addressed WAV cache. Repeated uncached syntheses are not assumed
to be byte-identical. The non-default `experimental-gpl-tts` feature and `beastie-tts` smoke binary
keep this path out of ordinary builds. Exact model and runtime hashes are in
`models/kitten-nano-en-v0_8-int8.toml`.

On the M5 Max native smoke, model load took 174 to 206 ms. A 4.08-second line generated in 857 to
869 ms, RTF 0.21, at roughly 171 MB RSS. Repeated identical input reused the hashed WAV cache. The
command and artifact verification steps are in `crates/beastie-ai-worker/TTS.md`.

The post-review smoke generated a fresh 1.52-second, 72,888-byte mono PCM16 WAV at 24 kHz. The game
boundary accepts only the pinned cache-key shape, opens the final entry without following a symlink
on Unix, copies a bounded file from the same handle, and validates the exact WAV contract before
playback. New speech invalidates older queued or in-flight completions.

Feature-gated gameplay keeps one TTS worker alive, submits versioned JSONL after dialogue text is
already visible, validates the returned cache key and scoped WAV, then plays speech asynchronously
through a dedicated replaceable sink. It remains off by default. The full development path is:

```sh
cargo xtask dev \
  --tts-model-dir /tmp/beastie-tts-smoke/kitten-nano-en-v0_8-int8 \
  --tts-cache-dir /tmp/beastie-tts-smoke/cache
```

This path is not releasable: sherpa-onnx 1.13.5 statically embeds GPLv3 espeak-ng. Do not enable
`experimental-gpl-tts` in an MIT-only release, package, or default CI build. Stage 5 must choose a
license-compatible native runtime or make an explicit product licensing decision before this path
can become a default or distributable full-local loop.

The reviewed macOS fixture path was launched through Alacritty and produced all three direct
320x180 logical-framebuffer captures in `target/captures/stage4-review`, including restored-memory
speech. It exited without leaving a Beastie, llama.cpp, or TTS process behind.
