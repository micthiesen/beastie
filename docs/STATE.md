# State

Last updated: **2026-09-09**.

## Now

- The three user-supplied gold screenshots are permanent visual authority for future UI/art
  work. Originals, baseline captures, final native comparisons and reproduction commands live
  in [style references](style-reference/README.md) and the [implementation note](style-reference/implementation.md).
- Gameplay has a compact identity/toy/chat rail, in-world selection brackets and nearby toy
  cards. Settings uses a right-side translucent plate and vertical categories. A living title
  screen adds a voxel logo and real Continue, Settings and Quit actions.
- The existing compute renderer now shades animated water highlights, warped caustics, depth
  tint, light shafts and fine sand grain. Warm lamps, metallic framing, taller plants and
  clustered rocks establish the reference palette. See [art bible](art-bible.md) and
  [renderer contract](raytraced-aquarium.md) for current ownership and deliberate approximations.
- `cargo xtask verify` passed 542 tests, 27 dialogue fixtures, 11 STT fixtures and spoken
  input replay. Native macOS input and repeated static/animated captures were reviewed; independent code
  review found no outstanding defects. The final recorded debug run on M5 Max/Metal measured
  16.62 ms median and 27.28 ms p95 wall frames including capture overhead. Other hosts remain
  unmeasured; this is not an uncaptured performance claim.
- Earlier movement/animation continuity and simulation authority remain intact. See
  [motion review](feel-review-motion-20260909.md). No native release, installer or model bundle
  was rebuilt for this visual iteration.

## Next

Measure the updated renderer and input path on Windows, Linux and an available lower-end GPU
using ordinary debug builds. Use the gold-reference scenarios and record host specifications,
uncaptured frame cadence and actual interactions. The new atmosphere is bounded, but current
native evidence covers only M5 Max/Metal. This closes a concrete portability/performance gap
before more lighting work. See [renderer contract](raytraced-aquarium.md) and
[MVP performance targets](mvp-spec.md#performance-budget).

## Candidates Not Chosen

- **Further gold fidelity:** richer glass reflections, softer light transport and more detailed
  plant/rock silhouettes remain visible differences. Hardware measurements should guide the
  next rendering investment; physical volumetrics and general bloom are not defaults.
- **Human full-speed and listening calibration:** remains useful for perceived motion and audio,
  beyond sampled recordings and automated assertions.
- **Full 3D navigation and collision:** changes gameplay without unblocking the current care
  experience, so it remains deferred.

## Learned Recently

- Gold art direction, comparisons, preserved semantics and capture commands:
  [style implementation](style-reference/implementation.md).
- Procedural effects and unified geometric UI: [renderer contract](raytraced-aquarium.md).
- Motion continuity and saved-state invariance: [motion review](feel-review-motion-20260909.md).
- Native recording and sound evidence: [feel-review-loop.md](feel-review-loop.md),
  [audio-direction.md](audio-direction.md).
- Product authority and relationship semantics: [game-design-philosophy.md](game-design-philosophy.md),
  [relationship-causality-rework.md](relationship-causality-rework.md).
