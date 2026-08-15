# Tooling preflight

Local development tooling was first smoke-tested on 2026-08-15 before implementation began. These
results proved that each external content path could run on the development Mac. Later V1
measurements and selected release paths are summarized here and detailed in
[local-mouth.md](local-mouth.md).

## Test host

- Apple M5 Max, 18 CPU cores, 128 GB memory
- macOS 26.5.2 (25F84), arm64
- `llama.cpp` 10310 (`cb26014d9`), built with AppleClang 21
- `uv` 0.11.28 for disposable Python smoke environments only

Shipping remains a native, offline application. Python is not a runtime dependency.

## Local language models

The following official GGUF repositories were exercised through `llama.cpp` with a deliberately
small, crude Beastie prompt. Generation speed is from this host and is not a low-end target:

| Candidate | Artifact | Generation | First smoke result |
|---|---|---:|---|
| Qwen3 0.6B | `Qwen/Qwen3-0.6B-GGUF`, Q8 | 218.4 tok/s | Fluent, but contradicted a supplied berry preference |
| Qwen3.5 0.8B | `ggml-org/Qwen3.5-0.8B-GGUF`, BF16 | 12.9 tok/s | Fluent, but contradicted the same fact |
| LFM2 700M | `LiquidAI/LFM2-700M-GGUF`, Q4_K_M | 206 tok/s | Returned no usable text on the first probe |

The checked-in version 1 Beastie corpus measures factual grounding, schema compliance, latency,
profanity/insult willingness, prohibited-output escape, and assistant-like filler. The final
CPU-only Qwen3.5 0.8B Q4 run passed 18/18 with no fallback or prohibited escape and a 1,452 ms
median. Qwen3 0.6B Q8 passed the earlier 5/12 corpus with six fallbacks and a 1,790 ms median.
The expanded final V1 corpus passed 26/26 through Metal with no fallback, permitted refusal, or
prohibited escape and an 891 ms median. Qwen3.5 Q4 is therefore the selected candidate. The release
adapter keeps one authenticated loopback `llama-server` sidecar warm. The final ad hoc signed
package served two grounded turns in 1,222 ms total including server load; an earlier package took
1,156 ms, with already-loaded probes at 107 ms and 52 ms.
Reproduction commands and caveats are in [local-mouth.md](local-mouth.md).

LFM2's current free commercial terms include a revenue threshold, so it should not become the
default without a fresh licensing decision.

On this Mac, `llama.cpp` initializes Metal even when `-ngl 0` is supplied. A genuinely CPU-only
run needs all of:

```text
--device none --no-op-offload -ngl 0
```

This matters for headless evaluation and for measuring a realistic CPU fallback.

## Local speech

`KittenML/kitten-tts-nano-0.8` generated the line:

```text
Rain is fine. Your red berry was bullshit.
```

The development Python package loaded in 3.821 seconds and generated 5.76 seconds of 24 kHz audio
in 0.163 seconds. It was research tooling only. The subsequent native Rust spike pins Kitten nano
v0.8 int8, loads in 174 to 206 ms, and generated a 4.08-second line in 857 to 869 ms (RTF 0.21) at
roughly 171 MB RSS. It uses versioned bounded JSONL and a content-addressed WAV cache, with no Python
runtime dependency.

That adapter and its in-game persistent-worker playback remain behind the non-default
`experimental-gpl-tts` feature. sherpa-onnx 1.13.5 statically embeds GPLv3 espeak-ng, so this path
must not enter an MIT-only release or default CI build. Exact hashes, the real smoke command, and
the release blocker are recorded in [local-mouth.md](local-mouth.md) and
`crates/beastie-ai-worker/TTS.md`.

The release voice instead runs eSpeak NG as a separate offline process through the same bounded
worker and cache. A real macOS eSpeak NG 1.52 smoke produced a 3.36-second mono PCM16 WAV at
22,050 Hz in under 10 ms with about 3.1 MB maximum RSS. The game path adds authoritative
emotion-derived pitch, speed, pauses, vocal-noise and mouth-timing metadata, while invalid or absent
speech remains non-blocking. Distribution is an aggregate and must include eSpeak NG's GPLv3
license, data, and corresponding source or source offer. Piper Plus and Flite were also smoked and
rejected as the portable default; their exact results are in
[tts-release-candidate.md](tts-release-candidate.md) and
[flite-release-candidate.md](flite-release-candidate.md).

Pocket TTS is not a default candidate. Its use restrictions are a poor fit for a game deliberately
designed to produce insults, spite, and crude speech.

## PixelLab

The Codex PixelLab MCP connection completed an authenticated account check and an asynchronous
Pixen image generation. The account is Pixel Apprentice with 2,000 subscription generations. The
smoke used one generation, leaving 1,999.

Production generation is also available through `cargo xtask asset generate <id> [--force]`. It
uses the official authenticated synchronous PixelLab endpoint, derives its bounded request from
the asset manifest, validates dimensions and alpha, and publishes atomically. Its tests use a mock
server and spend no account credits.

- Job: `884135c5-8036-4afb-a15b-f0ed54e76037`
- Model: Pixen
- Canvas: 64×64 transparent PNG
- Seed: `5912203`
- Result: technically valid and expressive, but it reads as a grumpy goblin or vegetable elder
  rather than the intended ageless animal

The probe is intentionally not checked into `assets/` or declared in the manifest. Production work
later selected an OpenAI-generated aquarium concept and PixelLab-generated background, cave,
plants, toys, creature base, and four-frame hover/swim/eat-recoil animation sets. Runtime drawing
uses exact integer crop and scale, with code-native expression fallbacks. Prompts, seeds, job IDs,
references, dimensions, hashes, palettes, and validation policy are recorded in
`assets/manifest.toml`.

## Implemented V1 tool paths

- Local LLM inference: selected Qwen3.5 Q4 with a warm packaged `llama-server` runtime.
- Local TTS generation: native, Python-free eSpeak NG process with bounded game playback and
  explicit GPL aggregate-distribution obligations.
- PixelLab and OpenAI generation: operational; the canonical aquarium art path is integrated and
  provenance-checked.
- Headless fixture path: the canonical aquarium scenario runs without external services and remains
  part of the display-free verification strategy.

Native macOS launch, window discovery, real rendered-window capture, pointer movement, and clicking
have also been proven. The shared semantic headless/visible aquarium loop is now implemented; see
[development-harness.md](development-harness.md). The repeated real model/eSpeak smokes and fresh
473-file macOS package hash audit pass. The final eight-frame aquarium capture and packaged game
smoke also completed from an Alacritty GUI child. The shared remote-control daemon could not acquire
a Metal drawable directly, and yabai had to be stopped for the packaged launch, then restored.
Physical-controller and native Windows/Linux acceptance remain unproven hardware/platform work.
