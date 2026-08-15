# Tooling preflight

Local development tooling was smoke-tested on 2026-08-15 before the MVP build began. These initial
results proved that each external content path could run on the development Mac. Stage 4 follow-up
measurements and provisional selections are summarized here and detailed in
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

The checked-in version 1 Beastie corpus now measures factual grounding, schema compliance, latency,
profanity/insult willingness, prohibited-output escape, and assistant-like filler. Through
`llama.cpp` 10310 on Metal, Qwen3.5 0.8B Q4 passed 11/12 with one fallback and an 889 ms median;
Qwen3 0.6B Q8 passed 5/12 with six fallbacks and a 1,790 ms median. Qwen3.5 Q4 is therefore the
provisional candidate. Each case still starts a fresh `llama-cli`, with a warm filesystem cache, so
these are not production warm-runtime measurements. Reproduction commands and lane results are in
[local-mouth.md](local-mouth.md).

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

Pocket TTS is not a default candidate. Its use restrictions are a poor fit for a game deliberately
designed to produce insults, spite, and crude speech.

## PixelLab

The Codex PixelLab MCP connection completed an authenticated account check and an asynchronous
Pixen image generation. The account is Pixel Apprentice with 2,000 subscription generations. The
smoke used one generation, leaving 1,999.

- Job: `884135c5-8036-4afb-a15b-f0ed54e76037`
- Model: Pixen
- Canvas: 64×64 transparent PNG
- Seed: `5912203`
- Result: technically valid and expressive, but it reads as a grumpy goblin or vegetable elder
  rather than the intended ageless animal

The probe is intentionally not checked into `assets/` or declared in the manifest. Production art
should begin with a small set of silhouette/style anchors, select one deliberately, and use it as a
reference for later character states and room assets. Record prompts, seeds, generator mode, job
IDs, dimensions, hashes, and any manual edits in `assets/manifest.toml` when assets are promoted.

## Ready for the MVP build

- Local LLM inference: operational; Qwen3.5 Q4 is provisional, while a warm packaged runtime remains open.
- Local TTS generation: native and Python-free with feature-gated game playback; a
  license-compatible default release runtime remains open.
- PixelLab generation: operational; the canonical art path is integrated.
- Headless fixture path: remains the required default for `cargo xtask verify`.

Native macOS launch, window discovery, real rendered-window capture, pointer movement, and clicking
have also been proven. This is enough to start autonomous full-MVP work. The richer shared semantic
headless/visible control loop should grow incrementally with actual gameplay interactions rather
than block them upfront; see [development-harness.md](development-harness.md).
