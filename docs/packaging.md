# Offline release packaging

Stage 5 packaging is intentionally a staging check, not a downloader. Build the game and the
warm local inference runtime first, then pass those paths to `cargo xtask package`. The
builder copies only those files and repository-owned manifests into a fresh platform directory.
It never opens a network connection and never deletes an existing directory.

```sh
cargo xtask package \
  --platform macos \
  --destination dist \
  --game target/release/beastie-game \
  --worker target/release/beastie-ai-worker \
  --model models/Qwen3.5-0.8B-Q4_0.gguf \
  --model-license /path/to/Qwen3.5-0.8B-LICENSE \
  --model-card /path/to/Qwen3.5-0.8B-README.md \
  --runtime /path/to/llama-server \
  --runtime /path/to/each-required-runtime-library \
  --tts-worker target/release/beastie-tts \
  --espeak /path/to/espeak-ng \
  --espeak-data /path/to/espeak-ng-data \
  --espeak-license /path/to/espeak-ng-COPYING \
  --espeak-source /path/to/espeak-ng-1.52.0.tar.gz
cargo xtask package --platform macos --destination dist --check
```

`--runtime` is repeatable. The list must contain `llama-server` (`llama-server.exe` on Windows)
and every dynamic library it needs. Package inputs may use only sibling-resolving runtime symlinks;
asset symlinks and runtime symlinks that escape their source directory fail closed.

Release packages require all five TTS inputs. Supplying only some of them fails before staging.
An intentionally silent developer package must opt in with `--development-package` during both
staging and checking. The TTS inputs must come from the same eSpeak NG distribution so the
executable, data, GPLv3 license, and corresponding source remain auditable as one separately
distributed component.

The package layout is deliberately explicit:

```text
dist/
  macos/
    beastie
    beastie-ai-worker
    beastie-tts              # external-process JSONL TTS adapter
    runtime/                 # local inference and TTS runtimes
      llama-server
      espeak-ng
      espeak-ng-data/**
      espeak-ng-COPYING
      espeak-ng-1.52.0.tar.gz
    assets/manifest.toml
    assets/generated/**
    assets/final/**          # when promoted assets exist
    models/manifest.toml
    models/Qwen3.5-0.8B-Q4_0.gguf
    models/LICENSE
    models/README.md
    THIRD_PARTY_NOTICES
    LICENSE
    package-manifest.json    # bytes and SHA-256 for every file above
  windows/                   # beastie.exe, beastie-ai-worker.exe, same subdirectories
  linux/                     # beastie, beastie-ai-worker, same subdirectories
```

Windows and Linux use the same worker protocol and manifest layout. Their platform-specific
executable names are the only required naming difference. macOS is the current packaging target;
the runtime list remains explicit because `llama-server` may depend on `libllama-server-impl`,
`libllama`, `ggml` libraries, and platform crypto libraries rather than being self-contained.

The selected dialogue model is `qwen3.5-0.8b-q4_0`. Packaging fails before staging if its local
GGUF is absent, its byte count or SHA-256 differs from `models/manifest.toml`, or its supplied
license and model-card snapshots do not match their pinned SHA-256 values. The manifest links the
official Qwen source revision `2fc06364715b967f1860aea9cf38778875588b17` to ggml-org's
quantization revision `8fea620810c4afa23dd6443f999a48574c1611a3`; that quantization's
`.src_sha` names the same source revision. Packaging also fails if any game,
worker, model manifest, asset manifest, runtime library, or third-party notice is missing. The
installed-size budget is 1,500,000,000 bytes. A package with a stale or unexpected file fails its
manifest check.

When all five TTS inputs are present, the packaged game discovers the sibling `beastie-tts`
automatically, points it at `runtime/espeak-ng` and `runtime/espeak-ng-data`, and stores generated
WAVs under the user's Beastie configuration directory. Development builds remain opt-in through
`cargo xtask dev --tts-cache-dir ...`. A package with any partial TTS bundle fails its check.

eSpeak NG remains GPLv3 software distributed in aggregate beside MIT-licensed Beastie. Release
assembly must preserve `runtime/espeak-ng-COPYING` and satisfy GPLv3 section 6 for the exact binary
and data shipped, including complete corresponding source or a valid written source offer. The
experimental statically linked sherpa-onnx backend and Kitten model remain prohibited release
inputs.

Every eSpeak-enabled package includes the exact eSpeak NG 1.52.0 source archive. Its byte count and
SHA-256 are enforced by the package builder and recorded in `THIRD_PARTY_NOTICES`.

The resulting manifest is suitable for release evidence: it records the platform, an explicit
`network = false` assertion, installed bytes and budget, and each staged file's byte count and
SHA-256. It does not embed or fetch model weights. A missing model or runtime is a hard failure,
not a fallback to a package that cannot run locally.

The macOS proof used the official llama.cpp `b10310` ARM64 archive, 10,982,134 bytes with SHA-256
`fdec9afcb2ee389dee74cc7c5f86c7f270b0f88fb248f0a7d1e5956fa61f100b`. The final acceptance bundle
contains release Beastie binaries, the real 563 MB Qwen model, all authored assets, the minimal
llama.cpp runtime closure, eSpeak NG, compiled voice data, licenses, and the exact eSpeak source
archive. It checks at 408 files and 633,013,119 bytes. Launched with no AI or TTS environment
variables, it produced the complete five-capture three-day scenario and four validated speech cache
entries, then reaped every child process. This proves the staged bundle does not depend on Homebrew
paths or an installed model.
