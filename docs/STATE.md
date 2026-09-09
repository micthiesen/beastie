# State

Last updated: **2026-09-09**.

## Now

- Beastie is the game and title brand; Mop is the default creature name. Gold-reference
  refinement, water/lighting detail, geometric UI and native interaction evidence remain in
  [style references](style-reference/README.md) and the [detail review](style-reference/detail-review.md).
- The renderer performance pass cuts measured 1080p world GPU median from 27.46 to 10.18 ms
  on M5 Max/Metal. UI wall-frame p99 falls from 502.95 to 23.44 ms. These are ordinary debug,
  controlled single-run comparisons, not cross-device FPS guarantees.
- Shared meshes, SAH acceleration, category-specific traversal, planar tessellation reduction,
  compact intersection records and exact UI/effect caches preserve resolution and lighting samples.
  Ordinary Metal/Vulkan/D3D12 compute remains the shared path; no RTX requirement or backend fork.
- Retained geometry GPU buffers are about 139 MB versus 285 MB derived for the original allocation
  policy. This excludes other GPU resources. Repeatable scenarios, raw reports, research sources,
  rejected experiments and validation are in the [performance review](renderer-performance.md).
- Verification passed 577 tests plus dialogue/STT fixtures and replay. Native comparison covers
  36 still states and a 504-frame motion sequence; 99.98% of still pixels and all motion state/event
  traces match. The separate Metal traversal oracle passes 2,145 rays.
- Physical glass refraction and full volumetric transport remain approximations. Windows/Linux
  and lower-end GPUs remain unmeasured. No release binary, installer or model bundle was rebuilt.

## Next

Measure the optimized renderer and input path on Windows, Linux and an available lower-end GPU
using ordinary debug builds. Run the capture-free performance scenarios and
`cargo xtask feel --suite gold-reference`, recording host, viewport, backend and actual interactions.
Current native evidence covers M5 Max/Metal; broader measurements should guide any hardware adapter
or more expensive lighting work. See [performance review](renderer-performance.md),
[renderer contract](raytraced-aquarium.md) and [MVP performance targets](mvp-spec.md#performance-budget).

## Candidates Not Chosen

- **Wider BVHs or device-specific dispatch:** the measured BVH4 and workgroup prototypes did not
  improve this device; keep the shared implementation until another adapter proves a benefit.
- **Physical glass and richer light transport:** measure other hardware before increasing
  transport, history or denoising cost.
- **Human full-speed and listening calibration:** useful for aesthetic and emotional judgment
  beyond recorded frames, motion and audio semantics.
- **Full 3D navigation and collision:** changes gameplay without unblocking the care experience.

## Learned Recently

- Performance decisions, native evidence, reproducible commands and limits:
  [renderer performance](renderer-performance.md).
- Visual decisions and preserved semantics: [gold refinement](style-reference/refinement.md),
  [implementation](style-reference/implementation.md), [art bible](art-bible.md).
- Motion continuity and native recording: [motion review](feel-review-motion-20260909.md),
  [feel-review loop](feel-review-loop.md), [audio direction](audio-direction.md).
- Product authority: [game-design philosophy](game-design-philosophy.md),
  [relationship causality](relationship-causality-rework.md).
