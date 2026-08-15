# Audio direction

The MVP cue set is deliberately tiny, dry, and synthetic. UI sounds should feel crisp without
resembling a phone notification. Creature noises should sound like a peculiar electronic animal,
not speech and not a human performing an animal voice. TTS remains a separate replaceable layer.

## Authored cues

All files are mono, 44.1 kHz, signed 16-bit PCM WAV. Their short fades make them safe to trigger
without added runtime envelopes.

| Audio event | Generated asset | Duration | Peak | RMS | Intended trigger |
|---|---|---:|---:|---:|---|
| `ui/select` | `generated/audio/ui/select.wav` | 0.105 s | -10.03 dBFS | -15.23 dBFS | Focus moves to a new enabled hit region |
| `ui/confirm` | `generated/audio/ui/confirm.wav` | 0.180 s | -8.76 dBFS | -14.50 dBFS | A semantic player command is accepted |
| `creature/mrr` | `generated/audio/creature/mrr.wav` | 0.440 s | -4.59 dBFS | -13.01 dBFS | Contented acknowledgment, comfort, or nonverbal dialogue fallback |
| `creature/annoyed` | `generated/audio/creature/annoyed.wav` | 0.460 s | -4.33 dBFS | -11.69 dBFS | `FoodRejected`, `ToyRejected`, or an annoyed nonverbal act |
| `creature/sleep` | `generated/audio/creature/sleep.wav` | 1.100 s | -7.12 dBFS | -16.37 dBFS | `SleepStarted`, once rather than looped every tick |

Do not emit selection audio from render planning every frame. Emit it only when the focused region
changes. Confirm belongs to accepted input, not pointer-down. Creature cues belong to authoritative
game events. If speech and a creature cue coincide, let the cue finish or duck it before TTS rather
than stacking both at full volume.

## Provenance

These sounds were authored for Beastie on 2026-08-15 with a deterministic local PCM synthesizer
using sine harmonics, envelopes, and seeded pseudo-random noise. No third-party samples, model
output, online service, or restricted source material was used. The generator was a temporary
development tool and is not a runtime dependency.

| File | SHA-256 |
|---|---|
| `creature/annoyed.wav` | `3f7ce97ef33a9e168bb6500016bc80cd07ecd84428484c3cb0e3b0fdadadfa6e` |
| `creature/mrr.wav` | `da92b76f9684db6e0afa0efa721d7f957d2a738aa82cdb58ef366ae4cc0124a1` |
| `creature/sleep.wav` | `b76720fac4545a0e45bf3257e10a668ae92fb40f15dbef4c6c2f0e471f9deeea` |
| `ui/confirm.wav` | `92a6d1e8504be4a2c38757f4f893235689bc771a5955d0f130685b26ff369c1a` |
| `ui/select.wav` | `2c57986e0d9988304fd4abb96976db265d084dd33cff24d158b117136dad66aa` |

Validation decoded every file with FFmpeg and independently checked headers and PCM samples. All
five files have one channel, 44,100 Hz sample rate, 16-bit sample width, finite duration, no clipped
samples, and absolute DC offset below 0.002.

## Runtime integration

The game disables ggez's compile-time audio feature because ggez treats failure to open the default
output device as a fatal context error. It uses rodio directly instead, opening the default output
sink opportunistically. If no sink exists, the rest of the game still starts and queued cues are
discarded.

Resolve each event through `assets/final/audio/<id>.wav`, then
`assets/generated/audio/<id>.wav`. Decode and cache valid bytes once at startup. Each trigger creates
a detached one-shot player on the retained mixer so short cues may overlap:

```rust
use std::io::Cursor;
use rodio::{DeviceSinkBuilder, play};

let output = DeviceSinkBuilder::open_default_sink()?;
let player = play(output.mixer(), Cursor::new(wav_bytes))?;
player.set_volume(0.32);
player.detach();
```

The output sink must outlive every player. Missing or invalid assets, decode errors, playback errors,
and an unavailable output device are all non-fatal, consistent with the game's fallback invariant.
