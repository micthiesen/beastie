# Renderer architecture efficiency

This records the pass shipped at `41c7006`. The subsequent
[energy and tail-latency review](renderer-energy.md) supersedes its current implementation and
viewport policy; the measurements below remain historical evidence for this pass.

Completed review, 2026-09-09. Baseline: `a19edc90450ed0b6beae6541270730693607ee8c`.
The [first performance pass](renderer-performance.md) reached approximately 10 ms GPU at
1080p on Apple M5 Max/Metal. This pass tests architectural changes that preserve perceived
quality. The performance targets are 4 ms median GPU, with a 2 ms stretch target, and no
warmed UI transition above 33 ms. Reaching a target does not end the investigation.

The current implementation combines raster primary visibility with shared compute lighting,
exact static shadow and bounce caches, compact lossless data, and retained UI label geometry.
Initial matched native runs show substantially lower GPU pass times and wall-frame tails.
The integrated renderer passed independent review, the 596-test headless gate, fresh still/motion
review and repeated native UI/world tests. A 92.775-second measured sustained run had no frame
above 33 ms. The ranked implementation experiments are complete, with raw measurements and
reproducible candidate patches in the [evidence directory](performance/20260909-efficiency/README.md).

## Contract

Preserve offline deterministic gameplay and macOS, Windows and Linux support without RTX or
hardware ray queries. Keep shared scene/material logic with small capability adapters.
Preserve silhouettes, geometric typography, lighting character, shadows, reflections, animation
and immediate interaction in stills and motion. Sample counts, intermediate resolution, caching,
geometry formats, GPU scheduling and raster/compute architecture are implementation choices.
Noticeable blur, shimmer, ghosting, popping or stale lighting is unacceptable.

Report CPU work, GPU measurements, frame tails, startup, memory and sustained behavior using
repeated controlled measurements. State which platforms were actually exercised. Completion
requires no known plausible material improvement left untested or inadequately explained within
scope, followed by independent review, native validation, the project gate, commit and push.

## Measurement rules and timestamp correction

All device measurements below are from Apple M5 Max/Metal at 1920×1080. Other backends have
headless shader translation coverage, not equivalent native performance evidence.

The offscreen harness constructs the actual seed-42 scene through `RendererPlugin` and
`RayScenePlugin`. It freezes simulation and excludes CPU scene updates, uploads, presentation
and UI transition cadence. A run dispatches 30 warmup frames and 180 measurement frames.
These metrics answer different questions:

- **Isolated GPU latency:** submit one frame, wait for its completion, then resolve timestamps.
  This prevents future frames from overlapping the measured interval.
- **Batched GPU throughput:** divide the complete GPU envelope by all 210 dispatched frames.
  This includes warmup, pipeline fill and drain. It is not a warmed frame percentile.
- **Warmed completion cadence:** measure adjacent completion timestamps after warmup. The
  current report requires both endpoints after warmup, giving 179 eligible intervals.
- **Native wall-frame intervals:** include CPU work, GPU backpressure and presentation pacing.
  Named GPU pass histograms measure pass latency separately; their percentiles must not be summed.

Early Metal counters resolved within the measured submission returned stale data: isolated
reads lagged by one frame and the last batched timestamp pair was zero. Separately, a hybrid
raster-start to compute-end interval overlapped other frames in a batch. Its reported median
was 58.728 ms even though native wall-frame intervals were approximately 6.07 ms. Those
intervals are rejected as frame-cost measurements.

The corrected harness waits for the measured submission before resolving queries in another
submission. It records raw timestamp pairs, invalid indices, overlaps and submission wall
bounds. Corrected isolated baseline and hybrid runs yielded 210/210 valid timestamps, including
180/180 measured samples, with no impossible GPU durations or overlapping frames. Isolated
samples exceeding their own submission-completion wall bound are excluded from the latency
summary while retaining their raw values and rejection count. Batched reports distinguish
throughput from overlapping pass latency and report rejected completion intervals explicitly.

Native instrumentation uses twelve bounded asynchronous slots, at most four per named pass.
Completion callbacks defer query resolution to a later frame; separate query sets measure
raster visibility, dynamic shadow maps and compute. The third pass required raising the old
eight-slot total because slots retain their pass names; otherwise fallback rendering could fill
the pool before shadow maps became ready. Instrumentation never blocks rendering. Samples can be skipped
when slots are busy, and readbacks still in flight at exit are excluded. Missing or zero GPU
timestamps mean unavailable measurement, not a zero-cost frame.

Early native attempts that never reached renderer readiness while the computer-use privacy
screen was active produced no usable report and are excluded. This includes the attempt with
a separate pre-existing game process, which was closed through its ordinary save/close path.

## First ray-class ablations

These historical measurements preceded the timestamp correction. Each batched run retained
179 positive measured timestamps and dropped the final zero pair. They identified useful
investigations, but are not the final comparative throughput evidence. Compiler optimization,
control flow and occupancy change between probes, so timing differences are non-additive.
None of the incomplete diagnostic pictures is an accepted quality setting.

| Probe | GPU pass median / p95, ms |
| --- | ---: |
| Full | 10.811 / 10.917 |
| Primary visibility only | 4.156 / 4.167 |
| No shadows | 5.673 / 5.693 |
| No bounce/contact ray | 9.963 / 10.073 |
| No traced reflections | 10.743 / 10.855 |
| No optics | 10.445 / 10.552 |
| One primary sample | 6.840 / 6.935 |
| Full repeat | 10.798 / 10.909 |

The repeated full median differed by −0.119%. Primary visibility and shadow traversal became
architectural priorities. Corrected isolated baseline was 10.9996 / 11.1588 ms median/p95;
raster visibility with shared ray lighting was 6.9700 / 7.1270 ms.

## Integrated architecture and correctness evidence

### Visibility and exact data

Hardware rasterization writes triangle/instance visibility at the original quarter-pixel sample
positions. Compute reconstructs surface attributes and retains the lighting, optics, reflections
and coverage rules. Packed 32-bit IDs serve ordinary scenes; larger IDs use 64-bit attachments.
Viewport limits select the shared compute-primary shader. Depth is a transient, discard-only
attachment that permits memoryless tile storage where supported. There is one scene/material/
shading implementation, without an RTX requirement.

Before dynamic maps, the exact-cache comparison covered 36 matched native 1080p stills: 8 gold-reference states and
28 UI states at normal and large text sizes. Aggregate exact RGB pixel equality is 99.970104%;
mean absolute channel difference is 0.013334/255. Localized high-contrast differences include
rock-boundary slits filled by raster coverage and one coverage row at lamp/title edges.
Full-frame and enlarged inspection accepted these as no noticeable degradation, with authored
voxel geometry and layout preserved. This is perceptual acceptance, not bitwise equality.

All 504 motion state records, 504 audio records, 480 events, 14 inputs and 7 markers match.
Seven raw checkpoints have MAE 0.0021–0.0053/255. Decoded H.264 mean/minimum SSIM is
0.995025/0.987164; independent compression contributes to this difference. Review covered
the worst full-resolution pair, the overview and all seven 10 Hz marker filmstrips. No broad
stale-lighting trails, shifted typography or broken transitions were observed in these samples.
This is native rendered evidence, not a measurement of human full-speed input latency.
See the [inspection record](performance/20260909-efficiency/visual/inspection.md),
[still metrics](performance/20260909-efficiency/reports/still-comparison.json) and
[motion comparison](performance/20260909-efficiency/reports/motion-comparison.json).

Shader stacks use 16 TLAS and 24 BLAS entries, with matching CPU depth bounds of 15 and 23 and
SAH median-fallback capacity reservation. The explicit GPU traversal oracle passes 6,435 rays,
including maximum-depth overlapping trees and empty shadow roots. Singleton TLAS roots support
32,768 instances; voxels share meshes rather than becoming one instance each.

The frozen scene contains 500,904 resident triangles. In the global representation audit,
87.3% had flat normals, 93.7% constant vertex colors, and 111,676 distinct full surface records
covered the scene. Production uses a per-mesh exact surface dictionary with a typed index in
an existing geometry padding word. Surface data has separate retained allocation, revision and
compaction tracking. Positions, normals and colors are not quantized.

### Static caches and invalidation

Static receivers cache exact static shadow visibility while dynamic casters are traced every
frame. Camera, viewport, static geometry, transform, material, visibility and membership changes
invalidate the cache. Stable receiver identity and triangle keys cover instance reordering and
arena relocation. Caves are eligible; swaying plants, creatures and toys remain dynamic.
The standalone 15-frame mutation oracle reported zero differing half-float RGB pixels on every
frame, and the combined frozen shadow-cache comparison was also exact.

Static bounce-hit reuse retains dynamic traversal and lighting evaluation. Its 15 mutation
cases matched uncached half-float RGB. Equal-distance static intersections need special handling:
filtered trees can choose different tied surfaces. Cold classification now detects ambiguity
and keeps full-world traversal for those rays. An additional GPU test moves nonintersecting
dynamic bounds to reverse the full-world tie winner, checking cold and warm results with
distinct surface colors.

Storage binding limits change cache coverage without changing lighting samples. With a 128 MiB
binding limit, 1080p caches both coverage samples, 4K caches one, and 8K traces directly. The
retained 1080p bounce buffer occupies 18,662,400 bytes including allocation slack. Retained
geometry allocations are 75,882,496 bytes. Nominal framebuffer storage, including transient
depth and retained caches, is 151,372,800 bytes. Nominal storage is not a measured driver allocation.

### CPU work and short GPU batches

UI labels retain independent meshes across dialogs, so an unchanged label does not rebuild
when another label changes. Recently inactive semantic slots are bounded; edited text still
rebuilds its own geometry and BLAS. `dev-perf` optimizes interactive CPU work while retaining
debug assertions and overflow checks. It is a development profile, not a release bundle.

Earlier isolated prototypes measured 7.2266 ms median for static shadow caching with cave
eligibility and 3.9760 ms for hybrid visibility plus caching. These are preliminary isolated
results with elevated tails, not a sustained thermal characterization.

Short batched controls measured 3.2016 / 3.2395 ms per dispatched frame. Smaller stacks measured
3.0128 / 2.9518 ms. With small stacks, the surface dictionary and packed IDs individually
measured 2.9637 and 2.9645 ms. Their compared images matched the combined baseline exactly.
Precomputing the original twelve light directions preserved every compared pixel and reduced
repeated batch means from 3.0660 / 3.0491 to 2.9356 / 2.9522 ms. The constants were accepted.

Static bounce reuse then measured 2.5976 / 2.5458 ms against a matching 2.9702 ms control.
These are GPU envelope/210-frame means including fill and drain, not overlapping pass medians.
Later rejection experiments below use their own paired controls and should not be compared as
if all trials shared one unchanged baseline.

The final exact-cache ablation uses corrected GPU envelope/210-frame throughput. Full controls
were 2.5819 and 2.6255 ms/frame; primary-only 0.2496, no shadows 1.0288, no bounce 2.2242,
no reflections 2.5323 and no optics 2.3074. All timestamp pairs were valid. These results keep
dynamic shadow work first in the remaining candidate ranking. Probe differences remain
non-additive and are not accepted visual settings.

## Matched optimized native evidence

Both baseline and current binaries use the optimized development profile, the same device,
1080p viewport and corresponding scripted scenarios. Reports are `baseline-perf-native-ui.json`,
`bounce-perf-native-ui.json`, `baseline-perf-native-world.json` and
`bounce-perf-native-world.json` in `/tmp/beastie-perf-v2-20260909/`.
These are initial matched runs, not repeat or sustained acceptance evidence.

| Native scenario | Build | Measured wall duration / frames | Wall p50 / p95 / p99 / max, ms | Frames >33 ms | Renderer ready, ms |
| --- | --- | ---: | ---: | ---: | ---: |
| UI | Baseline | 21.954 s / 1,855 | 8.634 / 16.874 / 17.100 / 20.983 | 0 | 617.320 |
| UI | Current | 22.025 s / 2,643 | 8.323 / 8.657 / 8.784 / 9.554 | 0 | 1,002.181 |
| World | Baseline | 18.012 s / 1,497 | 8.656 / 16.854 / 16.996 / 20.726 | 0 | 488.295 |
| World | Current | 18.091 s / 2,171 | 8.321 / 8.666 / 8.814 / 8.945 | 0 | 484.500 |

| Native scenario | GPU pass | Baseline p50 / p95 / p99, ms | Current p50 / p95 / p99, ms |
| --- | --- | ---: | ---: |
| UI | Compute | 11.9956 / 12.8864 / 13.3254 | 2.2975 / 2.9982 / 5.0041 |
| UI | Raster visibility | Not present | 0.2102 / 0.4482 / 0.8165 |
| World | Compute | 12.0270 / 12.6623 / 12.9910 | 2.4834 / 2.9053 / 3.1248 |
| World | Raster visibility | Not present | 0.2161 / 0.3279 / 0.6113 |

GPU pass medians are not a measured total-frame percentile. Native pacing also explains why
wall-frame medians do not fall in proportion to GPU pass latency. Baseline/current GPU sample
counts after warmup were 1,160/2,409 for UI and 940/1,911 for world; asynchronous samples are not
one-to-one with wall frames.

Current UI CPU extraction p99/max was 0.5291 / 2.0888 ms and UI sync p99/max was
0.1594 / 0.5462 ms. Baseline maxima were 2.7245 and 0.5362 ms respectively, so these runs do not
show an improvement in every CPU maximum. Current world extraction/UI-sync maxima were
0.5632 / 0.3064 ms. Geometry allocation fell from 138,596,096 to 75,882,496 bytes; the baseline's
zero framebuffer field means that field was not populated, not that framebuffer memory was free.
The slower current UI startup needs repeated comparison before drawing a conclusion.

## Dynamic shadow maps

The selected extension rasterizes dynamic casters into twelve 1024×1024 depth layers, one for
each original soft-light direction. It retains the same triangle geometry, transforms, receiver
offsets, six-to-twelve light refinement, static shadow cache, bounce, optics and shading. Fitted
light projections use current caster bounds every frame. Finite-distance ambiguity falls back
to dynamic ray traversal. The shared ray path also remains the capability fallback and is selected
above the validated 1920×1080 viewport range. This preserves shadow density at higher resolutions
without silently multiplying depth storage; primary rasterization and exact caches still apply.

This is a spatial shadow approximation, unlike the exact static caches. It adds 48 MiB of depth
textures plus 1,744 bytes of projection uniforms. The frozen workload contains 27 dynamic caster
instances. Current nominal framebuffer/cache storage rises from 151,372,800 to 201,706,192 bytes;
retained geometry stays 75,882,496 bytes. Dynamic projection/upload CPU median in the native
world test was 0.0446 ms.

The first unbounded 210-frame mapped submissions failed readback with `BufferAsyncError`, both
as one command buffer and as 210 buffers submitted together. They produced no valid candidate
timing. Twenty-one bounded batches of ten frames completed. Global time between batches includes
CPU wait/readback/recording gaps, so the comparison sums each batch's GPU envelope instead.
All 210 timestamp pairs were valid; warmed within-batch completion statistics have 162 intervals.

| Variant | Sum of batch GPU envelopes / 210, ms | Warm within-batch completion median, ms | Depth storage |
| --- | ---: | ---: | ---: |
| Original rays, before | 2.641 | 2.442 | None |
| Maps 512 | 2.034 | 1.850 | 12 MiB |
| Maps 1024 | 2.066 | 1.878 | 48 MiB |
| Maps 2048 | 2.199 | 2.021 | 192 MiB |
| Original rays, after | 2.623 | 2.429 | None |

A later same-binary three-way comparison measured 2.677 ms for original rays, 2.094 ms for
1024 maps, and 2.907 ms for maps used only on stationary receivers. The narrower policy preserves
exact creature self-shadows but loses the speed gain, so it is rejected. At 2048, memory grows by
192 MiB while sparse errors remain; it is rejected as the default. The 1024 option balances the
measured gain and storage cost. These results include all twelve depth passes and primary raster
visibility, not only the cheaper compute pass.

In the frozen exact half-float reference comparison, 1024 maps differ at 1,629 of 2,073,600 RGB
pixels, with isolated maximum linear-channel difference about 0.304. The 15-frame mutation
comparison also reveals small eyebrow/self-shadow and plant contact-edge differences; higher
resolution reduces their count without eliminating them. Original-ray controls remain exact.
Root inspected full native images, worst uncompressed crops and consecutive native-size face
and plant-contact video crops, plus enlarged versions. Enlarged images expose changing boundary
tones, but no distracting flicker or noticeable degradation was observed at native size in the
reviewed samples. That is the visual acceptance basis, not the percentage of matching pixels.
The 504 native motion state records match and temporal comparison selects zero-frame offset.
Independent H.264 compression limits attribution of video pixel deltas.

Native world/UI wall medians remained approximately 8.33 ms with presentation pacing. The
1024 world compute median was 1.629 ms, dynamic maps 0.804 ms and primary visibility 0.196 ms;
those separate pass percentiles must not be added into a claimed frame percentile. Serial UI
repeat wall p50/p95/p99 was 8.325/8.586/8.696 ms. An earlier UI attempt potentially overlapped
the next process's startup and is excluded in favor of that serial repeat. Final integrated
native repetitions and sustained operation are recorded below.

## Final integrated measurements

Corrected schema-4 same-binary bounded batches measured **2.07544 ms GPU envelope/frame** for
maps versus **2.59807 ms** for original rays. Warm batch envelopes were 1.99951/2.57706 ms/frame;
warm within-batch completion medians were 1.87908/2.42817 ms. Each run has 210 frames, 162 eligible
warm intervals and 17 excluded post-warmup batch boundaries. Every batch passed its own completed
submission wall-time bound. Invalid batches retain raw counters/reasons and suppress throughput;
the regression test proves that a 118 ms GPU span cannot be accepted against 110 ms of wall time.
These throughput figures are not isolated per-frame latency or presentation FPS.

The following final native repetitions use matched optimized development builds and corrected
timing. The baseline is still `a19edc9`, with timing/profile changes only. Map, visibility and
compute samples now have equal counts because the third-pass slot starvation was fixed.

| Run | Measured frames / seconds | Wall p50 / p99 / max, ms | Frames >33 ms | Ready, ms | Peak RSS / footprint, MB |
| --- | ---: | ---: | ---: | ---: | ---: |
| Baseline UI | 1,857 / 21.950 | 8.645 / 17.047 / 20.831 | 0 | 489.637 | 486.60 / 948.03 |
| Final UI | 2,643 / 22.025 | 8.329 / 8.684 / 8.930 | 0 | 500.671 | 487.65 / 1184.14 |
| Baseline world | 1,503 / 18.016 | 8.660 / 17.005 / 17.450 | 0 | 476.230 | 496.66 / 956.78 |
| Final world | 2,171 / 18.091 | 8.330 / 8.653 / 10.184 | 0 | 527.356 | 480.36 / 1164.63 |
| Final sustained | 11,133 / 92.775 | 8.329 / 8.687 / 9.863 | 0 | 522.418 | 477.18 / 1122.88 |

The sustained fixture contains 91.5 seconds of authored waits, two complete world/UI cycles
and title/settings/continue transitions. Measured wall time also includes command/frame pacing.
It demonstrates short sustained operation, not thermal endurance across hours or every save.

Final UI compute/map/visibility medians were 1.52375/0.796584/0.196875 ms. World medians were
1.610625/0.803541/0.195375 ms. Sustained compute p99/max was 2.510375/9.225333 ms; its wall max
remained 9.863084 ms. Separate pass percentiles are not a total-frame percentile. Sustained
extraction/UI-sync CPU maxima were 1.934167/0.531958 ms, and projection/upload max was 0.208 ms.

Memory is an explicit speed tradeoff. Retained geometry falls from 138.60 to 75.88 MB, while
intermediate visibility, exact caches and maps occupy a nominal 201.71 MB. Process RSS is similar
or lower in these runs, but macOS peak process footprint increases by about 208–236 MB in the
matched short scenarios. RSS, process footprint and nominal GPU allocations measure different
things; none alone is a portable total driver-memory counter. These results do not claim lower
overall memory usage. Startup repetitions are approximately 0.48–0.53 seconds; an earlier first
mapped run took 1.11 seconds, so warm repetitions do not establish cold-start equivalence.

### Final visual and correctness acceptance

Fresh final captures cover all 36 native 1080p still states. Against the instrumented baseline,
99.915633% of RGB pixels match exactly and mean absolute channel difference is 0.016787/255;
29,002 pixels have a channel difference above 8 across the full 74,649,600-pixel set. Independent
inspection reviewed 12 full images and five detailed comparisons, covering gameplay, title,
Normal/Large text, keyboard, settings, rename, speech, food, toys and context panels. The final
differences match accepted raster and sampled shadow edges, with no new material regression.
See [final still inspection](performance/20260909-efficiency/visual/final-still-inspection.md).

The fresh 504-frame motion sequence matches all 504 state/audio records, 480 events, 14 inputs
and 7 markers. Seven raw checkpoint MAEs range from 0.005838 to 0.015628/255. Encoded video
SSIM mean/minimum is 0.994994/0.987099, including independent compression differences. Root
inspected final full gameplay/title images, the worst decoded pair, the overview and selection
transition filmstrip, supplementing the preceding consecutive facial/contact-shadow review.
No broad ghosting, stale lighting, shifted text or broken transition was observed in those
samples. This does not substitute for human full-speed interaction or every-frame perception.

`cargo xtask verify` passed 596 tests, with seven GPU-dependent tests deliberately ignored by
the headless gate, plus 27/27 dialogue fixtures, 11/11 STT fixtures and spoken-input replay.
Explicit GPU runs passed 6,435 traversal rays, exact 15-frame cache comparisons, one-sample
cache coverage, equal-distance bounce ordering, bounded 15-frame map comparisons, and a
two-blocker finite-distance regression. The last fixture verifies a stored blocker at t=50
cannot hide another at t=10 from a ray limited to 35, and a lone t=50 blocker stays unblocked.
Portable shader checks cover MSL, HLSL and SPIR-V without optional rendering capabilities.

The final host run used `cargo xtask dev --fake-ai` for gold captures; UI and motion used its
byte-identical copied `dev-perf` executable, SHA-256
`fdefbb2e606ffb24ec0f04c10eead02c06e794ddec7a04a6ed15b3d0ee21ec79`.
Independent review covered traversal, caches, resource lifetime, map projection/fallbacks,
UI retention and timing. Both final timing findings were fixed and re-reviewed.

## Measured rejections

| Candidate | Matched evidence | Decision |
| --- | --- | --- |
| Dedicated index-only any-hit stack | Passed 6,435 oracle rays and pixel equality; 10.915 / 10.919 ms versus 10.811 / 10.798 ms in the original batched compute schedule | Rejected standalone; historical counters carry the caveat above |
| Cache only first coverage sample | Saves 24.88 MB; approximately 3.62 ms versus 3.0 ms with smaller stacks | Keep both coverage records where binding limits permit |
| Conservative 128×128 dynamic-shadow bitmap | 2.9279 versus 2.9404 ms, under 1% gain; adds 64 KiB and CPU projection work; exact pictures and mutation oracle | Rejected |
| Alternative workgroups | 8×4, 16×4, 16×8, 4×8, 32×2 and 32×4 did not beat 8×8 | Keep 8×8 |
| Conservative compressed BVH4 | Controls 2.537697 / 2.615635 ms; candidate 3.953284 / 3.946288 ms; node data 4.759 MB versus 9.507 MB | Reject the measured variant: memory falls, throughput regresses by about 53% |
| Compressed BVH8 with fixed octant order | Controls 2.546078 / 2.603623 ms; candidate 3.461518 / 3.445634 ms; node data 5.182 MB versus 9.507 MB | Reject the measured variant: about 34% slower; conservative bounds and backend translation tests passed |
| Separate shadow compute pass | Controls 2.619426 / 2.595442 ms; candidate 2.553358 / 2.503142 ms; exact half-float RGB with zero, one and two cache samples | Reject standalone: 0.079 ms mean gain adds 16.59 MB retained output and up to 33.18 MB/frame transfer |
| Integer avalanche hash for water | At water times 0/12/47 s, original 2.611–2.540 / 2.563 / 2.564 ms; candidate 2.619 / 2.536 / 2.609 ms | Reject: no repeatable material gain; caustic highlights visibly move despite preserved overall character |
| Conservative ellipsoid rejection before rounded-creature BLAS | Controls 2.602025 / 2.644642 ms; candidate 2.625408 / 2.639381 ms, summed bounded-batch GPU envelopes/frame; all four image comparisons exact | Reject: no repeatable gain; 13 eligible instances and 180,972 instanced triangles, 2,496 bytes of bounds |

The object-local grid/DDA experiment used the real triangle geometry and conservative cell
references. `grid-summary.json` records paired same-binary controls and three densities:

| Grid density | GPU envelope / 210 frames, ms | Warm completion p50 / p95, ms | Grid bytes |
| --- | ---: | ---: | ---: |
| BVH control before | 2.596550 | 2.415292 / 3.028458 | Grid inactive |
| 0.5 | 4.684524 | 4.360667 / 5.342959 | 6,594,956 |
| 1.0 | 4.880505 | 4.562708 / 5.603375 | 9,373,312 |
| 2.0 | 4.527650 | 4.220334 / 5.179500 | 12,852,648 |
| BVH control after | 2.598623 | 2.423979 / 3.057625 | Grid inactive |

The fastest grid was about 74% slower than its controls. All grids covered 500,904 triangles
across 36 mesh grids; their cell/reference counts ranged from 252,458/1,143,247 to
790,976/1,630,634. The original BLAS arena occupied 13,300,224 bytes. Controls matched exactly;
each grid differed in 10 of 2,073,600 RGB pixels, with maximum linear channel difference
0.0007019043. The grids are rejected on performance, with those small numerical differences
recorded rather than described as exact equality. This historical grid summary includes the
warmup-boundary completion interval; the current schema uses only fully post-warmup intervals.

Near-rectangle pairing found 155,432 candidate pairs covering 62.1% of triangles, but used an
eligibility epsilon and did not prove exact replacement. Raster visibility already handles
primary surfaces. This audit is not an accepted topology change or a measured speedup.

## Completion scope and remaining limits

The measured wide-BVH rejections do not reject all possible layouts, and the grid rejection
does not establish that all voxel-specific representations are unhelpful. The baseline already includes SAH, category
roots, shared meshes, planar reduction and exact invalidation caches.

Implementation, independent review, native repetitions, short sustained operation and fresh
visual/motion validation are complete for this measured workload. Windows/Linux native execution,
lower-end GPUs and higher-resolution mapped-shadow quality remain unmeasured. Higher resolutions
retain original shadow rays. No release binaries, installers or complete model bundles were rebuilt.

The final independent opportunity review found only the enclosing-ellipsoid candidate worth
one more bounded test; that test is now rejected. Primary-only costs about 0.250 ms, removing
reflections saves only about 0.05 ms, and the measured shadow split saves 0.079 ms while adding
memory traffic. CPU extraction/UI work is comfortably below the transition budget. Smaller caches
have a demonstrated GPU cost. These are the stopping reasons for this measured workload; they do
not establish a mathematical maximum or make future device/workload-specific improvements impossible.

Optional hardware ray queries are not the selected adapter: the pinned
[wgpu 29 feature documentation](https://docs.rs/wgpu/29.0.0/wgpu/struct.Features.html#associatedconstant.EXPERIMENTAL_RAY_QUERY)
lists the experimental path for Vulkan, so it cannot accelerate this measured Metal host through
the existing abstraction. Ordinary rasterization and compute already provide a shared path.
An optional hardware-ray adapter needs a supported device and its own measured benefit before
adding another acceleration-structure lifecycle. No RTX requirement is introduced.

## References

- [First renderer performance pass](renderer-performance.md),
  [raytraced aquarium architecture](raytraced-aquarium.md), and
  [game design philosophy](game-design-philosophy.md) provide the local baseline and constraints.
- [The Visibility Buffer](https://jcgt.org/published/0002/02/04/) describes raster visibility IDs
  followed by shared surface reconstruction and shading. Its published performance is not a
  Beastie result.
- [Efficient Incoherent Ray Traversal on GPUs Through Compressed Wide BVHs](https://research.nvidia.com/publication/2017-07_efficient-incoherent-ray-traversal-gpus-through-compressed-wide-bvhs)
  motivates conservative compressed bounds and traversal changes. The measured BVH4 variant
  above is one implementation, not a rejection of the paper's full design space.
- [TinyBVH](https://github.com/jbikker/tinybvh) provides primary implementation references for
  ordinary-compute traversal and multiple acceleration layouts. Its benchmark results are not
  transferred to Beastie.
