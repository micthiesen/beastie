# STT evaluation: "/home/michael/beastie-install/usr/bin/beastie-stt" "--backend" "parakeet" "--audio-root" "/home/michael/beastie-release/target/stt-eval-audio-20828" "--model-dir" "/home/michael/beastie-install/usr/bin/models/parakeet-tdt-0.6b-v3-int8"

Synthetic fixtures only: **yes**. This report is not real-human acceptance evidence.

- Passed: 7/11
- Normalized WER: 0.135 (7 errors / 52 reference words)
- Keyword recall: 0.688 (11/16)
- Exact transcripts: 5/9 speech cases
- No-speech: 2/2 correct
- Cold latency: 1067 ms
- Warm median latency: 265 ms
- Peak process-tree RSS: unavailable on this host

## Cases

| Case | Condition | Outcome | WER | Keywords | Confidence | Latency |
|---|---|---:|---:|---:|---:|---:|
| espeak-us-muck-berry | clean | recognized | 0.333 | 0/2 | 800 | 1067 ms |
| espeak-gb-why-there | question | recognized | 0.000 | 2/2 | 800 | 245 ms |
| espeak-us-mushroom | aquarium interaction vocabulary | recognized | 0.000 | 1/1 | 800 | 262 ms |
| espeak-gb-profanity | permitted profanity | recognized | 0.000 | 2/2 | 800 | 265 ms |
| espeak-us-bell-sock | aquarium object vocabulary | recognized | 0.125 | 1/2 | 800 | 298 ms |
| espeak-gb-asshole | permitted profanity | recognized | 0.400 | 0/1 | 800 | 264 ms |
| espeak-us-repeated-name | repetition and question | recognized | 0.400 | 1/2 | 800 | 329 ms |
| espeak-gb-disfluency | disfluency | recognized | 0.000 | 2/2 | 800 | 435 ms |
| espeak-us-noisy-aquarium | deterministic aquarium-like noise mix | recognized | 0.000 | 2/2 | 800 | 320 ms |
| silence | one second digital silence | no_speech | 0.000 | 0/0 | n/a | 0 ms |
| aquarium-noise | one second deterministic low-amplitude pink noise | no_speech | 0.000 | 0/0 | n/a | 172 ms |

## Confidence calibration

| Confidence | Cases | Exact | Exact rate | Mean confidence |
|---|---:|---:|---:|---:|
| 0-249 | 0 | 0 | 0.000 | 0 |
| 250-499 | 0 | 0 | 0.000 | 0 |
| 500-649 | 0 | 0 | 0.000 | 0 |
| 650-799 | 0 | 0 | 0.000 | 0 |
| 800-1000 | 9 | 5 | 0.556 | 800 |
