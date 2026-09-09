# State

Last updated: **2026-09-09**.

## Now

- Gold-reference refinement is implemented and repeatedly reviewed. Permanent originals,
  before/after screenshots and capture commands remain in [style references](style-reference/README.md).
  The [refinement record](style-reference/refinement.md) explains accepted findings and limits.
- Water now has smaller broken highlights, varied caustic coverage and stronger soft shafts.
  Contact darkening, warm practical fill, curved fronds, eroded rock courses, a rounded cave
  and restrained glass edges give the tank more depth.
- Settings has thinner stepped rims and much less visible transmission. Selection brackets
  follow actual transformed mesh bounds; toy cards choose clear space once and stay still.
  The title has a smaller creature clear of the cave and toys grounded against actual sand.
- `cargo xtask verify` passed 548 tests, 27 dialogue fixtures, 11 STT fixtures and spoken-input
  replay. Two final native motion captures and independent visual/code reviews found no known
  material reasonable correction remaining. Native macOS checks verified title/settings,
  Large text, return to gameplay, direct toy picking and Play. All seven matched authoritative
  checkpoints and the recorded event/audio semantics remain unchanged.
- Final recorded median/p95 remains 16.74/33.58 ms versus 16.66/33.44 ms before;
  geometry arenas are 29.2% smaller. An uncaptured 1080p debug comparison also remains
  near the baseline. Exact pose caching avoids repeated selected-toy mesh scans.
- Physical glass refraction and full volumetric transport remain approximations. No release
  build, installer or model bundle was rebuilt. Windows/Linux and lower-end GPUs remain
  unmeasured; timing evidence and caveats live in the refinement record.

## Next

Measure the updated renderer and input path on Windows, Linux and an available lower-end GPU
using ordinary debug builds. Use `cargo xtask feel --suite gold-reference`, record host
specifications, uncaptured frame cadence and actual interactions. Current native evidence
covers M5 Max/Metal; broader measurements should precede a more expensive glass or lighting
pipeline. See [renderer contract](raytraced-aquarium.md) and
[MVP performance targets](mvp-spec.md#performance-budget).

## Candidates Not Chosen

- **Physical glass and richer light transport:** the main remaining gold differences require
  broader rendering work; measure other hardware before increasing ray/denoising cost.
- **Human full-speed and listening calibration:** remains useful for aesthetic and emotional
  judgment beyond native playback, sampled recordings and recorded audio semantics.
- **Full 3D navigation and collision:** changes gameplay without unblocking the care experience.

## Learned Recently

- Visual decisions, comparisons, preserved semantics and native evidence:
  [gold refinement](style-reference/refinement.md), [implementation](style-reference/implementation.md).
- Geometry, procedural effects and unified UI: [art bible](art-bible.md),
  [renderer contract](raytraced-aquarium.md).
- A worker fixture must publish its complete PID marker atomically; creation alone is not
  readiness. The cleanup-test fixture now writes then renames the marker.
- Motion continuity: [motion review](feel-review-motion-20260909.md).
- Native recording and sound: [feel-review-loop.md](feel-review-loop.md), [audio direction](audio-direction.md).
- Product authority: [game-design philosophy](game-design-philosophy.md),
  [relationship causality](relationship-causality-rework.md).
