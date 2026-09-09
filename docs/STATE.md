# State

Last updated: **2026-09-09**.

## Now

- Mop's movement and animation continuity pass is implemented: destination-bounded prediction,
  creature-paced correction, stationary velocity normalization, continuous swim/fin phases and
  brief pose/face settling. See [motion review](feel-review-motion-20260909.md) and
  [contract](motion-continuity.md).
- Feel recordings now include actual rendered head/part motion measurements and diagnostic alerts.
  Reviewed tail sweeps and immediate eye responses remain intentional; thresholds do not replace
  frame review. See [feel-review-loop.md](feel-review-loop.md).
- Same-seed native comparisons preserve authoritative outcomes and sound cues. The final quiet
  recording peaks at 1.058 world units/s head speed and shows no head-speed or rotation alerts.
  Human full-speed perception, physical listening and other-host behavior remain unclaimed.
- The gate passes 538 tests, 27 dialogue fixtures, 11 STT fixtures and spoken-input replay.
  Saved stationary recovery, bulk/split ticks, slow frames and late-session animation have focused
  regressions. Ordinary native debug validation is recorded in the motion review.
- Earlier creature-presence, care, UI and renderer detail work remains complete. No release binary,
  installer or model bundle was rebuilt. Prior rendering measurements remain in the
  [detail review](feel-review-details-20260908.md).

## Next

Measure the current renderer and input path on Windows, Linux and an available lower-end GPU using
ordinary debug builds. This closes a concrete driver/performance uncertainty before adding more
rendering work; record host specifications, frame cadence and interaction results, without treating
video's fixed recording rate as measured runtime performance. See
[renderer contract](raytraced-aquarium.md) and [MVP performance targets](mvp-spec.md#performance-budget).
This needs the actual hosts/devices; allow a few hours per platform, with driver behavior and GPU
cost as the largest unknowns. It does not require release packaging.

## Candidates Not Chosen

- **Human full-speed and listening calibration:** a short observation session can assess animation
  taste, attachment, unaided discovery and mix quality beyond sampled evidence. It needs human
  perception; reviewed diagnostic alerts alone do not justify slowing expressive tail turns or eyes.
- **Further lighting or reconstruction features:** several days plus native comparisons. Their
  value depends on hardware measurements and a demonstrated visual need, so they wait.
- **Full 3D navigation and collision:** several days or more, with headless geometry tests and native
  readability review. It changes gameplay without unblocking the current care experience.

## Learned Recently

- Motion continuity, measurements and saved-state invariance: [motion review](feel-review-motion-20260909.md).
- Detail findings, rendering tradeoffs and capture integrity: [detail review](feel-review-details-20260908.md).
- Native capture and role-aware sound evidence: [feel-review-loop.md](feel-review-loop.md),
  [audio-direction.md](audio-direction.md).
- Broader creature/care evidence: [presence review](feel-review-creature-presence-20260908.md),
  [creature presence and care](creature-presence-and-care.md).
- Renderer, mesh and art ownership: [architecture.md](architecture.md), [art-bible.md](art-bible.md),
  [raytraced-aquarium.md](raytraced-aquarium.md).
- Product authority and relationship semantics: [game-design-philosophy.md](game-design-philosophy.md),
  [relationship-causality-rework.md](relationship-causality-rework.md).
- Local inference and recognition choices: [local-mouth.md](local-mouth.md),
  [stt-runtime.md](stt-runtime.md).
