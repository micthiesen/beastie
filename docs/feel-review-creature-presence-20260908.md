# Creature presence and care: implementation review

Date: 2026-09-08. Native integration review in progress.

This implements the seven accepted findings in
[the holistic review](feel-review-holistic-20260908.md) under the authorized
[creature presence and care contract](creature-presence-and-care.md). The user explicitly
authorized every change and continued implementation without another approval step.

## Final design

- Empty, unfocused composition has no filled inset. Editing retains the existing field and fixed
  hit targets. A short care invitation yields to hover, focus, speech and notices; only direct
  player care dismisses it. Autonomous play or consumption of old food cannot dismiss it.
- Microphone hover explains disabled recovery and hold/release operation. Disabled controls remain
  inert; reachable Settings focus carries the recovery explanation. Helpers use the reserved
  status strip, with technical and uncertain-hearing feedback taking priority.
- New aquariums stagger toy positions. The bell carries an integral cork float, cloth bends toward
  the substrate, and the ball remains buoyant. Old habitat coordinates are preserved. Toys now use
  cached exact presented mesh picking, including carried rotation and the bell's open loop.
- The shelter opening accommodates the full head and crown and sits behind the interaction plane.
  The rest destination and creature behavior remain authoritative and unchanged.
- Semantic mix roles distinguish ambience, UI, physical effects, creature calls and speech. Only
  creature calls and speech duck the bed, with bounded authored attack/release envelopes.
- Audio evidence retains actual resolved bytes, identities, owners, accepted lifetimes and gains.
  Reconstruction honors cancellation, suppression, mute and ducking. Missing/tampered sources,
  clipping and unrepresented starts reject evidence instead of producing a misleading soundtrack.
- Deferred words re-check the current direct-care handoff. Replacing the old owner cannot give
  older speech permission to erase a newer refusal, toy approach or food action. Unrelated private
  life still cannot hold words indefinitely; original expiry and immediate new care are preserved.

## Iteration and independent review

The first native preview exposed a remaining gap between the cloth and its shadow. The second
preview lowered the new default to `(8250, 10000)` and relaxed the mesh further. Independent review
also caught two interaction defects before final recording: proxy toy volumes did not cover the
new cuff/cork or follow rotation, and autonomous toy events could dismiss the invitation. Both were
corrected with regression coverage.

Audio review caught an evidence edge where playback could start and stop between snapshots.
Started source metadata and bytes are now retained, and reconstruction explicitly rejects that
insufficient timing evidence. This is a bounded frame-clock reconstruction, not sample-accurate
capture of a device buffer. No native result is silently promoted through that guard.

The first integrated interaction-chain capture was rejected because its final talk arrived at
`01:00.483`, inside the cooldown of the correctly deferred acceptance at `00:31.233`. The old run
accepted those words at `00:27.416` and therefore had enough idle time. The final quiet wait now
lasts 34 seconds instead of 30, preserving both refusal input times and allowing the final
active-TTS cancellation assertion to exercise a real source. The failed attempt remains in
`target/feel/presence-final-01/interaction-chain/attempts/attempt-01/`; it does not count as accepted
capture evidence. Runtime cooldown was not weakened to satisfy the fixture.

Independent visual, causal and audio reviewers examine the promoted evidence. The review uses
sampled native frames, dense sequences and synchronized traces. Neither the assistant nor those
reviewers can certify full-speed human perception, host listening, unaided discovery or physical
controller comfort. These limits remain explicit and do not imply an observed runtime defect.

## Evidence and dispositions

Before: `target/feel/holistic-20260908/`, seven seed-42 native experiences from the original review.
After: `target/feel/presence-final-01/`, same baseline scenarios, seed, viewport and fake backends.
The two promoted opening/quiet experiences remain there; repaired interaction and remaining
baseline experiences are under `target/feel/presence-completion-01/<experience>/<experience>/`.
Constructor toy positions are an intentional difference; fixture-backed existing saves retain
their original coordinates. The old reference soundtrack is not used as a faithful audio baseline.

Native UI previews: `target/captures/presence-01/` and `presence-02/`, each 28 Normal/Large captures
from `cargo xtask dev --fake-ai`. The second contains final visible geometry and helper placement;
subsequent exact toy picking and player-only invitation dismissal are verified separately.

First-five-minutes comparison: berry consumption stays at `00:54.166`. Ball contact moves from
`01:06.233` to `01:04.233`, consistent with its new canonical position. The shared-ball callback
remains memory-backed and the five-minute run retains ten autonomous starts. At `03:00`, the crown
has dark clearance below the arch and the complete face/body remain visible. The first reference
reconstruction retains 11 sources, has output available throughout and peaks at `0.421173`, with
zero clipping and no limiter.

Final baseline, second-pass, native-input and finding-by-finding results are recorded below when
their bundles have passed validation.

## Persistence and regression evidence

Core tests drive real contact to produce a moving ball and a carried sock, preserve deliberately
non-default saved coordinates, round-trip the complete state, and compare ten continuation ticks
and events. They require no duplicate payoff and eventual settling/release. Constructor tests
require catalogue and mutable toy positions to agree. Existing view tests cover carried projection.

Deferred-owner tests cover accepted play, refusal, food, cooldown-only deferral, repeated
replacement/expiry, typed/spoken parity, immediate new comfort and save/load. In an isolated copy
with the old owner-inequality shortcut restored, three new regressions fail while 46 other session
tests pass. The shared checkout was never reversed for this comparison.

Mesh tests cover shelter clearance, hollow plant/cave gaps, bell-loop gaps and carried-cloth
rotation. View/input tests cover state explanations, disabled access, focus reachability, Large
bounds, technical feedback priority/expiry and player-only invitation dismissal. Audio tests cover
semantic ducking, discarded commands, bounded envelopes, interrupted/muted sources, missing output,
overlap, retained source integrity and clipping visibility.
