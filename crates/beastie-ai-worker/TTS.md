# Experimental native TTS

The TTS worker loads Kitten nano v0.8 int8 once, accepts versioned JSONL requests, and writes 24 kHz
mono PCM WAV files beneath an explicit cache directory. Voice settings and cache keys are
deterministic; repeated native synthesis is not guaranteed to produce byte-identical PCM, so the
first valid cached result is reused. It has no Python dependency. Text, voice, speed, silence,
output duration, protocol lines, and cache filenames are bounded or validated before they cross the
worker boundary.

## License blocker: do not distribute

**The pinned sherpa-onnx 1.13.5 static runtime embeds GPLv3 espeak-ng.** The native adapter and smoke
binary therefore require the non-default `experimental-gpl-tts` feature. Do not enable that feature
in an MIT-only Beastie build, release, package, or CI gate. The model itself is Apache-2.0; exact
artifact provenance is in `models/kitten-nano-en-v0_8-int8.toml`. Upstream licensing is tracked at
<https://github.com/k2-fsa/sherpa-onnx/issues/3731>.

## Real smoke

Download and verify the pinned 31,220,690-byte archive outside the repository:

```sh
mkdir -p /tmp/beastie-tts-smoke
cd /tmp/beastie-tts-smoke
curl -fL -o kitten-nano-en-v0_8-int8.tar.bz2 \
  https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kitten-nano-en-v0_8-int8.tar.bz2
printf '%s  %s\n' \
  6fa5be852612ce761094ba74ee6123b4fc4acfefa79bf64dc63acae4a83af2fd \
  kitten-nano-en-v0_8-int8.tar.bz2 | shasum -a 256 -c -
tar xjf kitten-nano-en-v0_8-int8.tar.bz2
```

From the Beastie repository, send one request to the persistent-worker interface:

```sh
printf '%s\n' '{"protocol_version":1,"request_id":1,"text":"red shit again.","settings":{"speaker_id":0,"speed":1.0,"silence_scale":0.2}}' | \
cargo run -p beastie-ai-worker \
  --features experimental-gpl-tts \
  --bin beastie-tts -- \
  --model-dir /tmp/beastie-tts-smoke/kitten-nano-en-v0_8-int8 \
  --cache-dir /tmp/beastie-tts-smoke/cache
```

The reply contains a cache key, never a path. The game derives that key beneath its configured cache
root, copies the bounded WAV bytes, and plays them asynchronously. Repeating an identical request
reuses the same entry. On the M5 Max research smoke, model load took 174 to 206 ms; a 4.08 s line
generated in 857 to 869 ms (RTF 0.21) at roughly 171 MB RSS.

To run the feature-gated game integration:

```sh
cargo xtask dev --fake-ai \
  --tts-model-dir /tmp/beastie-tts-smoke/kitten-nano-en-v0_8-int8 \
  --tts-cache-dir /tmp/beastie-tts-smoke/cache
```

Default offline tests exercise the cache and WAV boundary with a fake synthesizer:

```sh
cargo test -p beastie-ai-worker --test tts
```
