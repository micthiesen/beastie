# Unified renderer review

## Scope and baseline

This pass migrates the complete aquarium, care deck and text to ordinary compute ray tracing.
The baseline is commit 166fce6 and the surface evidence recorded in
[feel-review-voxel-surfaces.md](feel-review-voxel-surfaces.md). Simulation and input ownership
are preserved. Rendering differences are intentional; temporal acceptance uses identical canonical
scenarios. Screenshots and dense frames establish visual evidence, not human full-speed perception.

## Implementation iterations

`ray-first` proves the initial unified path at 1920x1080, including outline glyph geometry. It
exposed excessive self-shadow detail. `ray-bias-02` and `ray-no-shadows` isolate shadow contribution.
Geometric origin offsets, wider deterministic area-light samples and smooth-normal orientation
reduce accidental local dark patches. Independent review of `ray-reuse-04` accepted glyph layout,
face readability and silhouette continuity, but retained overly crunchy skin as a material finding.
The skin response now uses restrained diffuse fill to soften local occlusion. This is an authored
approximation, not a claim of physically simulated subsurface transport.

The old raster scene schedule, lights, UI/text rendering plugins and active sprite renderer dependencies
were removed. Bevy retains infrastructure and CPU mesh/material descriptions. Every visible glyph
is intersected geometry; the final fullscreen triangle only transfers computed radiance.

## Code review

Round one accepted a deterministic startup-clear improvement. A proposed unsupported-glyph loss
was rejected after checking the actual embedded font: glyph zero contains five contours and is
visible. A new regression covers unsupported emoji and CJK through shaping and mesh creation.
The BVH constructor now states and enforces the u32-indexed median-tree bound: at most 31 deferred
siblings fit in 32 shader stack slots. No silent overflow or dropped branches are allowed.
Round two inspected near-first traversal, stable chunk uploads, compaction, extraction order,
startup/capture readiness and platform features; no new actionable finding was reported.

## Performance method and preliminary results

`--render-report PATH` measures Time<Real> wall-frame intervals after readiness and warmup.
These include CPU, GPU backpressure, pacing and any screenshot/readback cost. They are not GPU
pass timings. This host is an Apple M5 Max with 40 GPU cores using Metal. It does not establish
low-end or Windows/Linux performance. Initial 1080p median frame time was about 42 ms; reusing
illumination across nearby coverage samples brought it to about 25 ms. Two world coverage samples,
four UI samples and nearer-first traversal reached about 16.7 ms median with a 25 ms p95 in the
preliminary `ray-smooth-06` run. Capture-free final measurements are recorded below.

## Final review and native evidence

A final lifecycle review accepted a bounded startup-readiness wait. Capture now fails through the
existing error/exit path after 60 seconds without a ready renderer; a regression verifies that
waiting does not consume the eight successful-render warmup frames. The follow-up review cleared
this fix. General and focused geometry/text reviews found no remaining accepted material defect.
Optional upstream packages can remain in Cargo.lock; the active dependency graph has no Bevy
UI, text or sprite renderer.

Final skin/world stills are in `target/captures/ray-soft-skin-07` (five 1920x1080 poses), and all
28 Normal/Large UI states in `target/captures/ray-ui-07` passed independent native-size review.
The final host development command also completed with fresh captures in
`target/captures/ray-final-native`. Canonical recaptures passed manifest hash/size validation:

| Recording under target/feel | Frames / seconds | Comparison with surface baseline |
| --- | --- | --- |
| ray-accepted-interaction-chain/interaction-chain | 3,757 / 62.617 | Every creature state, input, event and audio cue/command matches |
| ray-accepted-interaction-chain/dialogue-races | 2,179 / 36.317 | Every creature state matches; superseded replies and subtitle toggles remain correct |
| ray-accepted-quiet-observation/quiet-observation | 10,812 / 180.200 | Every creature/view state, input, event and audio timeline matches |
| ray-accepted-bad-conditions/bad-conditions | 2,603 / 43.383 | Every creature state, 2,593 events, 21 inputs and cue/command timeline matches |

Independent visual and causal reviewers inspected overviews, dense event strips and native frames
across these recordings. Sleep, cave use, ball/bell contact, refusals, interruption and recovery
retain connected motion and readable acting. Asynchronous playback completion differs by one or
two frames in a few comparisons; content, authoritative behavior and caption expiry remain correct.
A shortened failure caption also exists in the baseline and is not glyph clipping introduced here.

`cargo xtask verify` passes 483 tests, 27 dialogue fixtures, 11 recognition fixtures and spoken-input
replay. Both shaders validate with no optional Naga capabilities and translate to SPIR-V, HLSL and
MSL. Only macOS/Metal was exercised on hardware. These checks do not establish Windows/Linux
driver behavior, low-end performance, human full-speed perception or physical audio/controller feel.

## Actual native input

`target/feel/ray-native-complete` records a 90-second run driven by actual macOS pointer and keyboard
events, with 13 window screenshots in `host-screens`. Independent visual review confirms complete
Large-mode entry of “Hello Mop, how is the water today?”, submission and readable reply, food tray,
world placement, creature context, settings tabs and restored Normal text. Large/reduced-motion
settings are exercised together. Persistent settings and backup SHA-256 remain unchanged before
and after the run. This is actual OS-event automation, not physical hand/controller testing.

The input trace contains 5,405 frames over 90.083 seconds: `talk_accepted` at 15.800s, a berry
drop at 19.833s at world position (5379, 2417), creature context at 21.566s and F4 `comforted`
at 23.250s. Settings restore and return to play complete at 27.166s. This direct input recording
has no canonical manifest; manifest validation applies only to the four canonical suites above.

## Final capture-free timing and memory

Final debug-binary runs used the normal 1920x1080 player window, fake AI and no frame readback,
with no concurrent build or agent native run. Start from `voxel-surfaces.jsonl` and
`voxel-craft-ui.jsonl`, replacing each capture command with a 750ms wait. Invoke the built game
with `--fake-ai --script <replacement.jsonl> --render-report <report.json>` under macOS `time -l`.
The reports use 30 warmup frames and nearest-rank percentiles. This small acceptance sample is
not a sustained thermal test, GPU-pass profiler, minimum-spec claim or real-inference benchmark.

| Sequence | Measured frames | Wall p50 / p95 / p99 (ms) | Logical geometry arena | Maximum process RSS |
| --- | --- | --- | --- | --- |
| world | 1,075 | 16.66 / 25.10 / 25.29 | 98,705,920 bytes | 567,246,848 bytes |
| ui | 1,312 | 16.69 / 25.33 / 27.44 | 104,604,160 bytes | 568,803,328 bytes |

Both runs use Apple M5 Max / Metal (40 GPU cores). macOS also reports peak process memory
footprints of 1,021,232,520 bytes (world) and 1,019,659,656 bytes (UI). These OS metrics are not
measured GPU allocation. The geometry arena is an aligned logical extent; actual buffers round
capacity upward, and textures, staging and driver allocations are additional. Peak instance/TLAS
data is 12,224 bytes. Renderer timing here includes extraction and GPU backpressure, but does not
isolate either cost. The earlier capture-heavy UI p95 near 239ms did not recur in this capture-free
run; it must not be presented as normal UI frame cadence.

All accepted findings in the exercised renderer scope are closed. The next checkpoint is native
Windows/Linux and lower-end hardware acceptance, together with live human perception and physical
input/audio checks. No distribution binaries or model bundles were rebuilt for this migration.
