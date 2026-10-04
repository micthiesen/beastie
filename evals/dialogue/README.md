# Dialogue evaluation

`corpus.json` is the versioned regression corpus for learned-word speech: every case carries the
`speech_intent` and vocabulary the simulation would send. It covers fresh-hatch babble, echoes of
unknown words, just-learned and player-coined words, answers that comply or refuse, wants with and
without a word, greetings, remarks, and prohibited input in every content-boundary category.
Prohibited input is represented only by its typed rejection category, so fixture requests never
retain the unsafe source text, and the creature only babbles back.

The deterministic gate scores `fixtures/dialogue/eval-replies.jsonl` and the no-model composer on
every case:

```bash
cargo xtask dialogue eval
```

This same check runs inside `cargo xtask verify` without a display, network, model, or worker.

To benchmark a local GGUF through the real worker, first build the worker, then run:

```bash
cargo build --package beastie-ai-worker
cargo xtask dialogue eval \
  --worker target/debug/beastie-ai-worker \
  --model models/candidate.gguf \
  --cpu-only \
  --label candidate-q4
```

Use `--llama-cli`, `--timeout-ms`, `--max-output-bytes`, and repeatable `--llama-arg` options when
the worker defaults are unsuitable. Omit `--cpu-only` for an accelerated comparison. Real runs
write reproducible per-case JSON and a compact
Markdown comparison under the ignored `evals/reports/` directory. The JSON report includes the
exact worker invocation and requests, raw replies, latency, word counts, protocol validity (which
includes using only learned words), content expectations, worker fallbacks, assistant-voice hits,
prohibited-content escapes, and the exact failed checks. The prompt itself is hillclimbed with the
larger frozen set in `speech/`.

Scoring is deliberately lexical and deterministic. It is a regression and model-comparison tool,
not a general content classifier. Extend terms and cases when a candidate exposes a new failure
mode, and inspect raw replies before selecting or shipping a model.
