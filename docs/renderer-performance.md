# Renderer performance review

2026-09-09. The final measured renderer reduces world GPU median time from **27.458 to
10.178 ms** on Apple M5 Max/Metal at 1920×1080, a **62.9% reduction**. The UI sequence improves
from **28.431 to 10.354 ms** on the GPU, while its wall-frame p99 falls from **502.946 to
23.437 ms**. The changes retain authored geometry, bevels, colors, geometric lettering,
lighting sample counts, render resolution and animation. Final acceptance is recorded separately
at the end of this document.

The renderer remains ordinary portable compute. It requires neither ray-tracing hardware nor
optional subgroup features. The [renderer contract](raytraced-aquarium.md) owns its architecture;
this record owns the performance evidence, experiments and remaining limits.

## Measurements and durable evidence

These are ordinary debug builds, the same deterministic command sequences, fake AI, a
1920×1080 window, requested uncapped presentation, and no image readback. Only one game ran at a time,
without concurrent builds or GPU tests. Each distribution excludes its first 30 samples
and uses nearest-rank percentiles. `AutoNoVsync` is a request; the backend can still pace presentation.
The original source is commit `9aaafbc28012d0b6298dd5c957a53c0ec06eaade`, built in a scratch copy
with the same corrected timestamp instrumentation. Sample counts differ with throughput and asynchronous
readback; the reports preserve those counts.

| Sequence and measurement | Original p50 / p95 / p99, ms | Final p50 / p95 / p99, ms |
| --- | --- | --- |
| World GPU compute | 27.458 / 28.796 / 29.018 | 10.178 / 10.530 / 10.705 |
| World wall-frame interval | 28.982 / 29.356 / 29.602 | 8.517 / 16.699 / 16.982 |
| UI GPU compute | 28.431 / 31.430 / 37.937 | 10.354 / 10.948 / 11.357 |
| UI wall-frame interval | 25.808 / 38.759 / 502.946 | 8.450 / 16.793 / 23.437 |

The GPU-duration ratios are 2.70× for world and 2.75× for UI. These are not a promise
of equivalent FPS on every device. GPU compute duration excludes presentation; wall intervals
include CPU work, GPU backpressure, window pacing and scheduling. Asynchronous pipelining means
an individual wall interval can be shorter than a GPU pass. Do not invert wall median into FPS.

The four JSON reports are retained without modification:

- [Original world](performance/20260909/baseline-timed-world.json)
- [Original UI](performance/20260909/baseline-timed-ui.json)
- [Final world](performance/20260909/final-world.json)
- [Final UI](performance/20260909/final-ui.json)

Bevy's encoder-based GPU diagnostics produced zero timestamps on this Metal path. The opt-in
report therefore measures at compute-pass boundaries using four reusable asynchronous readback
slots. A busy GPU can skip instrumentation instead of blocking the game. Unsupported or zero-only
measurements remain unavailable, never a zero-cost render. Pending readbacks at exit are omitted.
Ordinary play allocates no timing queries. CPU phase measurements are per invocation and may run
concurrently, so they must not be summed into frame time.

## What changed

- **Shared geometry and acceleration structures.** The creature's 32 articulated entities now
  use 12 mesh assets. Its 12 identical body segments share one mesh/BLAS; paired facial parts and
  the fins, crest and tail also reuse geometry. UI icons use canonical meshes with persistent
  instances, so moving an icon or revisiting a tray does not rebuild its voxel mesh or BLAS.
- **Better mesh BVHs.** Mesh-local BLAS construction uses 12-bin surface-area heuristic splits.
  Median fallback and a depth budget keep the maximum path at 31 edges, within the existing
  32-entry shader stacks. The small, changing TLAS retains cheaper median construction.
- **Less traversal work per ray.** Reciprocal directions are computed once per coordinate space.
  Explicit parallel-axis handling retains finite slab arithmetic at voxel boundaries. Auxiliary
  world and shadow roots skip irrelevant overlay and unlit instances before descending into their
  meshes; all roots share instance IDs and preserve the previous visibility rules.
- **Separate intersection and shading data.** A 48-byte record holds position and precomputed
  edges; normals and colors remain in a separate lossless 96-byte record with matching indices.
  This did not independently establish a Metal speedup: isolated world medians were 10.297 ms
  with the split versus 10.278 ms before it. It is retained as a smaller intersection working set,
  without claiming that it explains the aggregate improvement.
- **Remove redundant planar tessellation.** Eligible flat-normal environment faces retain their
  complete 12-vertex perimeter but replace 18 triangles with 10. Nonplanar bevels, material
  boundaries and shared edges remain intact. Creature and title paths with position-dependent
  attributes keep their original tessellation. Axis-normal normalization may differ within f32
  rounding; tests bound that difference rather than mistaking it for a changed surface.
- **Avoid repeated UI work.** Exact effect/selection inputs skip unchanged mesh mutations. Text
  rebuilds only when labels or actual clipping regions change; panel invalidation is independent.
  Glyph clipping uses inside/outside fast paths and bounded stack storage instead of allocating
  polygons per clipping plane. Color conversion happens once per label, and fully covered labels
  skip shaping. An independent copy of the old clipper checks vertices, colors and winding.
- **Avoid duplicate GPU storage.** CPU mesh assets no longer request unused raster uploads.
  Compute buffers retain modest growth slack instead of applying another power-of-two expansion
  over arenas that already reserve power-of-two mesh slots.

Interior voxel faces, cached mesh-local BVHs, near-first traversal, early shadow termination and
locally shared illumination already existed. They are not new features of this pass. The scene
is also more substantial than its visible voxel count suggests: final world telemetry records
501,922 triangles across resident mesh assets. That includes cached/hidden geometry and is not a
visible-triangle count.

## Geometry memory

| Quantity | Original world | Final world |
| --- | ---: | ---: |
| Triangle arena capacity | 956,752 slots | 767,312 slots |
| BLAS node arena capacity | 478,376 slots | 418,416 slots |
| Logical geometry arena extent | 153,080,320 bytes | 123,882,240 bytes |
| Retained compute geometry buffers | 285,212,672 bytes, derived | 138,596,096 bytes, measured |

The original retained buffer sum is **derived from the old allocation rule**, not a baseline
VRAM measurement: the 144-byte triangle arena rounds to 268,435,456 bytes and the 32-byte node
arena to 16,777,216 bytes. Final telemetry directly sums retained intersection, surface and BLAS
buffers. This comparison is a 51.4% reduction, from 272 MiB to 132.18 MiB. The final UI run
reports the same retained buffer sum.

These numbers are not total process or GPU memory. They exclude output textures, driver storage,
CPU meshes, instance/TLAS buffers and other application resources. Arena capacity includes
per-mesh spare slots and holes awaiting compaction. The final reports expose live mesh counts,
arena extents and actual retained compute buffers separately. More icon instances and auxiliary
TLAS nodes are intentional consequences of sharing geometry and filtering ray categories.

## Experiments not retained

Experiments used copied debug binaries so subsequent builds could not change a running candidate.
They were compared with the improved binary-BVH renderer, not the slower original. Individual rows
are controlled runs, not repeated statistical estimates; sub-percent differences here do not
establish a useful win.

| Candidate | World GPU p50 / p95, ms | Decision |
| --- | --- | --- |
| Binary BVH, 8×8 baseline | 10.297 / 10.534 | Retain |
| Unquantized BVH4 BLAS | 11.165 / 11.406 | Reject, 8.4% slower |
| 8×4 workgroup | 10.289 / 10.528 | No material improvement |
| 16×4 workgroup | 10.746 / 11.001 | Reject |
| 4×4 workgroup | 10.449 / 10.992 | Reject |
| 16×8 workgroup | 10.962 / 11.601 | Reject |
| Constant shadow-direction table | 10.288 / 10.537 | No material improvement |

The BVH4 prototype collapsed two binary levels, retained exact bounds and leaf ranges, tightly
packed 2–4 children in the existing 32-byte format, and left TLAS binary. Structural tests and
the Metal traversal oracle passed. Its local stack required 48 slots, with a proven maximum of
46 deferred siblings, versus 32 slots for binary traversal. Additional box tests, sorting and
larger live state are plausible costs, but were not isolated by hardware-counter profiling.

The shadow table evaluated the original twelve interleaved directions as f32 constants. Tests
verified finite unit vectors and sampling order, and shader translations passed. Timing was
effectively tied. Host constant evaluation can differ slightly from native GPU math at occlusion
boundaries, so an unmeasured benefit did not justify retaining it. Naga emits the original loop,
but Metal may optimize it later; no claim is made about the final machine code.

## Reference techniques and fit

- [TinyBVH](https://github.com/jbikker/tinybvh) and its
  [compute traversal](https://github.com/jbikker/tinybvh/blob/main/kernels/traverse_bvh2.cl)
  informed binned SAH, instancing, reciprocal directions and separate occlusion work. The reference
  supports software traversal; its published throughput is not a Beastie performance target.
- [VoxelRT](https://github.com/dubiousconst282/VoxelRT) compares acceleration structures and ray
  classes; its [Tree64 explanation](https://dubiousconst282.github.io/2024/10/03/voxel-ray-tracing/)
  illustrates wide occupancy masks and sparse traversal. Pure binary voxel cells cannot directly
  reproduce Beastie's beveled triangle surfaces and geometric glyphs. A hybrid could, but would
  add another representation and traversal path. World paging is irrelevant to this aquarium.
- [Compressed wide BVHs](https://research.nvidia.com/publication/2017-07_efficient-incoherent-ray-traversal-gpus-through-compressed-wide-bvhs)
  and the [reference implementation](https://github.com/AlanIWBFT/CWBVH) offer reduced memory traffic
  through quantization and wider nodes. Conservative bounds and ordering need their own validation.
  The rejected BVH4 experiment does not prove compressed BVH8 would lose, but it removes any basis
  for assuming that widening alone helps this workload.
- [Wavefront path tracing](https://research.nvidia.com/publication/2013-07_megakernels-considered-harmful-wavefront-path-tracing-gpus)
  separates work to reduce divergence and register pressure. Beastie's short, fixed lighting path
  would also pay queue, dispatch and memory costs. No wavefront implementation was benchmarked in
  this pass; a rewrite needs a measured scheduling bottleneck first.
- [Binary greedy meshing](https://github.com/cgerikj/binary-greedy-meshing) demonstrates occupancy
  masks and compatible face merging. Global merging can alter bevels, attribute interpolation or
  boundaries. This pass uses a smaller exact planar reduction. Raster vertex pulling does not
  directly accelerate compute triangle intersection.

These references guided independent changes and experiments. No claim is made that every voxel
engine technique belongs here, or that hardware-specific performance numbers transfer to Metal.

## Reproducing the measurements

Run from the repository root with a normal debug build. Set the game's saved display preference
to windowed scale 3 and use the same accessibility settings across candidates. Scripted performance
runs still load saved preferences. Confirm `viewport_pixels` is `[1920,1080]`; do not compare
different resolutions. The UI fixture explicitly exercises Normal and Large text. Use the normal
development setup if worker configuration is needed; recorded commands set no worker override.

```sh
cargo build -p beastie-game
mkdir -p target/performance
target/debug/beastie-game --fake-ai --script fixtures/scenarios/renderer-perf-world.jsonl --render-report target/performance/world.json --render-uncapped
target/debug/beastie-game --fake-ai --script fixtures/scenarios/renderer-perf-ui.jsonl --render-report target/performance/ui.json --render-uncapped
```

The [world fixture](../fixtures/scenarios/renderer-perf-world.jsonl) exercises creature motion and
toy interaction. The [UI fixture](../fixtures/scenarios/renderer-perf-ui.jsonl) traverses trays,
settings, bindings, dialogs and text scales. They replace image captures with waits. Keep the
window unobscured and avoid simultaneous builds, another game, image capture or GPU tests.
Use a different report filename for each comparison and record adapter, backend and build profile.

Run correctness separately from timing:

```sh
cargo xtask verify
cargo test -p beastie-game production_gpu_traversal_matches_brute_force -- --ignored --nocapture
cargo xtask dev --fake-ai
```

The mandatory gate remains headless. The explicit GPU oracle requires a native GPU and exercises
the production shader against a brute-force reference, including transformed overlapping instances,
parallel slabs, category roots and closest/any-hit rays. Shader translation alone cannot prove
driver behavior; native stills, motion and interaction remain separate checks.

## Practical limits

Only Apple M5 Max/Metal is measured. Windows, Linux, lower-end GPUs, thermal behavior, power use
and release-build performance remain unmeasured. No installer, release binary or complete model
bundle was rebuilt. The unchanged shading still spends multiple shadow rays and a secondary ray
per lit surface; this pass does not establish the best possible renderer for every future scene.

UI stalls are much smaller but not eliminated. Final UI CPU telemetry records scene extraction
p99 17.418 ms and maximum 41.047 ms, and UI synchronization p99 9.017 ms and maximum 22.515 ms.
Those are phase measurements, not additive frame durations. Retained CPU meshes and growing
geometry caches still deserve observation as content expands. The evidence favors this measured
implementation over adding another hierarchy, ray queue system or device-specific route.

## Final acceptance

`cargo xtask verify` passed: 577 tests, 27 dialogue fixtures, 11 STT fixtures, spoken-input
replay, format/lint, portable shader translation and the normal build. The optional Metal test
separately passed 2,145 rays (581 hits), comparing closest/any-hit traversal, transformed overlapping
instances, category roots, slab boundaries, barycentrics and interpolated normals/colors against
an independent double-precision reference. BVH construction is separately tested against brute
force, including depth bounds. Glyph clipping has 15,090 exact old/new comparisons. Independent
reviews of traversal/resources and UI/glyph changes found no remaining actionable defects.
The existing macOS linker warning about oversized compact-unwind offsets remains.

`cargo xtask dev --fake-ai --script fixtures/scenarios/gold-reference.jsonl --capture-dir ...`
completed all eight native gold captures. The separate `voxel-craft-ui.jsonl` run completed all
28 Normal/Large states. Compared with the original at 1920×1080, **99.9798% of RGB pixels are
exactly equal** across these 36 pairs. Mean absolute channel difference per image is below
0.004 on the 0–255 scale. The bottom UI rail is pixel-exact in all 28 UI states. Inspected keyboard,
rename, food/toy and speech regions are also pixel-exact. The inspected settings interiors differ
in 152 dark pixels, at most 2/255 per channel. Native-resolution crops and before/after images
show no visible layout, silhouette, typography or lighting regression. Sparse surface intersection
and shading differences remain; this is not a claim of bit-identical rendering.
[Full-image metrics](performance/20260909/still-comparison.json),
[UI-region metrics](performance/20260909/ui-region-comparison.json) and a
[gameplay comparison](performance/20260909/gameplay-comparison.png) preserve the evidence.
The comparison shows original left, final right, and an 8× amplified difference below.

Both `cargo xtask feel --suite gold-reference --game <candidate> --output <new-directory>` runs
completed the 8.4-second, 504-frame native recording at the suite's fixed 1280×720 resolution.
All 504 state records, 504 audio records, 480 event records, 14 input records and seven markers
match byte-for-byte. This includes presentation-motion measurements, selection, reduced motion,
title transition and return. Seven raw checkpoint images differ by less than 0.004/255 per channel
on average. Decoded H.264 video SSIM across all 504 frames is 0.99632 on average, with a minimum
of 0.99209; independent compression contributes to this metric. Filmstrips and the lowest-SSIM
frame pair were inspected without finding a material visual change.
[Motion comparison](performance/20260909/motion-comparison.json) records these checks.

Large native captures, videos and traces remain under `target/performance/20260909/` in
`baseline-gold`, `final-gold`, `baseline-ui`, `final-ui`, `baseline-motion` and `final-motion`.
They are local evidence, not checked-in runtime assets. The small reports above are durable.
Recorded/scripted input and sampled video review do not establish physical input latency, human
full-speed perception or other-platform driver behavior. The next measurement is the same workload
on Windows, Linux and a lower-end GPU; no material regression found here remains unfixed.
