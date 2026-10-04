# Audio direction

Beastie's aquarium sounds intimate, submerged, slightly grotty, and alive. The mix is deliberately
small and dry enough that dialogue remains the focus. UI sounds stay crisp without resembling a
phone notification. Creature noises sound like a peculiar electronic animal, not speech and not a
human performing an animal voice. TTS is a separate replaceable layer.

All authored files are mono, 44.1 kHz, signed 16-bit PCM WAV. One-shots include their own short
fades. `environment/underwater-loop` is exactly 12 seconds and phase-locked at its boundary.

## V1 event map

| Audio ID | Duration | Peak | RMS | Authoritative trigger |
|---|---:|---:|---:|---|
| `environment/underwater-loop` | 12.000 s | -17.00 dBFS | -25.92 dBFS | Continuous aquarium bed while the habitat is visible |
| `environment/bubbles-1` | 0.820 s | -10.50 dBFS | -20.27 dBFS | Ambient bubble emitter, variant chosen deterministically |
| `environment/bubbles-2` | 1.080 s | -11.00 dBFS | -21.70 dBFS | Ambient bubble emitter, variant chosen deterministically |
| `movement/swim-wake` | 0.720 s | -12.50 dBFS | -21.24 dBFS | A new purposeful swim or dash begins, never every movement tick |
| `food/drop-sink` | 0.620 s | -7.50 dBFS | -18.46 dBFS | A physical food object is accepted into the aquarium |
| `food/eat` | 0.480 s | -6.50 dBFS | -16.68 dBFS | The simulation resolves food as eaten |
| `food/spit-reject` | 0.560 s | -6.80 dBFS | -15.35 dBFS | The simulation resolves food as tasted then rejected |
| `environment/sand-disturb` | 0.780 s | -13.00 dBFS | -24.55 dBFS | Creature or object makes meaningful contact with the substrate |
| `environment/cave-settle` | 0.680 s | -10.00 dBFS | -21.39 dBFS | A familiar-cave relationship beat reaches its authoritative Act phase |
| `object/toy-impact` | 0.340 s | -8.50 dBFS | -18.37 dBFS | A legacy play outcome without a paired specific object response |
| `object/ball-nudge` | 0.550 s | -9.40 dBFS | -18.30 dBFS | Accepted direct or autonomous ball contact commits the ball's impulse |
| `object/bell-ring` | 0.820 s | -10.40 dBFS | -20.20 dBFS | Accepted direct or autonomous bell contact commits its strike |
| `object/sock-rustle` | 0.480 s | -9.10 dBFS | -25.10 dBFS | Accepted direct or autonomous sock contact begins its owned tug |
| `creature/affection` | 0.640 s | -7.50 dBFS | -16.98 dBFS | Positive touch, comfort, or attachment response |
| `creature/surprise` | 0.420 s | -7.20 dBFS | -15.79 dBFS | Startle or high-salience unexpected event |
| `creature/curious` | 0.580 s | -8.50 dBFS | -15.38 dBFS | Investigation begins or a novel object earns attention |
| `creature/sad` | 0.860 s | -10.00 dBFS | -14.75 dBFS | Brief low-valence nonverbal response, not a neglect alarm |
| `creature/wake` | 0.460 s | -9.00 dBFS | -17.55 dBFS | Sleep ends and the creature becomes responsive |
| `creature/mrr` | 0.440 s | -4.59 dBFS | -13.01 dBFS | Contented acknowledgment or neutral nonverbal dialogue fallback |
| `creature/annoyed` | 0.460 s | -4.33 dBFS | -11.69 dBFS | Annoyance event or direct rejection without a spit action |
| `creature/sleep` | 1.100 s | -7.12 dBFS | -16.37 dBFS | Sleep starts, once rather than looped every tick |
| `ui/select` | 0.105 s | -10.03 dBFS | -15.23 dBFS | Focus moves to a new enabled hit region |
| `ui/confirm` | 0.180 s | -8.76 dBFS | -14.50 dBFS | A semantic player command is accepted |

The simulation or session layer chooses semantic events. Rendering may place or attenuate them,
but must not infer that an interaction succeeded. Ambient bubble variants are selected from a
seeded presentation stream, with 6 to 18 seconds between bursts and no run longer than two of the
same variant, so captures remain reproducible without exposing a clock-like alternation.
Ordinary autonomous cave shelter can remain quiet. The cave cue punctuates a salient familiar-place
relationship beat; it is not a completion chime for every private rest. This distinction preserves
quiet observation rather than turning autonomous activity into a stream of notifications.
Direct toy responses use the exact toy interaction owner, separate from private-life activity
IDs. Each physical response has one specific sound; its paired play outcome retains delight
without stacking another generic impact. Deferred speech cannot undo a committed response.

## Mix and layering

- Treat `environment/underwater-loop` as the reference bed at 0.35 linear gain. Loop it gaplessly,
  without re-decoding or creating a fresh player at each boundary.
- Play ordinary aquarium one-shots at 0.40 to 0.55. Keep `movement/swim-wake` at or below 0.38 and
  rate-limit it to one start cue per 700 ms so continuous motion does not become noise.
- Let no more than two bubble one-shots overlap. If a third starts, discard the oldest or quietest.
- Decorative bubbles, UI and physical cues do not duck the bed. On creature vocalization, duck the ambience by 4 dB with a 35 ms attack and 180 ms release. On
  TTS, duck ambience by 7 dB and other creature cues by 5 dB, with a 45 ms attack and 260 ms release. These linear envelopes complete within their
  stated duration; repeated updates do not restart them. Physical and UI cues retain their gain
  during speech, so contact and direct input remain legible.
- Do not stack `food/spit-reject` with `creature/annoyed` at full level. Prefer the physical cue,
  then optionally play annoyance after 180 ms at half gain. Dialogue owns the loudness ceiling.

Missing assets, invalid decodes, an unavailable output device, and a failed loop must remain
non-fatal. The game stays fully playable in silence.

## Provenance and synthesis

Every file was authored for Beastie on 2026-08-15 by a deterministic local Python PCM synthesizer.
The V1 batch uses additive sine harmonics, sample-integrated chirps, shaped envelopes, and seeded
pseudo-random noise. The ambience uses only phase-locked integer-cycle oscillators, including its
bubble glints, so it repeats without a crossfade or copyrighted sample. The generator was a
temporary development tool and is not a runtime dependency. Existing MVP cues use the same local
synthesis approach. No third-party samples, model output, online service, or restricted source
material was used.

| File below `assets/generated/audio/` | SHA-256 |
|---|---|
| `creature/affection.wav` | `e0cf426b84346681ec7b2bc7ef716f6b6a0e4eb570fbcd901b147bc6478eff79` |
| `creature/annoyed.wav` | `3f7ce97ef33a9e168bb6500016bc80cd07ecd84428484c3cb0e3b0fdadadfa6e` |
| `creature/curious.wav` | `200dbbeba0bb09ecd73d97a08738e142f6d38849a8e1ebbd3fa29a24438be43a` |
| `creature/learned.wav` | `128f22ea8eec30cf132421847dfa3e8584b8af62176af8f9bc047bcca0ff6e16` |
| `creature/mrr.wav` | `da92b76f9684db6e0afa0efa721d7f957d2a738aa82cdb58ef366ae4cc0124a1` |
| `creature/sad.wav` | `32ffc0531c6673e778e6855bc01652822708dea22710c2295ef92f1565b9f079` |
| `creature/sleep.wav` | `b76720fac4545a0e45bf3257e10a668ae92fb40f15dbef4c6c2f0e471f9deeea` |
| `creature/surprise.wav` | `edbc5eb64916d38121ea47340c263647d327c624b10be0578609c6fd685ca69c` |
| `creature/wake.wav` | `e80856be26b1a646b657527fc103c9feb30377277765f1d4a63698eb6ed83c59` |
| `environment/bubbles-1.wav` | `b8942a78cce5b559e17a9ca51c43a18ecfce4e5cf7323ee5745d5d70f2ba0039` |
| `environment/bubbles-2.wav` | `56eb6bbff01bde5b581acb1bbea30806922182ee651cdc128ddf5c0b56b7f0f1` |
| `environment/cave-settle.wav` | `f7b6ab7a4a449455e1b3e6798ee49d113b71565ada8bcf7eaa57d502ef80f77a` |
| `environment/sand-disturb.wav` | `376d198de6423d85dd9185e630abfdbc728de6e6d52131040a501e5b973e5194` |
| `environment/underwater-loop.wav` | `05a5d3ebdddfa97d9173cbb6cff64fb65edea27d5602fb909bc42a58fb16c053` |
| `food/drop-sink.wav` | `ddde1ebc9140bdf4457c993c023eb46e72da3fee0699ee45a682d3148d046667` |
| `food/eat.wav` | `c30519619fd2fe437f69a177a22f2aa961d34698a83a2b10d2f07b97e2dcd40a` |
| `food/spit-reject.wav` | `fb512e71ab8d6403fa866dd87fea98dadc0e976f51f7621dba2a28aab38b535c` |
| `movement/swim-wake.wav` | `0f0ed6e2b4c41a8ae13df0cf2985f9a27cd10509703dc6e8d8e773aa46451de8` |
| `object/toy-impact.wav` | `1a6ac2f71c72bde130149ee02e64b04116332ebab6244d08fcd96ec50119a649` |
| `object/ball-nudge.wav` | `66afa878f3d37707bf5fd0915ef2d4855378c77840387c1613e4333a054349fa` |
| `object/bell-ring.wav` | `b3b0a055f01ef15381faf5061b0d687a4a14682a6a8eb469fb428016878273a9` |
| `object/sock-rustle.wav` | `73df5576081331ca3528727f5024006df174fed0114b2c35c0dd1a5734bf04e4` |
| `ui/confirm.wav` | `92a6d1e8504be4a2c38757f4f893235689bc771a5955d0f130685b26ff369c1a` |
| `ui/select.wav` | `2c57986e0d9988304fd4abb96976db265d084dd33cff24d158b117136dad66aa` |

`creature/learned.wav` was added on 2026-10-04 for the fun rework: a rising three-note bell
figure (C6, E6, A6) over a short upward chirp, about 0.9 s, peaking near -8.4 dBFS. It plays when
the creature learns a word. Its generator is retained at `tools/audio/synth_learned.py` and
reproduces the recorded hash exactly.

## Validation

FFmpeg decoded all 20 files without error. An independent PCM pass confirmed one channel, 44,100
Hz sample rate, 16-bit sample width, finite non-empty duration, no clipped samples, and absolute DC
offset below 0.002 for every cue. Peaks range from -17.00 to -4.33 dBFS, leaving mix headroom. The
ambience loop's last-to-first sample delta is 0.000061 full scale and its boundary slope mismatch is
below 0.000001 full scale, both safely below audibility.

## Runtime integration

The Bevy shell uses rodio directly, opening its default output sink opportunistically. Failure to
open it leaves the game playable in silence. Resolve each cue through `assets/final/audio/<id>.wav`,
then `assets/generated/audio/<id>.wav`; validate and cache encoded bytes at startup. Each accepted
playback decodes those cached bytes into a retained player. Set gain before appending the source.
The retained output sink outlives its players. Continuous ambience uses a gapless repeating decoder.

Mix priority uses semantic cue roles, independently of ownership/cancellation channels. Only
accepted, active and unmuted creature/speech sources affect ducking. Muted, missing, rejected and
cancelled sources cannot keep the bed quiet. At most two bubble sources overlap; a third retires
the oldest. Source ownership still governs cancellation, and a direct outcome replaces lower
priority creature voice without cancelling unrelated physical contact sounds.

## Recorded playback reconstruction

Feel captures retain post-arbitration playback snapshots and decisions in `audio.jsonl`, including
source/playback IDs, semantic and speech owners, settings, effective gain, looping, output
availability and asset SHA-256. The exact resolved source bytes are retained as
`audio-<sha256>.wav` and included in the manifest. Speech lifecycle records distinguish accepted
TTS bytes from a playback that actually starts; unavailable output does not invent active speech.

`reference-mix.wav` reconstructs active playback intervals and piecewise-constant frame gains from
these records. Cancellation truncates a source, mute produces silence, and offline source
resolution verifies retained hashes. It does not re-resolve current repository assets or synthesize
sounds from discarded cue requests. `audio-reconstruction.json` records the pre-limiter peak and
clipping sample count. No limiter conceals gain errors: clipping rejects the evidence. Start
metadata and bytes survive even if a source ends before a snapshot. Such an unrepresented playback
rejects frame-clock reconstruction explicitly, because its duration cannot safely be inferred.

This is a frame-clock playback reconstruction, not captured host audio. It reproduces recorded
start/stop/gain decisions; device buffering, hardware latency and perceived voice character still
require host listening. An unavailable-output capture reconstructs silence and records that fact.
Legacy cue-only captures remain historical review evidence but require recapture for this renderer.
