# Offline STT runtime evidence

## Decision

Moonshine Voice 0.1.2 at revision `07648e45e0b1daf1923ce325cd61d624a407e615`, with
Tiny Streaming English architecture 2, is the **provisional** V2 recognizer. It fits Beastie's
bounded, local, streaming interaction better than the current Whisper comparisons and keeps the
seven-file model at 51,441,771 bytes. It is not a shipping acceptance yet.

The choice remains provisional because all current accuracy evidence uses synthetic voices. That
is useful for plumbing and regression, but cannot establish performance on real people, rooms,
microphones, accents, speech differences, interruptions, or aquarium audio. The game must not
describe this runtime as accepted until the real-human matrix below passes.

Exact runtime, model component, license, size, and hash records live in `models/manifest.toml`.

## Repeatable commands

The normal path is display-free, model-free, device-free, network-free, and part of the full gate:

```bash
cargo xtask stt eval
cargo xtask verify
```

It validates every checked-in WAV by byte count, SHA-256, RIFF structure, PCM16 encoding, mono
channel count, and 16 kHz rate. It then scores frozen protocol replies and detects corpus or reply
identity drift.

The explicit real-runtime path starts one persistent worker, stages validated WAVs into a private
audio root as `<sha256>.wav`, sends correlated JSONL requests, and writes ignored JSON and Markdown
reports beneath `evals/reports/`:

```bash
cargo build --package beastie-ai-worker --bin beastie-stt
cargo xtask stt eval \
  --worker target/debug/beastie-stt \
  --model-dir /absolute/path/to/tiny-streaming-en \
  --moonshine-engine /absolute/path/to/beastie-moonshine-engine \
  --label moonshine-macos-arm64
```

The evaluator measures normalized word error rate, keyword recall, exact transcripts, speech versus
no-speech classification, confidence calibration buckets, first-request cold latency, later-request
warm median latency, and peak worker-plus-engine process-tree RSS on macOS. RSS is recorded as
unavailable on unsupported hosts instead of failing the run. The outer reply watchdog and bounded line reader prevent a stuck
or noisy worker from hanging the evaluation indefinitely.

## Corpus contract

`evals/stt/corpus.json` version 1 contains 11 short, frozen mono PCM16 WAV files at 16 kHz:

- two deterministic eSpeak NG 1.52.0 English voices;
- the proper name Muck plus berry, mushroom, aquarium, bell, and sock vocabulary;
- permitted profanity, questions, repeated address, and explicit disfluency;
- a deterministic synthetic noisy utterance;
- digital silence and deterministic low-amplitude pink-noise no-speech controls.

These fixtures are deliberately small and redistributable as project test data. Their provenance is
documented in `evals/stt/README.md`. Frozen perfect replies prove scorer and protocol behavior, not
recognizer quality.

## Current comparison

| Candidate | Pinned runtime | Model bytes | Current evidence | Status |
|---|---:|---:|---|---|
| Moonshine Tiny Streaming English | 0.1.2, `07648e45...` | 51,441,771 | Checked-in redistributable eSpeak corpus: 4/11 strict cases, 0.308 WER (16 errors/52 words), 7/16 keywords, 2/2 no-speech, 122 ms cold, 57 ms warm median, and 304 MiB peak process-tree RSS. A legacy system-voice corpus scored better, which reinforces that synthetic-voice results are not acceptance evidence. | Provisional preference based on architecture and runtime fit, not accepted accuracy |
| Whisper tiny.en Q5_1 | 1.9.2, `306c88f4...` | 32,166,155 | Directional v1.8.6 synthetic run only. Small and fast, but Muck, homophones, and noisy profanity were inconsistent. v1.9.2 still needs a harness rerun. | Comparison |
| Whisper base.en Q5_1 | 1.9.2, `306c88f4...` | 59,721,011 | Directional v1.8.6 synthetic run only. Often changed Muck to Mark and did not consistently beat tiny on the domain corpus. v1.9.2 still needs a harness rerun. | Comparison |

Moonshine's synthetic misses included proper names, object vocabulary, and permitted profanity. Whisper confidence
signals also failed to track correctness reliably enough to justify a global guessed threshold.
Moonshine's optional 32,515,016-byte attention decoder was rejected after the corresponding
word-alignment path took about 23 seconds for a short utterance. The seven-file runtime reports a
conservative 800 only when VAD and endpoint completion agree. It is a usability score, not a word
probability. The checked-in evaluator therefore preserves confidence buckets instead of presenting
confidence as calibrated truth.

## Real-human acceptance matrix

Before changing `selection.stt.status` from `provisional`, record a report that covers all of these:

1. At least three real speakers, including more than one accent or speech profile, using the actual
   in-game microphone capture path.
2. Quiet near-field speech, ordinary room noise, aquarium/game audio, and one competing-speech case.
3. Muck, aquarium objects, questions, disfluency, permitted profanity, silence, and background noise.
4. Cold and warm finalization latency, process RSS, no-speech false activations, and recovery after a
   malformed or unavailable backend.
5. Confidence calibration chosen from observed errors, followed by a fresh holdout run. Do not tune
   the threshold on the same clips used to report success.

Acceptance also requires listening and interaction judgment in the real game. Aggregate WER alone
cannot establish whether endpointing, early attention, recovery, and latency feel like sharing a
space with the creature.

## Distribution boundary

Runtime play remains offline. The engine and all seven exact model components must be packaged with
the game, along with Moonshine's license and required third-party notices. No runtime downloader or
silent cloud fallback is permitted. Windows, Linux, and macOS builds must each prove native engine
loading, recognition, worker shutdown, and package audit before release.
