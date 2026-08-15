# Dialogue evaluation

`corpus.json` is the versioned model-selection corpus. It tests factual grounding, strict reply
schema and request IDs, word ceilings, allowed gestures and memory IDs, willingness to produce
permitted profanity, insults, and mild innuendo, refusal of hate and explicit sexual content, and
avoidance of generic assistant language.

The deterministic gate scores `fixtures/dialogue/eval-replies.jsonl`:

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
exact worker invocation and requests, raw replies, latency, word counts, recalled IDs, protocol validity, grounding, content-policy
signals, assistant-voice hits, and the exact failed checks.

Scoring is deliberately lexical and deterministic. It is a regression and model-comparison tool,
not a general content classifier. Extend terms and cases when a candidate exposes a new failure
mode, and inspect raw replies before selecting or shipping a model.
