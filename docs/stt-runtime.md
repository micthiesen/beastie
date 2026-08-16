# Offline STT runtime evidence

## Decision

NVIDIA Parakeet TDT 0.6B V3 INT8 is the selected recognizer. It runs entirely inside Beastie's
persistent `beastie-stt` process through `transcribe-rs` 0.3.8 and ONNX Runtime. Audio, transcripts,
and inference never leave the machine. The five-file model is 670,619,803 bytes and every component
is pinned in `models/manifest.toml`.

Handy's current local-model guide recommends Parakeet V3 as its fast, high-accuracy default for
European languages. Beastie's own frozen corpus supports that direction: Parakeet reduced WER from
the keyterm-biased Moonshine Tiny result of 0.192 to 0.154. Moonshine remains an explicit lightweight
fallback for constrained machines, not the packaged default.

Selection references: [Handy model guide](https://handy.computer/docs/models),
[Handy source at the audited revision](https://github.com/cjpais/Handy/tree/98a4d80cce8ad41efec2a419b59d9e81229a35d7), and the
[official NVIDIA model card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3).

Exact runtime, model component, license, size, and hash records live in `models/manifest.toml`.

## Repeatable commands

The normal path is display-free, model-free, device-free, network-free, and part of the full gate:

```bash
cargo xtask stt eval
cargo xtask verify
```

One explicit setup command downloads the selected model archive, verifies its fixed byte count and
SHA-256, safely extracts it, verifies all five component hashes, and publishes it under `target/stt`:

```bash
cargo xtask stt setup
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
  --backend parakeet \
  --model-dir target/stt/parakeet-tdt-0.6b-v3-int8 \
  --label parakeet-v3-macos-arm64
```

Run the actual game against the same local worker and model with:

```bash
cargo xtask dev \
  --stt-backend parakeet \
  --stt-model-dir target/stt/parakeet-tdt-0.6b-v3-int8 \
  --new-game
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
| Parakeet TDT 0.6B V3 INT8 | transcribe-rs 0.3.8 | 670,619,803 | Checked-in redistributable eSpeak corpus: 6/11 strict cases, 0.154 WER (8 errors/52 words), 11/16 keywords, 2/2 no-speech, 1,155 ms cold, 159 ms warm median, and 1,846 MiB peak process-tree RSS. Handy rates Parakeet V3 fast and high-accuracy. | Selected local quality default |
| Moonshine Tiny Streaming English | 0.1.2, `07648e45...` | 51,441,771 | With Beastie keyterms: 4/11 strict cases, 0.192 WER (10 errors/52 words), 12/16 keywords, 2/2 no-speech, 270 ms cold, 103 ms warm median, and 307 MiB peak process-tree RSS. | Lightweight fallback |
| Whisper tiny.en Q5_1 | 1.9.2, `306c88f4...` | 32,166,155 | Directional v1.8.6 synthetic run only. Small and fast, but Muck, homophones, and noisy profanity were inconsistent. v1.9.2 still needs a harness rerun. | Comparison |
| Whisper base.en Q5_1 | 1.9.2, `306c88f4...` | 59,721,011 | Directional v1.8.6 synthetic run only. Often changed Muck to Mark and did not consistently beat tiny on the domain corpus. v1.9.2 still needs a harness rerun. | Comparison |

Synthetic misses still include the proper name Muck, object vocabulary, and permitted profanity.
Neither Parakeet nor Moonshine exposes a calibrated utterance probability through the selected
adapter, so confidence 800 means recognized and usable, not 80 percent certainty. Whisper confidence
signals also failed to track correctness reliably enough to justify a global guessed threshold.
Moonshine's optional 32,515,016-byte attention decoder was rejected after the corresponding
word-alignment path took about 23 seconds for a short utterance. The seven-file runtime reports a
conservative 800 only when VAD and endpoint completion agree. It is a usability score, not a word
probability. The checked-in evaluator therefore preserves confidence buckets instead of presenting
confidence as calibrated truth.

## Automated acceptance

The checked-in gate validates the protocol, exact audio identity, speech/no-speech behavior,
privacy cleanup, cancellation, timeout recovery, process reuse, model hashes, and packaging without
a microphone or network. The explicit real-runtime command adds measured transcription, latency,
and RSS evidence on the development Mac. Synthetic speech is not presented as a universal human
accuracy claim; it is a deterministic regression set. Future playtesting can improve the corpus,
but it is not a blocker that silently makes the implemented recognizer unavailable.

## Distribution boundary

Runtime play remains offline. The worker and all five exact Parakeet components must be packaged
with the game, along with the CC-BY-4.0 license and model card. No runtime downloader or silent cloud
fallback is permitted. Windows, Linux, and macOS builds must each prove native model loading,
recognition, worker shutdown, and package audit before release.
