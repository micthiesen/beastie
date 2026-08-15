# Local mouth status

Stage 4 established the replaceable offline dialogue and speech boundaries. The deterministic
fixture backend remains the default for development and `cargo xtask verify`. Real model weights
remain ignored, local, and described by hashes in `models/manifest.toml`.

## Provisional dialogue model

Qwen3.5 0.8B Q4 is the selected dialogue model. It was selected with the version 1 corpus
in `evals/dialogue/corpus.json`, not a single attractive response:

| Candidate | Passed | Protocol valid | Grounded | Fallbacks | Permitted sharpness | Prohibited content | Median |
|---|---:|---:|---:|---:|---:|---:|---:|
| Qwen3.5 0.8B Q4, final CPU-only | 18/18 | 18/18 | 18/18 | 0 | 3/3 | 7/7 | 1,452 ms |
| Qwen3 0.6B Q8 | 5/12 | 12/12 | 11/12 | 6 | 0/3 | 3/3 | 1,790 ms |

The final Qwen3.5 run passed every case without fallback after known food and toy nouns were
anchored from authoritative projected memory facts. It used the deliberately portable CPU-only
lane. The checked-in report is `evals/reports/mvp-final-cpu-fixed.{json,md}`.
Rust selects the response lane, allowed gesture, and recalled-memory ID; the model only phrases the
short `say` field. A final deterministic filter rejects near-verbatim player echoes, a narrow
prohibited lexicon, invalid IDs and gestures, empty or control-bearing text, replies above 512 UTF-8
bytes, and replies over the request's word limit.

The evaluation measurements above used `llama.cpp` 10310 (`cb26014d9`) on the M5 Max through Metal. Every
case launched a new `llama-cli`, so the median includes process startup and model loading. The OS
filesystem cache was warm after the first case. This is neither a first-boot disk-cold measurement
nor evidence of a production warm runtime.

Reproduce a real-model run after placing an ignored GGUF at the recorded path:

```sh
cargo build --package beastie-ai-worker
cargo xtask dialogue eval \
  --worker target/debug/beastie-ai-worker \
  --model models/Qwen3.5-0.8B-Q4_0.gguf \
  --timeout-ms 30000 \
  --label qwen35-local
```

Add `--cpu-only` to exercise the portable fallback. On this Mac, genuine CPU-only llama.cpp needs
all of `--device none --no-op-offload -ngl 0`; the worker supplies them together.

## Process boundary

The game owns one long-lived `DialogueManager` thread and reuses one Beastie worker process across
requests. Only one talk may be outstanding. The outer JSONL transport has bounded lines, validates
both sides of the protocol, has a watchdog covering both model attempts plus grace, returns authored
fallback on failure, and restarts a failed worker on the next request.

Worker output is also bounded at the channel boundary, unsolicited replies force a restart, and
shutdown interrupts generation before joining the manager thread. On macOS and Linux, each worker
uses its own process group so a timeout or quit reaps descendant inference processes as well. On
Windows, both the game and worker own kill-on-close Job Objects for descendant containment.

The selected release adapter starts one `llama-server` sidecar on authenticated loopback and reuses
the loaded model. It binds an ephemeral local port, supplies a random per-process API key, disables
network-dependent behavior, bounds HTTP requests and responses, disables reasoning, and uses
deterministic sampling. A hidden supervisor kills the sidecar if the worker is forcibly terminated.
Malformed, oversized, timed-out, unsafe, or factually mismatched replies restart the sidecar once,
then use authored fallback.

The real packaged two-turn smoke produced two valid grounded replies in 1,156 ms total, including
server startup and model load. Separate already-loaded probes took 107 ms and 52 ms. A CPU-only
corpus run passed 18/18 with no authored fallback and a 1,452 ms median through the deliberately
cold `llama-cli` evaluation path. The release package discovers
its sibling worker, server, and model without developer environment variables.

Grounding now checks both the authoritative memory ID and its concrete subject. A response that
claims a berry memory but says `ball` is rejected. Disliked and liked memories must also express
the supplied valence. If both model attempts fail, a memory turn falls back to a short factual line
such as `berry remains bad.` rather than losing the remembered event.

The simulation learns only typed exposure to permitted profanity, crudeness, and innuendo after an
accepted conversation. Raw player text is not stored as exposure. Prohibited input is normalized to
a typed violation and returns an authored in-character refusal before any model process is invoked.
The shared deterministic boundary covers protected-class hate, explicit sex, sexual minors or
ambiguous age, coercion or abuse, defamatory sexual claims, self-harm encouragement, and credible
violence. Ordinary profanity, direct insults, and mild innuendo remain intentionally available.

## Release speech

The default release voice is a separately executed eSpeak NG process. It is deliberately crunchy,
fast, entirely offline, CPU-only, and available for macOS, Windows, and Linux. The persistent
`beastie-tts` JSONL worker sends bounded text on stdin, validates mono PCM WAV output and duration,
and publishes it through the same deterministic content-addressed cache used by the game. Speech
failure never delays dialogue text or stops play.

The native macOS 1.52.0 smoke synthesized `Rain is fine. Your red berry was bullshit.` as a
3.36-second mono PCM16 WAV at 22,050 Hz in under 10 ms, with 3.1 MB maximum RSS for the eSpeak
process. The statically built executable is 490 KB and the complete compiled voice data directory
is 19 MB. The real Beastie worker accepted the same line, returned a validated cache key, and
published a 2.71-second WAV for voice variant 3.

eSpeak NG is GPLv3. Beastie's Rust code remains MIT and communicates with the unmodified program at
arms length through its ordinary command-line interface. A distributable bundle must include the
eSpeak NG executable, data, GPLv3 license, and the exact corresponding source or written source
offer described in `THIRD_PARTY_NOTICES`. This is an aggregate distribution decision, not a claim
that eSpeak NG is MIT. Platform packaging still needs its native build verified before release.

## Experimental speech spike

The optional Kitten nano v0.8 int8 adapter is native Rust and has no Python runtime dependency. It
loads the model once per TTS process, accepts bounded versioned JSONL, synthesizes 24 kHz mono PCM,
and reuses a deterministic content-addressed WAV cache. Repeated uncached syntheses are not assumed
to be byte-identical. The non-default `experimental-gpl-tts` feature and `beastie-tts` smoke binary
keep this path out of ordinary builds. Exact model and runtime hashes are in
`models/kitten-nano-en-v0_8-int8.toml`.

On the M5 Max native smoke, model load took 174 to 206 ms. A 4.08-second line generated in 857 to
869 ms, RTF 0.21, at roughly 171 MB RSS. Repeated identical input reused the hashed WAV cache. The
command and artifact verification steps are in `crates/beastie-ai-worker/TTS.md`.

The post-review smoke generated a fresh 1.52-second, 72,888-byte mono PCM16 WAV at 24 kHz. The game
boundary accepts only the pinned cache-key shape, opens the final entry without following a symlink
on Unix, copies a bounded file from the same handle, and validates the exact WAV contract before
playback. New speech invalidates older queued or in-flight completions.

Feature-gated gameplay keeps one TTS worker alive, submits versioned JSONL after dialogue text is
already visible, validates the returned cache key and scoped WAV, then plays speech asynchronously
through a dedicated replaceable sink. It remains off by default. The full development path is:

```sh
cargo xtask dev \
  --tts-model-dir /tmp/beastie-tts-smoke/kitten-nano-en-v0_8-int8 \
  --tts-cache-dir /tmp/beastie-tts-smoke/cache
```

This path is not releasable: sherpa-onnx 1.13.5 statically embeds GPLv3 espeak-ng in the same binary. Do not enable
`experimental-gpl-tts` in a release package or default CI build. The separate eSpeak NG process is
the release path; the Kitten adapter remains a quality experiment until its combined-work license
problem is removed.

The reviewed macOS fixture path was launched through Alacritty and produced all three direct
320x180 logical-framebuffer captures in `target/captures/stage4-review`, including restored-memory
speech. It exited without leaving a Beastie, llama.cpp, or TTS process behind.
