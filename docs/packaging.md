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
  --espeak-source /path/to/espeak-ng-1.52.0.tar.gz \
  --stt-worker target/release/beastie-stt \
  --stt-model-dir /path/to/parakeet-tdt-0.6b-v3-int8 \
  --stt-model-license /path/to/CC-BY-4.0-legalcode.txt \
  --stt-model-card /path/to/parakeet-model-README.md
cargo xtask package --platform macos --destination dist --check
```

`--runtime` is repeatable. The list must contain `llama-server` (`llama-server.exe` on Windows)
and every dynamic library it needs. Package inputs may use only sibling-resolving runtime symlinks;
asset symlinks and runtime symlinks that escape their source directory fail closed.

Release packages require complete TTS and STT inputs. Supplying only part of either bundle fails
before staging. A developer package may omit either bundle only by passing
`--development-package` during staging and checking. This opt-out is never valid for a release.
The TTS inputs must come from the same eSpeak NG distribution so the
executable, data, GPLv3 license, and corresponding source remain auditable as one separately
distributed component.

The package layout is deliberately explicit:

```text
dist/
  macos/
    beastie
    beastie-ai-worker
    beastie-tts              # external-process JSONL TTS adapter
    beastie-stt              # bounded persistent JSONL recognition worker
    runtime/                 # local inference and TTS runtimes
      llama-server
      espeak-ng
      espeak-ng-data/**
      espeak-ng-COPYING
      espeak-ng-1.52.0.tar.gz
    assets/manifest.toml
    assets/generated/**
    assets/final/**          # when promoted assets exist
    assets/licenses/**       # bundled font and asset license texts
    models/manifest.toml
    models/Qwen3.5-0.8B-Q4_0.gguf
    models/LICENSE
    models/README.md
    models/parakeet-tdt-0.6b-v3-int8/
      config.json
      decoder_joint-model.int8.onnx
      encoder-model.int8.onnx
      nemo128.onnx
      vocab.txt
      LICENSE
      README.md
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

The packaged game discovers sibling `beastie-stt` and
`models/parakeet-tdt-0.6b-v3-int8` relative to itself. Parakeet inference is embedded in the worker,
so no separate STT engine or Homebrew path is needed. The selected model is exactly five files and
670,619,803 bytes. Every component size and SHA-256 comes from `models/manifest.toml`. Missing,
partial, tampered, symlinked, or unexpected contents fail closed. The CC-BY-4.0 license and model
card are mandatory. Moonshine fallback packages may still supply the optional external STT engine,
runtime license, and notices as a complete group.

The macOS app declares `NSMicrophoneUsageDescription` with the bounded push-to-talk purpose before
signing. The game requests audio only after the player explicitly enables the microphone and begins
a push-to-talk capture. Permission denial leaves text and the rest of the simulation available.

The resulting manifest is suitable for release evidence: it records the platform, an explicit
`network = false` assertion, installed bytes and budget, and each staged file's byte count and
SHA-256. It does not embed or fetch model weights. A missing model or runtime is a hard failure,
not a fallback to a package that cannot run locally.

## Installer and release artifacts

The staging directory is the only input to platform packaging. No installer script downloads a
model or runtime. Build the platform package first, run `--check`, then invoke the matching wrapper:
The first-run discovery and user-data boundary is recorded in [`packaging/README.md`](../packaging/README.md).

```sh
packaging/macos/build-app.sh --package-root dist/macos --output dist/Beastie.app
packaging/macos/build-dmg.sh --app dist/Beastie.app --output dist/Beastie-macos.dmg
packaging/linux/build-appimage.sh --package-root dist/linux --output dist/Beastie-linux.tar.gz
# On Windows, compile packaging/windows/Beastie.iss with Inno Setup 6.
```

Without `--sign`, the macOS app receives an ad hoc hardened-runtime signature so its bundle
resources and nested executables can be checked as one internally consistent artifact. Passing
`--sign <Developer ID identity>` replaces that with the timestamped release signature required for
notarization. A real Developer ID certificate, Apple team, notarization credentials, and a
notarization submission are external release prerequisites. The Windows Inno
configuration and Linux AppImage wrapper are checked in and consume the same sibling layout. Linux
falls back to a `.tar.gz` equivalent when `appimagetool` is not installed.
The tar wrapper targets an x86-64 desktop installation rather than a minimal server image. It
includes `README-LINUX.txt` with the launch command and baseline X11/Wayland, Vulkan/Mesa, ALSA,
udev, and xkbcommon-x11 requirements. Ubuntu 24.04 Desktop supplies that baseline; a minimal
Ubuntu image needs the packages listed in the bundled README before native launch validation.

`packaging/windows/validate-layout.ps1` and `packaging/linux/validate-layout.sh` validate extracted
artifacts. Their optional launch smoke is only a native-runner proof. CI config is not evidence of
Windows or Linux runtime behavior from a Mac.

All wrapper outputs refuse to replace an existing caller-selected artifact unless `--force` is
passed explicitly. The macOS wrapper ad hoc signs local builds, verifies the complete bundle before
DMG creation, and verifies the resulting disk image. Developer ID signing changes executable bytes,
so the staged package manifest remains pre-sign provenance; final distribution identity comes from
the bundle signature plus the retained DMG SHA-256.

## Steam

Steam configuration deliberately uses placeholders. `steam/config.example.env` has zero IDs and
must never be used to publish. `steam/validate-config.sh --mode release --config <private-env>`
rejects placeholders and requires all three depot IDs plus an executable `steamcmd`. The checked-in
`steam/achievements.json` and `steam/input/beastie-gamepad.json` are deterministic design inputs;
the renderer script sorts and validates achievement IDs before a build. `steam/build.sh` prepares
preview VDF/depot files from already staged platform packages and only invokes SteamCMD in release
mode.

Steam App IDs, depot IDs, branch names, account credentials, and signing identities are intentionally
not present in this repository. A release operator must supply them through a private environment.
The store page should use the screenshot/trailer plan in `docs/acceptance/distribution-checklist.md`:
capture the aquarium idle, feeding/rejection, expression, and reunion beats at native integer scale;
the trailer must show deterministic interaction and disclose that dialogue and speech run locally.

## CI and acceptance evidence

`.github/workflows/release.yml` builds all three targets, validates the checked-in wrapper/config
files, and retains package/layout evidence. Native launch jobs must use a real staged package supplied
by protected release inputs. The workflow never fabricates a model or claims a cross-platform launch
from macOS. Follow [the dated checklist](acceptance/distribution-checklist.md) and attach manifests,
installer hashes, launch logs, save-path checks, and child-process cleanup evidence to the release.

The macOS proof used the official llama.cpp `b10310` ARM64 archive, 10,982,134 bytes with SHA-256
`fdec9afcb2ee389dee74cc7c5f86c7f270b0f88fb248f0a7d1e5956fa61f100b`. The final acceptance bundle
contains release Beastie binaries, the real 563 MB Qwen model, all authored assets, the minimal
llama.cpp runtime closure, eSpeak NG, compiled voice data, licenses, and the exact eSpeak source
archive. The final V1 stage checks at 473 files and 666,289,145 bytes. Its ad hoc signed DMG has
SHA-256 `418698f2c905bd360ff7033fc593246e94d05e5a05b383244bcc6cc982152271` and passes
`hdiutil verify`. With no AI or TTS environment variables, the staged worker served two grounded
requests through one packaged warm server and packaged eSpeak generated a validated cache entry;
every child exited afterward. The immediately preceding bundle of the same game shell completed
its three-frame GUI smoke. The final GUI retry was stopped after the shared remote daemon again
failed to grant the window a drawable, so it is not recorded as a second GUI pass. This proves the
final staged runtime does not depend on Homebrew paths or an installed model while keeping the GUI
evidence boundary honest.
