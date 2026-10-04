# Learned-word speech eval

Measures the local model's voice for requests carrying a `speech_intent`, the only dialogue the
game sends since the fun rework. It exists to hillclimb the worker's speech prompt and sampling
(`crates/beastie-ai-worker/src/speech.rs`) against the deterministic composer
(`crates/beastie-protocol/src/speech.rs`), which is the complete no-model voice.

- **Target:** `speech_prompt`, `parse_speech_line`, speech sampling parameters, and composer
  phrasing. The simulation's intents, vocabulary and the reply validator are fixed for this eval.
- **Cases:** `make_cases.py` writes 47 frozen requests grouped by scenario (new word, echo, answers
  by response, wants, remarks, babble across vocabulary sizes and moods). Groups are split
  train/validation (32/15). Diagnose on train, accept changes on validation.
- **Grader (`run.py`):** a line is *good* when the model's own line is accepted (no worker fallback),
  it uses the word the intent is about (new word, answer, want/remark with a known word, echo
  attempt), babble is sounds only, and a refusal says no. Also reported: distinct model lines and
  how often the model merely copies the composer's example.
- **Stop:** when validation good-rate stops improving across two coherent changes, or every
  remaining failure is a model limitation the composer fallback already covers.

Run (worker built, Qwen3.5 0.8B Q4 GGUF at `models/`):

```sh
cargo build -p beastie-ai-worker
python3 evals/dialogue/speech/run.py train <label>
python3 evals/dialogue/speech/run.py val <label>
```

`results/` holds raw graded runs. The experiment log is `log.md`.
