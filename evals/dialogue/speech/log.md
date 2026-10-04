# Speech hillclimb log

Model: Qwen3.5 0.8B Q4_0 through the worker's `llama-server` backend, M5 Max, 2026-10-04.
Each change is one coherent cause; train is for diagnosis, validation for acceptance.

| Version | Change | Train good | Val good | Notes |
|---|---|---:|---:|---|
| v0 | JSON scaffold prompt (pre-rework worker path) | 0/32 | — | Every speech line fell back: the model dropped JSON fields and used sounds outside the fixed list. |
| baseline | Line-only prompt, worker builds the reply; stretched creature sounds accepted | 20/32 | — | Model copies glosses ("coming over"), omits the intent word, mangles echoes. |
| v1 | Echo intents answered with the simulation's exact attempt; "know" added to glue | 23/32 | — | The composer itself said "mop know ball!" with an unallowed word. |
| v2 | Vocabulary listed as bare words; situations phrased with the creature's own word | 26/32 | — | Gloss leakage gone. Remaining misses omit the target word. Retries reused the same seed. |
| v3 | Retries reseed on the warm server (3 samples); prompt and parser require the target word | 31/32 | 15/15 | Only miss: a stage-one refusal that never says "no" (no glue word available yet), covered by the composer. |
| v4 | Answers may not contradict the creature's choice (no "no" when complying, a no when refusing) | 31/32 | 15/15 | Found in native play: the model once said "no sock!" for a complied request. Composer variation now uses splitmix64 and six remark templates. |

Accepted: v3. 24 distinct model lines on train; the model copies the composer's example about a
third of the time, so variety comes mostly from the model's own short variations. Latency stays
around 140 ms per line on a warm server; a rejected sample costs one more round trip.

Diagnostics: set `BEASTIE_DEBUG_MODEL=1` to have the worker print each raw speech sample with its
request ID to stderr; `run.py` attaches them to failing cases.

v5 (after removing the legacy intent-free dialogue path; no prompt change): 31/32 train, 15/15
validation, confirming the refactor kept the accepted behavior.
