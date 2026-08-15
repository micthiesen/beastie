# Tooling preflight

Local development tooling was smoke-tested on 2026-08-15 before the MVP build begins. These
results prove that each external content path can run on the development Mac. They do not select
the shipping model, voice, or art.

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

One prompt is not a selection test. The next model step is a checked-in Beastie dialogue corpus
that measures factual grounding, schema compliance, latency, profanity/insult willingness,
prohibited-output rate, and assistant-like filler. Qwen is the default licensing-safe family to
evaluate first. LFM2's current free commercial terms include a revenue threshold, so it should not
become the default without a fresh licensing decision.

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
in 0.163 seconds. The output was intelligible and fast enough to justify a native integration
spike. It remains a development preview, so the shipping path should benchmark both KittenTTS and
Kokoro behind the worker's replaceable TTS interface. Prefer `sherpa-onnx` Rust bindings so the
packaged game has no Python dependency, and pin the exact runtime and model revisions after the
choice is made.

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

- Local LLM inference: operational; model selection still needs the real eval corpus.
- Local TTS generation: operational; native Rust packaging still needs an integration spike.
- PixelLab generation: operational; canonical art anchors are not selected yet.
- Headless fixture path: remains the required default for `cargo xtask verify`.
