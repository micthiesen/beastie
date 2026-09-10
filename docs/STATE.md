# State

Last updated: **2026-09-09**.

## Now

- Beastie is the game and title brand; Mop is the default creature name. The deterministic,
  offline creature simulation and declarative scene remain authoritative.
- The renderer combines hardware raster visibility, shared compute lighting, exact static
  shadow/bounce caches, compact lossless surface data and retained UI label geometry.
  Twelve 1024² dynamic shadow maps accelerate validated viewports through 1920×1080;
  larger views and capability limits retain original rays. No RTX requirement or backend fork.
- On M5 Max/Metal at 1080p, corrected bounded GPU batches measure 2.075 ms/frame, with a
  1.879 ms warmed completion median. Matched native UI wall p99 is 8.684 ms versus 17.047 ms
  for the first-pass baseline. A 92.775-second sustained run has p99 8.687 ms, max 9.863 ms
  and no frame above 33 ms. GPU throughput and native presentation cadence are distinct.
- Retained geometry is 75.88 MB versus 138.60 MB, but intermediate storage is 201.71 MB and
  measured process footprint increases. The [efficiency review](renderer-efficiency.md)
  records memory/startup tradeoffs, methodology, rejected experiments and reproducible evidence.
- Final verification passes 596 tests plus dialogue/STT fixtures and replay. Explicit GPU
  traversal/cache/map/fallback checks pass. Fresh evidence covers 36 still states and 504 motion
  frames with exact semantic traces. Small raster/shadow-edge differences are visually accepted;
  images are not bit-identical. Native coverage remains M5 Max/Metal.
- `cargo xtask dev` now uses optimized `dev-perf` with assertions and overflow checks.
  Use `--profile dev` for unoptimized debugging. No release bundle or model bundle was rebuilt.

## Next

Measure the finished renderer on Windows, Linux and an available lower-end GPU using `dev-perf`.
Run the capture-free performance scenarios and gold-reference motion suite, recording the selected
shadow mode, actual viewport/backend, wall tails, GPU statistics and memory. Compare mapped and
original-ray controls where supported. This is the evidence needed for further device adapters
or higher-resolution shadow maps; the current implementation already preserves ray fallback there.
Commands and source reconstruction are in the
[evidence guide](performance/20260909-efficiency/README.md).

## Candidates Not Chosen

- **Further generic traversal rewrites:** grids, compressed BVH4/BVH8, analytic ellipsoid hints,
  alternative workgroups and occlusion variants did not beat their measured controls.
- **More passes or cheaper water noise:** split shadows saved too little for their memory cost;
  integer noise showed no repeatable gain. Reproduction patches preserve these results.
- **Larger or stationary-receiver-only maps:** 2048 maps cost 192 MiB and retain sparse errors;
  the narrower receiver policy was slower. Fixed 1024 maps apply only in the reviewed viewport range.
- **Physical glass and richer light transport:** remain separate visual work; no full refraction,
  volumetric transport or subsurface-scattering claim is made.

## Learned Recently

- Current architecture, measurements, review, limitations and rejected experiments:
  [renderer efficiency](renderer-efficiency.md), [renderer contract](raytraced-aquarium.md).
- Historical first-pass measurements: [renderer performance](renderer-performance.md).
- Visual direction: [gold refinement](style-reference/refinement.md),
  [implementation](style-reference/implementation.md), [art bible](art-bible.md).
- Native review and product boundaries: [feel-review loop](feel-review-loop.md),
  [game-design philosophy](game-design-philosophy.md), [audio direction](audio-direction.md).
