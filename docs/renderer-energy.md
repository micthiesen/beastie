# Renderer energy and tail-latency experiments

Started September 9, 2026, against `41c7006`. This continues the
[renderer efficiency review](renderer-efficiency.md), with energy demand and p99
frame times as the priorities. Native evidence is Apple M5 Max / Metal. Ordinary
GPU support, foreground animation cadence, deterministic gameplay and perceived
image quality remain requirements.

## Final GPU result

Paired runs of the integrated implementation reduce the 1080p GPU batch envelope
from 2.238/2.183 ms to 1.514/1.504 ms per dispatched frame, approximately 32%.
Excluding warmup, the respective envelopes are 2.158/2.148 and 1.470/1.464 ms.
Warmed completion medians are 1.950/1.909 versus 1.336/1.327 ms. All timestamp
samples passed validation. These are GPU throughput measurements, not native
presentation latency. Test executables for `41c7006` and the final implementation
were measured in control/candidate/candidate/control order on the same device.

The final integrated map oracle covers fifteen scene mutations at 1080p and
twelve consecutive blink/movement frames at both 640×360 and 3840×2160. This tests
LOD and adaptive map density together. Relative to original rays, low-resolution
frames differ at 589–648 RGB pixels, with mean error 0.0135–0.0147/255; 4K differs
at 4,824–5,306 pixels, with mean error 0.00350–0.00378/255. LOD alone changes at
most 13 and 202 pixels respectively. Visual inspection covered the full low-resolution frame,
twelve-frame native-pixel facial comparisons and six 4K floor-shadow comparisons.
Expression, broad shadows and contact appearance remain intact; sparse edge
differences are accepted. These checks do not claim pixel identity.

## Native frame tails and memory

The final capture-free runs use the same bounded custom profiler and optimized
development build. Requested presentation is `AutoNoVsync`; observed native
cadence is about 120 Hz. Timings include presentation and host scheduling.

| Scenario | Measured frames | Seconds | p99 ms | p99.9 ms | Maximum ms | Frames >16 ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| World 1 | 2,170 | 18.085 | 9.190 | 9.720 | 10.100 | 0 |
| World 2 | 2,170 | 18.084 | 9.177 | 9.542 | 10.259 | 0 |
| UI | 2,643 | 22.026 | 9.245 | 9.722 | 10.166 | 0 |
| Churn 1 | 2,104 | 17.534 | 9.206 | 9.504 | 10.048 | 0 |
| Churn 2 | 2,105 | 17.543 | 9.242 | 9.939 | 13.418 | 0 |
| Sustained | 11,132 | 92.768 | 9.253 | 9.695 | 10.670 | 0 |

Across these 22,324 measured frames, none exceeded 16 ms. The longest run spans
about 93 seconds, not a thermal-endurance test. Maximum recorded shadow-preparation
CPU phases range from 0.319 to 1.737 ms across these runs. Warm-launch renderer
readiness is 495–509 ms. Background preparation removes the cold mesh/title stalls
seen in the rejected synchronous implementation.

The original baseline executable reported lower p99, around 8.64–8.70 ms, but
retained different profiling overhead and objc2 development checks. Rebuilding
the `41c7006` renderer with only the same profiler cleanup and objc2 profile
override gives the appropriate control:

| Matched instrumentation | Baseline p99 / max ms | Final p99 / max ms | Task CPU W, baseline / final |
| --- | ---: | ---: | ---: |
| World | 9.274 / 10.076 | 9.247 / 9.706 | 0.474 / 0.352 |
| Churn | 9.309 / 10.474 | 9.205 / 9.762 | 0.483 / 0.313 |

All four runs have zero frames over 16 ms. This supports maintained p99 near
9.2 ms, not a strong p99 improvement from a single pair. Report-mode task CPU
energy falls about 26%/35% in these pairs. The ordinary-game energy measurements
below are separate and avoid profiler overhead entirely.

At 1080p, standard-scene retained GPU geometry grows from 75,882,496 to
80,910,192 bytes; indexed shadow bytes are already included. Nominal framebuffers
shrink from 201,706,192 to 182,833,568 bytes, including shadow depth reduced from
48 to 30 MiB. Together these reported allocations fall about 13.85 MB, or 5%.
Churn geometry reaches 82,206,712 bytes. This is not a reduction in process memory:
matched world peak footprint rises from 1,187,530,216 to 1,348,093,224 bytes, and
churn rises from 1,136,346,528 to 1,352,402,336 bytes. The sustained peak is
1,368,032,696 bytes. Retained CPU geometry, staging and transient driver allocations
are a real memory tradeoff; the measurements do not isolate each contributor.

## Measurement contract

Run GPU experiments serially, with compilation outside measurement intervals.
Use paired controls and the existing bounded ten-frame offscreen batches. Their
GPU envelope per frame measures throughput, excluding inter-batch CPU gaps; it
does not measure display cadence or isolated latency. Native world, UI and
sustained scenarios establish wall and named-pass tails separately. Never add
percentiles of overlapping passes.

Whole-device and GPU watts/joules are unavailable in this session. Unprivileged `powermetrics`
requires superuser access; Xcode's Power Profiler rejects macOS; readable battery
telemetry remained unchanged across repeated samples while connected to AC.
Unprivileged `proc_pid_rusage` does expose live, kernel-accounted task CPU energy
through `ri_energy_nj`, documented by [Apple's Recount implementation](https://github.com/apple-oss-distributions/xnu/blob/main/doc/observability/recount.md).
The sampler verifies process identity and records raw counters; CPU time uses
the measured Mach timebase. CPU energy excludes GPU, display and other processes.
GPU execution demand and suppressed hidden render work remain proxies, not
battery-life measurements. Profiling itself is opt-in through `--render-report`;
ordinary gameplay does not allocate timestamp queries or read them back.

The combined resize/capture/report test exposed a Metal counter-buffer allocation
failure on entering fullscreen. Ordinary resize/fullscreen capture passed. The
old report path also enabled Bevy's redundant diagnostic collector, whose pending
frame pool can grow without a limit and allocates 256 timestamp queries per frame.
The report path now uses only the existing bounded custom collector: twelve slots,
two queries each. Legacy `ray_passes` stays empty with an explicit unavailable
description; named GPU passes, CPU phases and wall intervals remain available.
The error log cannot identify the failing allocation by itself. Two native repeats
after removing the redundant collector completed all nine size/fullscreen captures,
with all three custom GPU passes available. Diagnostic
overhead changes apply only to report-mode comparisons, not ordinary CPU-energy
or GPU batch measurements. A world diagnostic probe overlapped image analysis and
is retained as troubleshooting evidence, excluded from clean final timing results.

## Candidate ledger

| Candidate | Current evidence | Decision |
| --- | --- | --- |
| Workgroup-local full transport reuse, 2×1, 2×2, stricter 2×2, 4×2 | 2.58–2.69 ms versus 2.04–2.10 ms controls; portable shader translation passes | Reject these implementations; coordination and fallback outweigh saved work |
| Workgroup-local bounce-only reuse | About 2.55 ms versus 2.04 ms rearranged control | Reject these implementations |
| Separate coarse transport producer and full-resolution resolve | 2.27–2.31 ms versus 2.07 ms control; retains local coverage and spatial detail | Reject these implementations |
| Multiple neighboring pixels per invocation | 2.38–2.88 ms versus 2.09 ms control; avoids barriers and intermediate buffers | Reject these implementations |
| Smooth water transfer reuse within a pixel | 2.14 ms versus 2.12 ms split control and 2.10 ms original; detailed surface ripples remain independent | No demonstrated gain; reject |
| Persistent static shadow maps for moving receivers | Initial 512/1024 trials about 1.8–1.9 ms; moving mutation oracles and native motion comparison pass | Keep with adaptive density, 512 at 1080p |
| Static maps for all receivers | Many more differing pixels without additional warm speed benefit | Reject |
| Indexed position-only shadow geometry | About 5% GPU saving, bit-identical images; 3.97 MB extra buffers before LOD | Keep with retained background preparation |
| One depth atlas pass for twelve light views | About 8% additional GPU saving over indexed control, bit-identical images; unchanged depth bytes | Integrated |
| Sixteen-bit shadow depth | Same combined speed; 0–4 RGB pixels differ per 1080p mutation frame; 60→30 MiB combined depth storage | Integrated after native motion review |
| Single-map blocker-search soft shadows | 1.85 ms versus 2.12 ms control, but removes the ball's broad shadow and weakens bell contact; 7,543 pixels differ by >8/255 | Reject visible regression; slower than the accepted combined approach anyway |
| Skip unused normal work on unlit scenery | 2.051/2.060 ms versus 2.053/2.067 ms controls | No demonstrated gain; keep existing code |
| Confirmed hidden-window policy | Native minimization/Hide already removes Bevy render views; host still runs at 60 Hz. Hidden-only 10 Hz lowers accounted CPU energy 0.136→0.021 W | Keep hidden pacing; do not attribute existing GPU suppression to the new hook |
| Shadow-only mesh simplification | 197,364→88,120 shadow triangles at a 0.002 world-unit metric cap; about 10% further GPU saving; 54 pixels differ in the frozen 1080p still; 63-frame oracle and native tail tests pass | Keep with original geometry while background preparation is pending |
| Map density scaled to viewport | 640×360 warm 0.63–0.65 ms versus 0.71–0.74 ms, depth 30→7.5 MiB; 4K 5.94–5.98 ms versus rays 8.71–9.07 ms, using 120 MiB depth | Keep after paired twelve-frame blink/motion review and six native sizes plus fullscreen/restore |
| Vertex-cache index ordering after simplification | Warm GPU cadence differs by less than 0.3%; four images bit-identical | Reject: no material repeatable gain |
| Disable objc2 development checks in `dev-perf` only | Repeated ordinary task CPU energy 0.272–0.293 W versus 0.511–0.534 W controls, with higher CPU time | Keep scoped override |

Within-pixel lighting reuse already existed at the baseline. The new spatial
experiments share between pixels. Their approximation cannot be proved safe by
matching surface IDs and normals alone: shadow/contact boundaries can cross an
otherwise flat surface. Full-mode timing rejection does not establish that all
possible implementations of spatial reuse are unhelpful.

`renderer-perf-churn.jsonl` adds first-use and repeated food meshes, cleanup,
and title/return transitions. Existing world/UI scenarios do not exercise food
caster membership changes. Retained shadow geometry must be judged on those
rebuild tails as well as warmed animation.

The first integrated churn run illustrates why p99 alone is insufficient. Wall
p99 was 8.687 ms versus 8.697 ms for the baseline, but p99.9 rose from 9.073 to
20.708 ms, with five frames over 16 ms. The maximum shadow preparation CPU phase
rose from 0.080 to 14.599 ms because adding one mesh welded the whole dynamic set.
This result was rejected. Retaining individual meshes by immutable source identity
removed repeated welding, but synchronous simplification still added an 18 ms title
frame. One background job now prepares the replacement while original geometry
draws. Four matched churn runs produced p99.9 of 8.994/8.987 ms for LOD versus
9.045/8.927 ms controls. LOD maxima were 9.787/9.146 ms, with no frame over 16 ms;
the measured shadow preparation phase stayed below 0.668 ms. These prototype
results cover 2,105 measured frames per run; final integrated evidence is separate.

The LOD cache holds immutable source `Arc`s so arena relocation can reuse work
without confusing recycled pointers with old geometry. It counts vector capacities
and retained source triangles. On preparation completion, it preserves current active
meshes and prunes inactive entries toward 32 MiB/128 meshes. Cached data survives
idle or disabled intervals, so this is not a global cap. The standard active set
accounts for 36,351,152 bytes (34.67 MiB), including sources shared with the scene;
food adds 2.00 MiB and the title transition reaches 43.87 MiB. Transient staging is
additional. Only one task can run;
results upload only if their mesh key is current. Draw-time transform bounds select
retained original indices for unusually scaled objects. The standard indexed GPU
stream grows from 3,970,256 to 5,027,696 bytes because it retains both index ranges.

The 1080p LOD review covered fifteen scene mutations and forty-eight consecutive
frames. At most 64 RGB pixels changed per motion frame, with mean linear RGB error
at most 3.02e-7. Native-pixel face and contact-shadow sequences showed no material
loss. The simplifier's metric is not a maximum surface-distance proof, so numerical
bounds complement visual review rather than replacing it.

Depth16 also requires a conservative finite-distance guard. A blocker at
35.0001 units in a 100-unit projection span can quantize to 34.99962, apparently
inside the original 35-unit ray limit. Both map helpers therefore trace the
original ray within one depth step plus a small arithmetic margin of that limit.
The explicit GPU fixture checks both helpers at 34.9999, 35, 35.0001 and 50 units,
with and without a nearer blocker. Depth quantization remains an image
approximation elsewhere; this guard preserves the finite-limit decision.

Density follows the uniform fit of the authored 16:9 scene, using the smaller
width/height ratio, rounded upward to a power of two. Dynamic/static maps range
from 512/256 to 2048/1024. Wide or tall margins do not increase scene pixel
density. Viewports beyond 3840×2160 or incompatible adapter limits retain rays;
there is no automatic reduction of primary resolution, lighting samples or
foreground frame cadence.

The density comparison covers a full blink and gentle movement at both endpoints.
At 640×360, 574–641 pixels differ per frame, with mean absolute RGB error
0.0131–0.0147 on the 0–255 scale. At 4K, 4,777–5,257 pixels differ, with mean error
0.00345–0.00374. Enlarged crops expose sparse contact-edge differences; native-pixel
face sequences preserve expression and contact appearance. These are reviewed
approximations, not bit-identical pictures.

## Hidden-window behavior

Only an explicit primary-window occlusion event reduces host updates to 10 Hz.
The previous focused/unfocused Winit policies are retained and restored on an
unoccluded or focused event. Focus loss alone keeps the visible policy. Missing
platform occlusion events fail open. Script, capture, feel, report and smoke
automation are not suppressed or throttled by this policy; the framework can
still withhold a render view when the OS does not provide one.

Native verification used a uniquely identified scratch app with counters at
render entry, hidden return and compute dispatch, plus a main-world heartbeat.
The temporary fixture-argument relaxation prevents save writes during unscripted
testing. Neither the counters nor the argument relaxation enters production.
Both minimize and native Hide froze render-entry and dispatch counts before the
custom node, while hidden-return count stayed zero. Thus Bevy/macOS already
suppressed those GPU submissions; the new measured saving is host CPU demand.

At 1920×1080, Fifo presentation, the 60 Hz hidden control accounted for 0.13595
CPU W and 0.32317 CPU-core equivalents over 48.19 seconds. The 10 Hz candidate
accounted for 0.02113 W and 0.05298 cores over 33.10 seconds, about 84.5% less
accounted CPU energy per second. Its host update rate was 9.9997 Hz and the game
clock advanced 995.4 ms per wall second. Native restore returned to approximately
120 dispatches per second with changed pose, current animation and intact UI.
Restore-transition samples were excluded using heartbeat timestamps. These are
short task-CPU measurements on this Mac, not GPU watts or battery-runtime claims.

The first native visible-unfocused attempt did not establish focus loss, so it
is inconclusive. Unit tests cover focus loss without occlusion and exact visible
policy restoration. CUA target inspection can focus or restore the target;
hidden energy intervals therefore used raw counters without observing the target.

## Ordinary CPU energy and development overhead

At 1920×1080 with Fifo presentation and the same familiar-ball fixture, ordinary
final-renderer controls accounted for 0.534/0.511 CPU W versus a matched `41c7006`
repeat at 0.782 W, approximately 33% lower. Each interval collected 45 seconds and
excluded its first ten seconds. These unscripted scene intervals are matched
workload classes, not identical frozen frames. Runtime cadence was not instrumented
in these ordinary measurements; viewport and policy follow source, settings and
native framing. Scripted frame-time evidence is collected separately.

The native stack also showed Objective-C signature checks on Metal draw calls.
A version-specific Cargo override disables `objc2@0.6.4` debug checks only in
`dev-perf`. Repeated candidates accounted for 0.293/0.272 CPU W, about 46% below
the final renderer controls. CPU time increased to 0.605/0.617 core equivalents,
versus 0.525/0.551, so this is not a claim of less executed CPU work. Core placement
and frequency effects were not measured. Separate stack samples no longer showed
the prior wgpu signature-check path. Application assertions, overflow checks and
ordinary dev/test verification remain enabled. All objc2 debug checks in this
profile are affected, including nil/class checks, not just signature parsing.

Release builds already omit those development checks. The combined roughly 64%
CPU-energy difference from the baseline therefore must not be presented as a
shipped renderer improvement or a whole-device battery saving. The renderer's
approximately 33% ordinary CPU-energy reduction and the additional development
profile reduction are separate findings. Raw intervals, source/compiler hashes
and the inspection are in the CPU evidence archive.

## Final verification and visual acceptance

`cargo xtask verify` passes 611 tests, 27 dialogue fixtures, 11 STT fixtures and
deterministic replay. The ordinary development build emits a nonfatal macOS
linker warning about its large `__eh_frame` section; optimized native validation
completes. Explicit GPU tests also pass for production traversal against brute
force, exact and one-sample cache invalidation over fifteen mutations, finite
shadow-ray limits, static bounce tie order and combined LOD/map-density images.
An initial diagnostic oracle binding mismatch at slots 13/15 was corrected before
rerunning traversal and the full gate; it did not affect the production renderer.

The baseline/final native comparison covers 36 still states and 504 motion frames.
State and audio traces match for all 504 frames; 480 event entries, 14 inputs and
seven markers also match. Still differences remain sparse: worst mean RGB error
is 0.002942/255; that still has 1,215 changed pixels, 464 differing
by more than 8/255, and maximum channel difference 18. The largest channel
difference anywhere in the still suite is 49/255 at a sparse title-return shadow
edge, also reviewed at native pixel size. Decoded motion SSIM averages
0.996337 with a minimum of 0.994322, including video-compression differences.
Native-pixel face, floor-contact and worst-motion comparisons preserve expression,
surface detail and broad shadows. Approximate shadow edges are accepted.

`cargo xtask dev --fake-ai` with the gold-reference script passes on the final
source. Seven of its eight images match the preceding final captures exactly in
RGB; title-return differs at 55 pixels, maximum 24/255, without a material visual
change. The earlier 36-still/motion suite and map oracles preceded only the
diagnostic cleanup/test-binding correction; the rendering algorithm is unchanged.
Binary and source manifests retain those distinctions.

All six window sizes from 640×360 through 3840×2160, native fullscreen at
3024×1898 and windowed restoration pass. Two repeats with bounded profiling
also pass. A narrow water-colored strip at the top of the taller fullscreen frame
is pre-existing: the original `41c7006` renderer and final frame are RGB-identical
across the entire top 190 rows. The baseline comparison adds only scripted
resize/fullscreen aliases to the matched control and retains source hashes.

Source review covers shadow-cache ownership and invalidation, background
preparation, map limits and fallback, hidden pacing and profiling, with independent
review of the asynchronous preparation and final collector cleanup.
Windows, Linux and lower-end GPU native execution remain unmeasured. Portable
shader translation and headless tests support compatibility but do not establish
cross-device performance or appearance. The next useful measurement is on those
devices, with the same scenes and instrumentation. No RTX dependency, primary
resolution reduction or foreground frame cap was introduced.

## Research informing the experiments

- [Intel coarse pixel shading](https://www.intel.com/content/www/us/en/developer/articles/technical/coarse-pixel-shading-with-temporal-supersampling.html)
  separates shading rate from coverage. The experiment here keeps original
  coverage and does not introduce temporal accumulation.
- [id Software / Xbox variable-rate compute shading](https://developer.microsoft.com/en-us/games/articles/2026/04/variable-rate-compute-shaders-doom-the-dark-ages/)
  reduces compute work by allowing entire waves to retire. Merely disabling some
  lanes does not guarantee a proportional saving. This motivates testing a
  separate coarse producer after the workgroup-local approach regressed.
- [NVIDIA graphics pipeline performance](https://developer.nvidia.com/gpugems/gpugems/part-v-performance-and-practicalities/chapter-28-graphics-pipeline-performance)
  explains indexed vertex reuse; the shadow-stream experiment requires no mesh
  shader or ray-tracing extension.
- [Apple attachment load/store actions](https://developer.apple.com/documentation/metal/setting-load-and-store-actions/)
  motivates measuring render-pass memory work. A larger atlas is a hypothesis,
  not an assumed optimization.
- [meshoptimizer](https://github.com/zeux/meshoptimizer) and its
  [Rust wrapper](https://github.com/gwihlidal/meshopt-rs) supply the shadow-only
  simplification experiment. Its reported error is an optimization metric, not
  a proof of maximum surface distance or perceptual equivalence. Exact primary
  meshes and native motion comparisons remain the visual authority.

The [evidence guide](performance/20260909-energy/README.md) links experiment
sources, rejected prototypes, raw reports, reviewed images, source/binary hashes
and reproduction commands. The goal of this pass is met by measured reductions
without a material visual regression; these results do not establish a universal
optimum or a whole-device battery-life percentage.
