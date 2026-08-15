# Piper Plus TTS release-candidate smoke

Status: **blocked. Do not package or enable this candidate.**

This is a Stage 5 smoke of the exact upstream Piper Plus v1.13.0 macOS arm64
prebuilt release and the pinned `piper-plus-css10-ja-6lang` model/config. The
artifacts were downloaded to `/tmp/beastie-piper-plus-smoke-20260815` only. No
model weights or release archive were added to the repository.

## Acceptance context

The MVP requires CPU-only inference, no network during play, no Python runtime,
an under-100 MB TTS model, and less than 2 GB total AI runtime memory. Speech
must remain optional and the game must continue when it fails. The measured
historical control is well inside those size and memory ceilings, but the exact
current model does not load, so this is not a release selection.

Host: Apple M5 Max, macOS arm64. `uname` reported Darwin 25.5, arm64. The
repository's tooling preflight identifies this host as an 18-core, 128 GB M5 Max.

## Exact upstream artifacts

### Runtime

- Release: [ayutaz/piper-plus v1.13.0](https://github.com/ayutaz/piper-plus/releases/tag/v1.13.0)
- Release commit: `7d3aa34e7acf5f8c91d6ad280aefbb369905db2a`
- Download: `https://github.com/ayutaz/piper-plus/releases/download/v1.13.0/piper-macos-arm64.tar.gz`
- Archive size: 39,499,867 bytes
- Archive SHA-256: `bf123f4c54911a411183184bca2e22762e5f6281ab878bd76a83041c172b01d8`
- GitHub release digest: `sha256:bf123f4c54911a411183184bca2e22762e5f6281ab878bd76a83041c172b01d8`
- Cosign bundle: `piper-macos-arm64.tar.gz.cosign.bundle`, 8,797 bytes,
  SHA-256 `ca4b3057b87cb7fcd740fbb65079d3000bd91bf3b82b386e9b81283972c6ef34`.

The archive contains a Mach-O arm64 `piper/bin/piper` (1,749,320 bytes), two
regular ONNX Runtime 1.20.0 arm64 dylibs (25,477,000 bytes each), OpenJTalk
dictionary data, and bundled CMUdict/Pinyin JSON dictionaries. `otool -L`
shows only the bundled `@rpath/libonnxruntime.1.20.0.dylib` plus macOS system
libraries. The extracted `piper/` directory is 159 MiB, dominated by the
103,073,776-byte `sys.dic`.

The archive has only one license file, the dictionary `COPYING`; it has no
top-level Piper Plus, ONNX Runtime, or third-party notice file. Upstream source
licenses are MIT for Piper Plus and ONNX Runtime. The bundled OpenJTalk/UniDic
dictionary `COPYING` contains BSD-style redistribution terms. A redistributable
Beastie package needs an assembled notice set before shipping.

### Model and config

The current pinned Hugging Face revision is `bd0d812d4db9182ecdb907ef074becf9e230c17f`,
last modified 2026-05-03:

- Repository: [ayousanz/piper-plus-css10-ja-6lang](https://huggingface.co/ayousanz/piper-plus-css10-ja-6lang)
- ONNX: `https://huggingface.co/ayousanz/piper-plus-css10-ja-6lang/resolve/bd0d812d4db9182ecdb907ef074becf9e230c17f/css10-ja-6lang-fp16.onnx`
- ONNX size: 39,652,717 bytes
- ONNX SHA-256: `5ebc51dbf897238523f3df0d6e0f6c93033bc5cda3f8602a8379ebe2a4738c42`
- Config: `https://huggingface.co/ayousanz/piper-plus-css10-ja-6lang/resolve/bd0d812d4db9182ecdb907ef074becf9e230c17f/config.json`
- Config size: 5,912 bytes
- Config SHA-256: `3c613dacf139349c20159433559b2e94e6364e9a340d95245f3de6006f453fcc`
- Claimed format: FP16 MB-iSTFT-VITS2, 173 phonemes, one speaker, 22,050 Hz,
  Japanese/English/Chinese/Spanish/French/Portuguese.

## Exact smoke

The actual synthesis was run after all downloads, with no Python and with
network-dependent dictionary download disabled:

```sh
PIPER_OFFLINE_MODE=1 \
PIPER_AUTO_DOWNLOAD_DICT=0 \
OPENJTALK_DICTIONARY_PATH=/tmp/beastie-piper-plus-smoke-20260815/release/piper/share/open_jtalk/dic \
PIPER_DICTIONARIES_PATH=/tmp/beastie-piper-plus-smoke-20260815/release/piper/share/piper/dicts \
/tmp/beastie-piper-plus-smoke-20260815/release/piper/bin/piper \
  --model /tmp/beastie-piper-plus-smoke-20260815/model/css10-ja-6lang-fp16.onnx \
  --config /tmp/beastie-piper-plus-smoke-20260815/model/config.json \
  --language en \
  --text 'Little creature, you are safe here.' \
  --output_file /tmp/beastie-piper-plus-smoke-20260815/results/english.wav \
  --debug
```

Result for the exact current model: **failed before model load**. The process
exited 134 after 0.01 seconds with maximum RSS 15,695,872 bytes and produced no
WAV. The runtime logged:

```text
"œ̃" is not a single codepoint (ids=149,)
Phonemes must be one codepoint (phoneme id map)
```

The config's offending key is UTF-8 `c5 93 cc 83` (`œ` plus combining tilde),
whereas the v1.13.0 C++ parser requires one Unicode codepoint per map key. This
is an exact artifact/runtime incompatibility, not a transient inference or
network failure. The model README describes this revision as the post-May
MB-iSTFT-VITS2 generation and says it requires a runtime after Piper Plus PR
#320, but the exact v1.13.0 binary still rejects its config. Do not rewrite the
config or substitute a different runtime while calling this candidate exact.

## Compatibility control, not a candidate

For diagnosis only, the previous HF revision
`bf70fae2e21f9670456ebb40e8df131f146f1821` was downloaded and tested with the
same v1.13.0 arm64 bundle. It uses a pre-MB-iSTFT config and is not the current
pinned release model:

- ONNX: 39,414,515 bytes,
  SHA-256 `375694f9a9c24d57ebbccfff7e16b20a3775a926754603f34123744d2e4d9d2a`
- Config: 8,966 bytes,
  SHA-256 `bace7b5559fe5dc5eaf95062ad017c265f15b023ba1ccdb8b53a53b9361af859`
- Model load: 377.122 ms (runtime debug measurement)
- Warmup: 61 ms for two runs
- Generation: 41.252 ms inference, 1.822766 s output including silence,
  RTF 0.0240079
- Whole process: 0.50 s
- Maximum RSS: 348,389,376 bytes
- WAV: 84,640 bytes, mono PCM16, 22,050 Hz, 1.918277 s according to `afinfo`
- WAV SHA-256: `9adfab93815236c59cce59e09a2eb9f956944c082a819ce558c2478a9fe1c346`

This control proves that the arm64 bundle, bundled dictionaries, CPU ONNX
Runtime path, and offline command shape can synthesize English. It does not
clear the current model for release and should not be silently pinned instead.

## Provenance and licensing gate

The model card labels the artifact `other` with `license_name:
css10-public-domain` and points to the CSS10 repository. The [upstream CSS10
repository](https://github.com/Kyubyong/css10) advertises an Apache-2.0
repository license. The model README says it is licensed according to CSS10,
but does not provide a separate model license or an explicit redistribution
grant. Those labels are materially different and cannot be treated as an SPDX
license decision. The training claim also references a 6-language base model
and fine-tuning, so the release review must confirm rights for the resulting
weights, not only the source dataset repository.

Release gate: obtain an authoritative redistribution statement for the exact
`bd0d812` weights and reconcile the Hugging Face `css10-public-domain` label
with the CSS10 repository's Apache-2.0 license. Separately, add the missing
Piper Plus, ONNX Runtime, and third-party notices to any package. Until those
legal and technical gates pass, keep TTS conditional and retain the authored
nonverbal/fallback path.

## Platform gaps

Only macOS arm64 was exercised. The v1.13.0 release lists separate Linux
arm64/x64/armv7 and Windows x64 artifacts, but no Windows arm64 or Linux
portable runtime was tested here. The exact current model/config must be
retested on each supported runtime after the codepoint incompatibility is
resolved. This smoke does not establish Windows Job Object behavior, Linux
library loading, or CPU performance on the project's old x86-64 reference
machine.
