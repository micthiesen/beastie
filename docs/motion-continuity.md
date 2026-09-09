# Motion continuity

Date: 2026-09-09. Status: implemented; repeated native review and headless verification complete.

Mop should swim, settle and change expression without one-frame relocation, repeated tick-boundary
recoil, or body waves that accelerate as the session gets older. The user authorized automatic
fixes and repeated review. Preserve simulation ownership, immediate care receipt, truthful contact,
offline operation and the existing creature identity. No save migration or new game system.

## Accepted changes

- Clear stale authoritative velocity when motion is stationary, including sleep and food acting.
  Presentation must also handle existing saves containing stale velocity.
- Bound forward prediction by the actual travel destination, separately from gaze. Correct an
  interrupted prediction at creature speed rather than the former 12 world units per second.
  Preserve normal travel and flee pacing; do not delay all presentation by a simulation tick.
- Integrate swim-wave phase with frame delta and current frequency. Session age and speed changes
  must not relocate the phase. Blend supporting nod, tilt and fin gestures through recipe changes
  while current eye/mouth meaning and speech cancellation remain immediate.
- Record bounded, privacy-safe rendered motion diagnostics in synchronized feel state. Thresholds
  flag review candidates, not automatic gameplay errors. Distinguish startup/clock discontinuities,
  presentation-clock elapsed time, root movement and articulated part motion. No normal-play log spam.
- Settle brow/smile changes briefly; keep gaze retargeting, authored blinks and current mouth
  aperture responsive. Integrated fin/tail cycles must also cross hour boundaries continuously.
- Consume the complete host frame delta, up to 250 ms, using bounded body substeps rather than
  accumulating presentation lag until a distance reset. Normalize old saved stationary velocity
  before input or phase transitions, preserving simulation chunk invariance.
- Investigate any remaining measured boundary or body-chain jumps before acceptance. Continuous
  tail arcs can legitimately cross a translation threshold; adjudicate frames before changing rates.

## Acceptance

Regression tests cover stationary saved motion, speed changes late in a session, interrupted
travel, arrival and bounded pose transitions. Existing core event semantics remain intact.
Same-seed quiet and interaction captures establish the before/after result. Complete two valid
native review passes after the last material change, review motion traces and dense frames, and
record any unavailable full-speed perception or physical-listening evidence honestly. Run
`cargo xtask verify` and `cargo xtask dev --fake-ai`, update the review and STATE, commit and push.


The [review](feel-review-motion-20260909.md) records accepted comparisons, exact binaries,
adjudicated diagnostic alerts and evidence limits. All accepted scope is implemented. Ordinary
prediction correction is capped at max(authoritative world speed, 0.9) + 0.15 units/s, with a
1.9 units/s overall ceiling. This permits faster flee motion without granting ordinary corrections
the old 12 units/s dash. No model, audio, save-format or authoritative contact policy was changed.
