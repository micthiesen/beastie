# Flite 2.2 release-candidate fallback smoke

Status: **blocked for Beastie release.** Flite is a technically viable
Python-free, offline native speech fallback on macOS and likely Linux, but the
current v2.2 source does not provide a maintained native Windows build path for
Beastie's first-class Windows target. Keep this as a bounded fallback candidate,
not a default or shipped runtime, until that platform gate is cleared.

## Exact source and license

- Official source: [festvox/flite](https://github.com/festvox/flite)
- Release tag: `v2.2`
- Tag commit: `e9e2e37c329dbe98bfeb27a1828ef9a71fa84f88`
- Archive: `https://github.com/festvox/flite/archive/refs/tags/v2.2.tar.gz`
- Archive size: 20,233,792 bytes
- Archive SHA-256: `ab1555fe5adc3f99f1d4a1a0eb1596d329fd6d74f1464a0097c81f53c0cf9e5c`
- `COPYING`: 10,022 bytes,
  SHA-256 `d31bceaf2823d56a8c9400f7bde3b17140e739e991eb4f203bdcf9827754ab59`

The upstream `COPYING` calls the core code BSD-like and permits commercial
use, modification, and distribution, with attribution, modification marking,
and no-endorsement conditions. It also enumerates exceptions that must remain
with a redistribution: NAIST/NITECH/CMU MLSA/MLPG/VC code, Cepstral/Cobalt SAPI
code, University of Toronto regex, Sun G72x, Festival/University of Edinburgh
derived models, FSF configuration scripts, and UIUC Apache-2.0 grapheme tables.
This is suitable for the MIT-style Beastie release only if the complete
`COPYING` and required third-party notices ship with the runtime. Do not reduce
the aggregate to an unqualified SPDX identifier.

## macOS build

The source built natively outside the repository on the Apple M5 Max with
Apple Clang 21.0.0 and macOS SDK 26.5:

```sh
./configure
make
```

The build completed serially in about 10 seconds. It produced a Mach-O arm64
CLI with all built-in voices (`kal`, `awb_time`, `kal16`, `awb`, `rms`, `slt`).
There are no external voice files in this build; the selected voice is linked
into the executable.

Stripped artifact measurements:

| Artifact | Bytes | SHA-256 | Output |
|---|---:|---|---|
| all-voice `flite` | 20,289,608 | `6d2eb37819b8b85078cafe4b39629dd2238ced02934ac53a9d9f300a74bd498a` | all built-ins |
| `flite_cmu_us_kal` | 3,194,632 | `b41e1760ba28fda9cc94cc4fdd78336a5911c0a068def3b456f25dc3983df4a7` | 8 kHz diphone |
| `flite_cmu_us_slt` | 4,700,744 | `43497db294d18c2f63bb44e20db4b2716c0d69afafcde99f1e66056989e110ee` | 16 kHz clustergen |

Each binary links only `/usr/lib/libSystem.B.dylib`. The `slt` voice is the
best fallback candidate from this smoke because it produces 16 kHz output and
is smaller than the all-voice binary. The bundled `kal` voice is substantially
faster and smaller but is an intentionally old, robotic 8 kHz diphone voice.

## Offline synthesis proof

Both lines were synthesized with the stripped native binary, with no Python,
network, audio device, or external voice data. `-v` reports synthesis timing;
`/usr/bin/time -l` reports process RSS.

### `slt`, punctuation and profanity

```text
No berries?! You gave me red betrayal; I remember your bullshit.
```

- Synthesis: 0.019989 seconds
- Audio duration: 4.090000 seconds
- RTF: 0.0048878 (204.6x faster than real time)
- Whole process: 0.02 seconds
- Maximum RSS: 8,929,280 bytes
- WAV: 130,924 bytes, mono PCM16, 16,000 Hz, valid RIFF
- WAV SHA-256: `b2a595b1b936d9f605bac0404244079ad066b43d361ee4d4a850ed2c02d92845`

### `kal`, profanity and ellipsis

```text
Little beastie says: shit, fuck, and bullshit... really?!
```

- Synthesis: 0.002847 seconds
- Audio duration: 4.278500 seconds
- RTF: 0.0006654 (1,502.8x faster than real time)
- Whole process: less than 0.01 seconds
- Maximum RSS: 3,555,328 bytes
- WAV: 68,500 bytes, mono PCM16, 8,000 Hz, valid RIFF
- WAV SHA-256: `9047fc2215f50cdeb48d12a1f4378021a587c924d066a53259b3739523f828f1`

The command-line smoke proves Flite accepts the target profanity and unusual
punctuation as ordinary text and emits valid WAV files. It does not constitute
an audio-quality judgment; the upstream README explicitly describes the
included `kal` voice as an old 8 kHz diphone voice. The `slt` output is the
more plausible Beastie fallback, but still sounds much more synthetic than the
Kitten research spike.

## Proposed worker contract

The native C API is sufficient for a persistent worker and avoids spawning a
CLI per utterance:

1. Initialize once with `flite_init()`.
2. Select the statically linked `slt` voice with `flite_voice_select("slt")`.
3. Accept one bounded UTF-8 request at a time and call
   `flite_text_to_wave(text, voice)`.
4. Resample the returned `cst_wave` to 24 kHz with
   `cst_wave_resample(wave, 24000)` so it matches Beastie's current speech
   boundary, then validate mono PCM16 sample count and duration.
5. Copy bounded RIFF bytes into the existing content-addressed cache, return
   only the cache key, and call `delete_wave(wave)`.
6. Keep the existing timeout, stale-request invalidation, authored fallback,
   and process-group containment. Serialize requests until a native adapter
   proves its thread behavior under load.

This contract keeps facts and dialogue authority in Rust, requires no network or
Python, and fits the MVP's TTS model and AI-memory budgets by a wide margin.
The current codebase must not wire it until the Windows build gate is resolved.

## Windows support gate

There is no `CMakeLists.txt` in the v2.2 source. The root `flite.sln` is a
Visual Studio 14-era solution with only Debug/Release `Win32` configurations,
mixes `.vcxproj` and obsolete `.vcproj` projects, and references the missing
`flite/flite.vcxproj`. The root `fliteDll.vcxproj` itself points at that missing
project. Its x64 entries do not make the solution a working x64 release path.

The `windows/Makefile` is explicitly labelled Windows Mobile and says it builds
DLLs with MinGW. The README's supported Windows entries are historical Cygwin
and Windows Subsystem for Linux paths, and its native Visual Studio notes refer
to old SAPI/Visual C++ workflows. SAPI projects exist, but they are not a
maintained Beastie CLI/library build path and require legacy SAPI SDK setup.

Therefore this smoke does **not** accept Cygwin, WSL, or MinGW as proof of
Beastie's native Windows target. A release candidate needs either a maintained
MSVC project that builds the same C API/voice on current Windows x64, or an
explicitly supported native Windows toolchain with a PE artifact and acceptance
test. Until then, Flite remains blocked despite the strong macOS measurements.
