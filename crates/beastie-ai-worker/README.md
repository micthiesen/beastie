# Beastie AI worker

The worker speaks versioned JSONL on standard input and output. Its deterministic fixture backend
is the default and requires no model, runtime, network, display, or GPU:

```sh
cargo run -p beastie-ai-worker < fixtures/dialogue/berry-memory.json
```

The opt-in `llama-cpp` backend is a bounded integration and evaluation spike:

```sh
cargo run -p beastie-ai-worker -- \
  --backend llama-cpp \
  --model models/Qwen3-0.6B-Q8_0.gguf \
  --cpu-only
```

`BEASTIE_AI_BACKEND`, `BEASTIE_AI_MODEL`, `BEASTIE_LLAMA_CLI`,
`BEASTIE_AI_TIMEOUT_MS`, `BEASTIE_AI_MAX_OUTPUT_BYTES`, and `BEASTIE_AI_CPU_ONLY` provide the same
configuration for launchers. Repeat `--llama-arg VALUE` to add runtime-specific arguments.

Each attempt receives a curated structured-output contract, a 128-token ceiling, and reasoning
disabled. llama-cli 10310 writes its console banner and echoed prompt to stdout, so the adapter
requires exactly one unique JSON value in that stream to parse and validate as a `DialogueReply`. Validation rejects an
incorrect request ID, disallowed gesture, unoffered memory, or excessive word count. The worker
retries once, then returns the authored constrained fallback. Child stdout and runtime are bounded,
stderr is discarded, and `--cpu-only` supplies `--device none --no-op-offload -ngl 0`.

Before acceptance, a deterministic last-line filter rejects near-verbatim player echoes, racial
slurs, protected-class extermination endorsements, and a narrow graphic-sex lexicon. Ordinary
profanity and mild non-explicit innuendo remain allowed. Rust also selects the content lane,
gesture, and recalled-memory ID before prompting; the model only phrases `say` in a request-specific
reply scaffold.

## Runtime limitation

This backend launches `llama-cli` once per attempt, so it reloads the GGUF for every utterance.
llama.cpp 10310 interactive stdin mode was investigated but is not a dependable framed protocol:
EOF produced an unbounded prompt loop, and its human console delimiters can collide with generated
text. The installed build's `--json-schema` sampler also failed to initialize even for a generic
object schema, so schema enforcement remains in Beastie's strict parser and validator. Treat this
adapter as replayable model evaluation, not the shipping warm runtime.

The game currently also launches the Beastie worker once per dialogue. Production integration must
first keep one worker alive for the game session, then replace this backend with a persistent native
runtime or a separately framed long-lived llama.cpp service. The JSONL and `DialogueBackend`
boundaries are already reusable for that replacement.
