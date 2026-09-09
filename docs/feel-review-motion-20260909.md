# Animation and movement review

Date: 2026-09-09. Automatic fixes and iteration authorized by the user.
Status: implementation and review complete. Contract: [motion continuity](motion-continuity.md).

## Findings

### M1: brief movement bursts around travel changes

- Valid baseline: `target/feel/motion-before-20260909/quiet-observation`, seed 42,
  normal debug Metal at 1280x720. Same original binary retained at
  `/tmp/beastie-motion-before-20260909` for the interaction baseline under
  `target/feel/motion-interaction-before-20260909/interaction-chain`.
- Dense native frames at 65.000–65.367 show a rapid arrival correction followed by stillness.
  At 157.000–157.367 leaving the cave produces fast translation and tail rotation.
- Cause: prediction continues the previous tick's velocity, but the next authoritative position
  uses the new velocity. The old correction ceiling was 12 world units/s, compared with normal
  approach around 0.9. Prediction could also pass its target and correct backward at arrival.
- Severity: medium creature-continuity defect. Acceptance: endpoint-bounded prediction, bounded
  correction at creature pace, and native contact remains aligned with authoritative payoff.

### M2: body waves jump when swim speed changes

- Code evidence: `time * (3 + speed * 3)` makes frequency changes also change phase. A normal
  acceleration frame at 600 seconds can advance phase by 102.8 radians instead of about 0.063.
  Smoothing speed alone does not resolve the multiplying session age.
- Severity: medium animation-continuity defect. Acceptance: integrate frequency over frame delta;
  test late-session starts/stops with actual rendered body transforms.

### M3: stationary phases retain travel velocity

- Interaction baseline retains y velocity 35 in food Inspect/Act/Recover while position is fixed.
  This particular case is small, around0.016 world units/s; do not present it as the major burst.
  Sleep and existing saved actions can retain larger stale values.
- Cause: stationary simulation branches skip movement without clearing velocity, while rendering
  extrapolates it each second. Acceptance: clear it on input-only and fixed-tick boundaries,
  protect existing saved stationary scenes, preserve full state/event equivalence for bulk ticks.

### M4: supporting pose changes snap

- First candidate's rendered trace flags brow rotations of 15.53–16.63 rad/s at 1.016, 3.016 and 25.016s.
  These are recipe changes, not body travel. Whole nod/tilt/fin recipes also changed discontinuously
  on entry/exit, especially private activity ownership changes.
- Severity: low animation craft defect. Acceptance: brief continuous supporting gestures and
  brow/smile transitions; preserve blink timing, current speech aperture and immediate cancellation.

## Iteration findings

The first correction exposed two issues during independent code review and focused testing:
clearing velocity only at the end of a bulk step broke chunk invariance after food recovery;
and capping animation delta at 100 ms discarded travel time on slow host frames. The candidate
clears velocity at every tick boundary and consumes up to the host's full 250 ms delta with bounded
body integration substeps. Tests reproduce both failures.

The first native diagnostic also reported tiny rotations on identical zero-time frames.
`Quat::angle_between` amplified normalization roundoff through acos. Relative-quaternion atan2
measurement now handles identical/sign-equivalent transforms and tiny actual changes correctly;
thresholds were not raised to hide the false positives.

## Evidence limits and non-findings

Native video is retained in full. Agent visual assessment uses sampled and dense sequential frames;
full-speed human perception and physical listening are not claimed. Fixed-rate capture does not
prove native 60 fps performance. This pass does not certify other operating systems or GPUs.
No material boundary snap is established by the baseline; hard-boundary constraints remain under
review through rendered part measurements. Existing layout, creature identity, offline failure
behavior and sparse quiet life remain the baseline, with no new product system requested.

## Candidate comparisons

`motion-after-01-20260909` is the first complete candidate. Its head ceiling was still 1.9 units/s
for all travel and its eyebrows were not yet blended. Dense 65.000–65.367 samples remove the old
rightward arrival correction. At 157.000–159.100, departure and turning progress gradually with
body length and readable face orientation retained. Its diagnostic brow alerts led to M4's final
short settle; the false zero-time quaternion alerts led to the measurement correction above.

`motion-final-quiet-20260909` is the second candidate, before the last saved-state/hour-clock
edge repairs. It records 10,812 frames, no clock discontinuities, a peak head speed of 1.057547
world units/s, and peak part rotation of 5.462959 radians/s. All 212 authoritative events and
15 playback-start records match the baseline in value, order and simulation time. Its reconstructed
sound peak is 0.172722, with no clipping. These are playback reconstructions, not captured host sound.

Two tail translation alerts remain at 99.066 (8.163062 units/s) and 157.783 (8.065184 units/s).
Dense consecutive frames show continuous arcs around a stable head, without a disconnected joint,
reversal or one-frame pose break. The first is partly occluded by the bell. Independent review
rejects slowing all turns merely to eliminate these two alerts; thresholds remain unchanged.
No material hard-boundary jump was found in these recordings. This does not assert that every
possible saved placement or future travel route has been visually inspected.

`motion-final-interaction-20260909` adds interaction-chain and dialogue-races. The interaction
pass retains all 118 authoritative events and 17 playback-start records. The reference peak changes
from 0.424320 to 0.454733 due to playback-frame alignment, with no clipping or output loss. Peak
head speed is 1.057552 units/s. The 17.033 highlight alert is a gaze/expression change and normal
blink with the glint still attached to its eye. Seven zero-time alerts reflect immediate pupil or
mouth changes on semantic inputs; dense 17, 27 and 65-second samples establish no rogue highlight,
body pop, refusal regression or delayed speech cancellation. Do not turn these semantic responses
into delayed animation solely to make every diagnostic count zero.

The last code review also reproduced an old saved Recover action that could finish before its
stale velocity was cleared. Normalization now happens before input replacement, after input, and
at every fixed-tick boundary. A real save round trip and 1000-versus-500+500ms regression cover it.
Fin and tail oscillators now use a bounded integrated phase too, avoiding the former one-hour
clock wrap; a rendered fin test crosses that boundary. No new migration or save version is needed.

The original before executable is
`bc5e359b71f40c71dbc7c57df4f6b19aa06af5532452b7d92a86447303430208` in both baseline
manifests. They correctly report dirty source because implementation began while the unchanged
executable recorded. The copied before executable has the same hash. Intermediate source edits
and rebuilds are not silently treated as identical binaries; each experience manifest records its
executable. The final retained executable is
`bb095dfd53844904e5d765559ee9d231c32632d624d39cb089ba931949cce4fa`.

Independent final code review finds no further actionable defect. Both saved-state chunking
reproductions pass. In the interaction trace, food consumption occurs at 14.033 seconds, with
rendered head within 0.0001 world units of its authoritative anchor; sock response occurs at
38.233 seconds. Comfort at 65.316 seconds clears dialogue, caption and mouth ownership in the same
frame. All original headless assertions, picking tests and speech cancellation tests remain green.

## Final acceptance

The retained executable's two final valid native experiences are
`target/feel/motion-verified-quiet-20260909/quiet-observation` and
`target/feel/motion-final-interaction-20260909/dialogue-races`. Both manifests identify the final
`bb095df…` binary. The quiet recapture deliberately uses the fixed executable copy so later normal
builds cannot change its evidence identity. Both pass complete-video visibility, input isolation,
recording completeness and artifact hashing checks.

Final quiet review covers the three-minute sequence and dense arrival/departure windows. Its
10,812 frame measurements reproduce the second candidate's head peak and exactly the same two
adjudicated tail alerts. All 212 events, 15 playback starts and the unclipped reference peak match
the original baseline. Dialogue-races contributes 2,179 frames, peak head speed 1.057548, no speed
or rotation alerts, and one immediate eye-retarget alert at 1.200 seconds. There is no known
material movement or animation defect left in the reviewed experience. Full-speed human
perception and other-host performance remain the explicit limits above.

The required `cargo xtask dev --fake-ai` path completed a bounded comfort/play script and produced
`target/captures/motion-dev-final-20260909/motion-dev-final.png` at 1920x1080. Native output is
visible and the creature, toys, body shape and controls remain coherent. It exited successfully;
the host logged a nonfatal winit destroyed-window warning during shutdown. The existing debug
linker's large unwind-section warning also remains nonfatal.

`cargo xtask verify` passes 538 tests, 27 dialogue fixtures, 11 STT fixtures and spoken-input replay.
The contract and STATE record the completed pass. The next step remains cross-platform and
lower-end GPU measurements; no release artifacts or model bundles were rebuilt.
