# Gold fidelity refinement

## Contract

The permanent gold images remain the art authority. This follow-up closes the five
material gaps identified after the first implementation, preserving the simulation,
real controls, accessibility, offline behavior and existing compute renderer.

1. Break the marbled, metallic-looking water surface into irregular smaller reflection
   patches with dark teal intervals, sparse warm glints and quiet continuous movement.
2. Deepen contact and layered lighting around scenery, retain readable creature eyes
   and expression, and make warm practical illumination contrast with cool water.
3. Make settings a dark, legible plate with faint background context, thinner rims,
   stepped corners and less heavy controls. Keep all Normal/Large controls available.
4. Replace slab-like plant and repetitive rock silhouettes with intentional stepped
   fronds and varied clusters, preserving open swimming space and semantic locations.
5. Refine title lettering depth and warm lighting, quiet menu plates, and avoid toy
   silhouettes intersecting the title menu without changing authoritative positions.

Use existing geometry, materials, ray traversal and declarative UI. Investigate cheap
approximations before deferring effects; do not add full physical volumetrics or a
new renderer solely for screenshot equivalence. Bound geometry and ray costs and
compare native wall-frame measurements under the same capture conditions.

## Evidence and acceptance

Baseline is commit `a8581ff`, its committed screenshots under `implemented/`, and a
fresh same-seed motion recording in `/tmp/beastie-gold-refinement-before/`.
Use the existing still scenario for gameplay, selected toy, settings Normal/Large
and title comparisons. `cargo xtask feel --suite gold-reference` packages and validates
the animated scenario, including motion, selection, settings and reduced motion.

At every native pass compare composition, palette, highlight distribution, object
contact, UI restraint, text readability and temporal quiet against the originals.
After the final material change, complete two valid native review passes with no
known material reasonable improvement remaining. Inspect independent code and visual
reviews, fix accepted findings, run `cargo xtask verify` and native fake-AI validation.
Record measured effects, deliberate remaining approximations and honest platform
coverage here; update state, final screenshots and implementation notes, then commit
and push. Green tests alone do not close visual acceptance.

## Iteration record

Initial inspection confirmed the water uses one heavily
warped threshold field, ambient fill weakens scenery contact, settings transmission
still reveals toy silhouettes, plants are broad extruded blades, and the title menu
intersects current toy placement. No save or simulation changes are required.

### First integrated comparison

`/tmp/beastie-refinement-pass1/captures` showed the marbled water resolved into
small reflected patches and quieter UI rims. Independent review identified material
follow-ups: rock shelves still read as columns, title bell contact floated, title
creature scale crowded the cave, sand caustics lacked quiet regions, shafts were
too faint, and the selected-object card was too large. These findings are accepted
and included in the implementation scope. Stronger localized shafts, varied caustic
coverage, substrate-aware title staging and further geometry refinement follow.

The first feel-harness registration exposed missing scenario markers. That recording
was rejected, markers were added and reduced motion is explicitly disabled again
before title playback. The baseline was recaptured with the amended same-seed
scenario using the original copied binary, producing a validated evidence bundle
at `/tmp/beastie-gold-refinement-before/validated2`.

### Second integrated comparison

`/tmp/beastie-refinement-pass2/captures` confirms broken surface reflections,
stronger upper-water shafts, rounded rock courses and correctly grounded title
toys. Accepted additional corrections: the cave crown remains too pointed and flat
relative to the rounded reference arch; the lower apron consumes excess space;
selected brackets drift down because their foreground offset ignores camera pitch;
tiny labels beneath familiar bottom icons add avoidable density. These are being
corrected without changing simulation locations or action semantics.

Native capture from the standard debug path repeatedly failed before frame zero.
No failed run was treated as visual evidence. A copied debug executable with the
existing `--game` harness option and a longer bounded startup deadline produced
validated motion evidence at `/tmp/beastie-refinement-record/pass1`. Yabai was not
running; no desktop service was restarted.

### Integrated review and final findings

Pass 3 raced the final UI source save and was retained only as an intermediate
geometry capture. Pass 4 captured the completed changes and verified the corrected
silhouette brackets and icon-only utility controls. Independent visual review found
three remaining reasonable fixes: faint but recognizable toys and sand horizon in
the settings plate, dark bubble glints, and the context card covering the bell.
All three are accepted before final verification.

The full gate exposed an existing test-only readiness race: the worker-tree fixture
created its PID marker before writing the PID, and the test sometimes read an empty
file. The fixture now writes a sibling temporary file and renames it into place;
production worker behavior is unchanged.

The selected card's collision-aware placement is chosen once when opened. This
keeps moving aquarium actors from moving action buttons under the player's pointer.
Settings transmission is reduced in linear light rather than assuming an sRGB
opacity percentage maps directly; bubble glints remain localized to bubble geometry.

## Visual candidate review before performance correction

The visual candidate was captured before the performance correction. Two validated
animated runs after the last material change are retained at
`/tmp/beastie-refinement-final1/feel/gold-reference` and
`/tmp/beastie-refinement-final2/feel/gold-reference`, using the same copied debug
binary, seed 42 and eight-second scenario. Independent visual reviewers found no
further material, reasonably contained visual correction after these fixes.
Surface/floor samples show gradual drift, localized glints and sparse bubbles.
The evidence combines native playback, stills, sampled filmstrips and traces; it
does not substitute for human full-speed aesthetic or physical speaker judgment.

All seven matched before/after markers at 0/2/3/4/5/7/8 seconds have identical
authoritative creature snapshots. All 480 event records and 504 audio records match
when playback/frame metadata is excluded. Four transition motion alerts occur in
both versions at reduced-motion/title transitions and are not new regressions.

Native macOS pointer checks verified title Settings, Large text, Escape back to
title, Continue restoring gameplay size/positions, direct ball mesh picking,
correct brackets, clear stable card and Play producing `swimming to a toy`.
This used a disposable scripted session in a uniquely identified debug wrapper.
`cargo xtask verify` passed 546 tests, 27 dialogue fixtures, 11 STT fixtures and
spoken-input replay. Shader translation passed Metal/Vulkan/DirectX validation.

## Remaining practical differences

The gold images still have richer glass reflection/refraction, softer multi-bounce
lighting and finer natural surface variation. The current implementation uses
procedural water and shafts, reused-bounce contact darkening, authored glass-edge
glints and geometric bubble rings. Closing those remaining transport differences
would require a broader glass/lighting pipeline, denoising or significantly more
rays, with unmeasured cost on the other supported platforms. Those are justified
limits of this pass, not a claim of pixel-perfect reproduction. The existing
creature identity, semantic positions, readable Unicode body font and real menu
actions remain deliberate differences from the mockups. No release bundle,
installer, model download or runtime online dependency was introduced.

## Performance correction

A sequential same-scenario recorded timing comparison rejected the visual candidate
for cost: median wall frames rose from 16.66 ms to 25.12 ms, p95 from 33.44 ms to 45.80 ms,
and geometry arenas from 214,359,040 to 298,580,480 bytes. These include recording
overhead, but the increase is material. The candidate was not shipped.

Contact darkening now reuses the existing diffuse bounce instead of adding an
occlusion traversal. Distant decor retains its exact voxel occupancy, folds and
colors while removing subpixel bevel tessellation; nearby foreground rocks and
semantic objects retain bevel treatment. A headless aggregate garden triangle
budget guards this regression. New native comparison and timing follow this change.

The geometry correction reduced peak arenas to 151,779,840 bytes, below the original
214,359,040 bytes. A separate five-second native CPU sample identified the remaining
selected-toy scan: `selection_bounds` occupied 229 of 2,592 sampled intervals. Exact
pose results are therefore cached rather than rescanning unchanged meshes each frame.
The wider shadow disk was measured separately and retained: its small timing change
did not justify sacrificing softer contact. Shaft modulation is shared outside the
four-beam loop and uses a bounded sine field instead of repeated hashed noise.

### Final measured acceptance

After exact pose caching, the final recorded median is **16.74 ms**, p95 **33.58 ms**,
versus **16.66 ms / 33.44 ms** before refinement. Peak geometry falls from
214,359,040 to **151,779,840 bytes** (29.2% lower). The same seed, 1280×720 debug
rendering and eight-second recorded scenario were used sequentially on Apple M5 Max /
Metal. Reports are retained in `implemented/render-report-before.json` and
`implemented/render-report.json`. These wall intervals include GPU backpressure,
pacing, video encoding and named PNG capture; p99 capture spikes are not steady-state
GPU timings. The final five representative motion checkpoints are pixel-identical
to the uncached visual candidate after geometry and shader optimization.

A second sequential comparison removes all recording and named captures at the
current 1920×1080 window preference. Across 469 measured frames, median/p95 are
29.06/52.40 ms before and 29.61/54.60 ms after (`uncaptured-before.json` and
`uncaptured-final.json`). These unoptimized debug results remain near the original
cadence and include settings/title geometry transitions; they do not establish
release performance or a lower-end GPU budget. No additional render-quality controls
are needed to keep the refinement within the existing default cost.

## Completion evidence

The final source passed `cargo xtask verify`: **548 tests**, 27 dialogue fixtures,
11 STT fixtures and spoken-input replay. One full-gate attempt hit the existing
`warm_server_reuses_one_pid_and_receives_flags_auth_and_bounded_payload` assertion
(two fixture starts instead of one). Its focused suite and the complete gate rerun
passed without changing the worker. This transient failure is retained here rather
than attributed to the visual changes.

`cargo xtask dev --fake-ai --script fixtures/scenarios/gold-reference.jsonl`
completed on macOS and produced the refreshed 1920×1080 images in `implemented/`.
Two final validated native motion runs after all performance changes are retained at
`/tmp/beastie-refinement-complete/pass1/gold-reference` and
`/tmp/beastie-refinement-complete/pass2/gold-reference`. Their binary SHA-256 is
`4c1a8a8f24eb1fb949197abd2fa6ded4fb2e262bdc42f392e9ab96eae1520d69`.
Final still and filmstrip review retains the previous native interaction findings;
cache tests and pixel-identical checkpoints confirm the optimization changes cost,
not visible placement. The originals remain unchanged, and gameplay semantics remain
authoritative. No known material correction with a reasonable contained cost remains
in this pass; the transport and platform limitations above remain explicit.

Independent final review compared both optimized runs against the validated baseline:
all seven markers, all 480 event records, all 504 audio records and authoritative
creature state at each marker match. Stills and filmstrips retain accepted composition,
lighting, silhouettes and readable overlays. The final cache/code review found no
outstanding correctness issue; the performance correction is accepted.
