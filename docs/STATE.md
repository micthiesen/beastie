# State

Last updated: **2026-09-09**.

## Now

- Beastie remains an offline, deterministic creature game. Simulation owns facts;
  the model supplies expression, and the game works without inference or speech.
- The renderer combines raster visibility and shared compute lighting with exact
  static caches, persistent static shadow maps, a twelve-view dynamic shadow atlas,
  indexed shadow-only LOD and background geometry preparation. Visible meshes and
  foreground animation cadence retain their detail. Ordinary GPU support requires
  no RTX hardware or vendor-specific renderer.
- The latest paired M5 Max/Metal runs reduce 1080p GPU throughput time by about 32%
  beyond `41c7006`, to 1.50–1.51 ms/frame. Matched ordinary task CPU energy falls
  about 33%; hidden-only 10 Hz pacing cuts hidden task CPU energy about 85%.
  Matched native p99 remains about 9.2 ms; 22,324 final measured frames include
  none above 16 ms. Process footprint increases despite lower GPU allocation.
  These are separate measurements, not whole-device battery-life claims.
- Adaptive Depth16 maps cover reviewed viewports through 4K, using 7.5/30/120 MiB
  at 360p/1080p/4K. Original rays remain the larger-view/adapter fallback. A bounded
  preparation job uses original geometry while pending, avoiding synchronous
  spawn/title stalls. Retention, approximations, native frame tails and rejected
  experiments are recorded in the [energy review](renderer-energy.md).
- `dev-perf` retains application assertions and overflow checks while disabling
  development-only checks in the pinned objc2 dependency. This additional CPU-energy
  saving is separate from renderer work; release already omits those checks.
  Opt-in profiling now uses only bounded custom GPU counters after the redundant
  Bevy diagnostic pool failed during fullscreen capture.
- Verification covers 611 tests, dialogue/STT fixtures and replay, explicit GPU
  traversal/cache/map oracles, 36 still states and 504 motion frames with identical
  semantic traces. Combined LOD/map-density images and all six window scales plus
  fullscreen/restore are reviewed. Native coverage remains M5 Max/Metal.

## Next

Measure this implementation on Windows, Linux and an available lower-end GPU.
Use the capture-free world/UI/churn/sustained scenarios and gold-reference motion
suite. Record actual viewport/backend, selected map density, frame tails, memory
and available energy accounting. Cross-device evidence is needed before choosing
further adapter-specific policies. Reproduction commands and raw evidence are in
[the evidence guide](performance/20260909-energy/README.md).

## Candidates Not Chosen

- **Spatial lighting reuse:** workgroup sharing, coarse producer/resolve passes and
  multiple pixels per invocation all regressed against paired controls.
- **Single-map soft shadows:** faster than its control but visibly weakened contact
  and removed broad shadows; rejected.
- **More generic traversal or index reordering:** previous grids/compressed trees
  and the latest vertex-cache ordering showed no material repeatable benefit.
- **Lower visible cadence or primary resolution:** retained current presentation
  quality; hidden-window pacing provides savings without changing visible animation.

## Learned Recently

- Current measurements, decisions and limitations: [renderer energy](renderer-energy.md).
- Current rendering contract: [raytraced aquarium](raytraced-aquarium.md),
  [architecture](architecture.md).
- Historical measurements: [renderer efficiency](renderer-efficiency.md),
  [first performance pass](renderer-performance.md).
- Product and visual authority: [game-design philosophy](game-design-philosophy.md),
  [art bible](art-bible.md), [gold refinement](style-reference/refinement.md).
