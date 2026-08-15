# Offline TTS

`beastie-tts` is a persistent versioned-JSONL worker. Its default backend runs a separately
installed eSpeak NG executable on the local CPU. It has no Python, network, GPU, or display
dependency. Beastie sends at most 2,048 UTF-8 bytes through stdin, accepts only canonical mono
16-bit PCM WAV output between 8 and 192 kHz and at most 30 seconds, then rewrites it atomically
beneath the configured cache directory. A process failure or invalid output returns a protocol
error and the game continues silently.

Cache keys bind the adapter version, voice, speaker variant, speed, silence setting, and text.
The first validated WAV is reused, so repeated requests are deterministic even when synthesis
bytes vary between eSpeak versions.

## eSpeak NG setup and smoke

Install eSpeak NG using the platform's normal package or from an upstream release, then confirm
`espeak-ng --version` succeeds. Common package commands are `brew install espeak-ng` on macOS,
`apt install espeak-ng` on Debian or Ubuntu, and the eSpeak NG installer or package manager on
Windows.

```sh
printf '%s\n' '{"protocol_version":1,"request_id":1,"text":"red shit again.","settings":{"speaker_id":0,"speed":1.0,"silence_scale":0.2}}' | \
cargo run -p beastie-ai-worker --bin beastie-tts -- \
  --backend espeak \
  --espeak espeak-ng \
  --cache-dir /tmp/beastie-tts-cache
```

The reply contains a cache key, never a path. `BEASTIE_TTS_BACKEND`, `BEASTIE_ESPEAK_NG`,
`BEASTIE_ESPEAK_DATA`, `BEASTIE_ESPEAK_VOICE`, and `BEASTIE_TTS_CACHE_DIR` provide the same
configuration. `--espeak-data` must point to the directory named `espeak-ng-data`; the adapter
passes its parent as eSpeak NG's `--path` so a packaged runtime does not consult a host install.
To run the game integration:

```sh
cargo xtask dev --fake-ai \
  --tts-cache-dir /tmp/beastie-tts-cache \
  --tts-espeak espeak-ng
```

## Distribution and GPLv3 compliance

Beastie's Rust code and binaries remain MIT licensed because they communicate with eSpeak NG only
as a separate executable process. eSpeak NG and its voice data are GPLv3 software. Distributing
them beside Beastie is an aggregate distribution, not a relicensing of Beastie, but the eSpeak NG
files still carry their own GPLv3 obligations.

Any Beastie package that includes eSpeak NG or its data must also include the corresponding GPLv3
license and copyright notices, identify the included eSpeak NG version and modifications, and
provide the complete corresponding source in the package or a valid written source offer as GPLv3
section 6 permits. The source or offer must cover the exact executable and data shipped, including
build scripts and local modifications. Do not imply that Beastie's MIT license covers those files.
If a platform package relies on a user-installed eSpeak NG instead, document the dependency and do
not copy its executable or data into the Beastie package.

## Experimental Kitten backend

The previous Kitten nano path remains available only for research. The pinned sherpa-onnx 1.13.5
static runtime embeds GPLv3 eSpeak NG, so it requires the non-default `experimental-gpl-tts`
feature and must not enter an MIT-only Beastie release:

```sh
cargo run -p beastie-ai-worker \
  --features experimental-gpl-tts \
  --bin beastie-tts -- \
  --backend sherpa-kitten \
  --model-dir /path/to/kitten-nano-en-v0_8-int8 \
  --cache-dir /tmp/beastie-tts-cache
```

Focused fake-executable coverage is platform-independent and requires no installed TTS runtime:

```sh
cargo test -p beastie-ai-worker --test tts
```
