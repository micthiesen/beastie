# Beastie AI worker

The worker speaks versioned JSONL on standard input and output. Every request carries a speech
intent decided by the simulation (a request without one, from an older fixture or save, babbles).
The model writes only the spoken line, which must use the creature's learned words, its sounds and
stage glue; anything else is answered by the deterministic composer, `compose_line`, which is the
complete no-model voice. Prohibited player input and echo attempts never reach a model.

The fixture backend is the default. It returns the composed line and requires no model, runtime,
network, display, or GPU:

```sh
cargo run -p beastie-ai-worker --bin beastie-ai-worker < fixtures/dialogue/speech-request.json
```

The companion `beastie-tts` binary provides bounded offline speech through a separately installed
eSpeak NG executable. It stays behind the existing persistent JSONL and content-hashed WAV cache
boundary, fails silently from the game's perspective, and does not link GPL code into Beastie's
MIT Rust binaries. Setup, smoke commands, and distribution compliance are in [TTS.md](TTS.md).

The opt-in `llama-cpp` backend is a bounded integration and evaluation spike:

```sh
cargo run -p beastie-ai-worker -- \
  --backend llama-cpp \
  --model models/Qwen3-0.6B-Q8_0.gguf \
  --cpu-only
```

The opt-in `llama-server` backend keeps one local `llama-server` sidecar alive for the worker
process, so the GGUF loads once and two dialogue requests reuse it:

```sh
cargo run -p beastie-ai-worker -- \
  --backend llama-server \
  --model models/Qwen3.5-0.8B-Q4_0.gguf
```

It binds only `127.0.0.1` on an ephemeral port, retries startup, creates a unique API key for the
worker process, waits for `/health`, and sends authenticated bounded requests to
`/v1/chat/completions`. Each request samples one short line (see `src/speech.rs` for temperature,
token ceiling and per-request seeds) with reasoning disabled (`reasoning_effort: "none"` and
`chat_template_kwargs.enable_thinking: false`). A line outside the creature's words is resampled on
the warm server, up to three draws, then the composed line answers. A timeout, oversized response,
or sidecar exit stops that sidecar and starts a replacement once; a second transport failure
returns the composed line marked `generation_failed`. `--cpu-only` supplies
`--device none --no-op-offload -ngl 0` to the server as well.

`BEASTIE_AI_BACKEND=llama-server`, `BEASTIE_LLAMA_SERVER`, `BEASTIE_AI_MODEL`,
`BEASTIE_AI_TIMEOUT_MS`, `BEASTIE_AI_MAX_OUTPUT_BYTES`, and `BEASTIE_AI_CPU_ONLY` configure this
mode. Repeat `--llama-server-arg VALUE` for a server-specific option. The original
`BEASTIE_LLAMA_CLI` and `--llama-arg` contract remains exclusive to `llama-cpp` evaluation.

On macOS and Linux the server is in a dedicated process group, which is killed on retry and worker
drop so server descendants are reaped with it. Windows currently terminates only the direct server
process; Job Object containment remains required before a Windows release can guarantee descendant
cleanup after a forced worker shutdown.

`BEASTIE_AI_BACKEND`, `BEASTIE_AI_MODEL`, `BEASTIE_LLAMA_CLI`,
`BEASTIE_AI_TIMEOUT_MS`, `BEASTIE_AI_MAX_OUTPUT_BYTES`, and `BEASTIE_AI_CPU_ONLY` provide the same
configuration for launchers. Repeat `--llama-arg VALUE` to add runtime-specific arguments.

Each attempt receives the same line-only prompt with reasoning disabled. The adapter takes the first
non-empty output line, and the worker builds the `DialogueReply` itself. The worker retries once,
then returns the composed line. Child stdout and runtime are bounded, stderr is discarded, and
`--cpu-only` supplies `--device none --no-op-offload -ngl 0`.

Every accepted reply also passes protocol validation, including the content boundary for racial
slurs, protected-class extermination endorsements, and a narrow graphic-sex lexicon.

## Runtime limitation

This backend launches `llama-cli` once per attempt, so it reloads the GGUF for every utterance.
llama.cpp 10310 interactive stdin mode was investigated but is not a dependable framed protocol:
EOF produced an unbounded prompt loop, and its human console delimiters can collide with generated
text. Treat this
adapter as replayable model evaluation, not the shipping warm runtime.

The game currently also launches the Beastie worker once per dialogue. Production integration must
first keep one worker alive for the game session, then replace this backend with a persistent native
runtime or a separately framed long-lived llama.cpp service. The JSONL and `DialogueBackend`
boundaries are already reusable for that replacement.
