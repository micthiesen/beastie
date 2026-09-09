# Gold-reference implementation

Implemented 2026-09-09, then refined through repeated native comparison. The
[refinement contract and review](refinement.md) records subsequent findings and decisions.
The original JPGs remain unchanged. The PNGs below are actual compute-rendered game
frames, not mockups or image-generated replacements.

| State | Gold | Implemented |
| --- | --- | --- |
| Gameplay / selected toy | [Reference](gameplay.jpg) | [Gameplay](implemented/gameplay.png), [selected toy](implemented/selected-toy.png) |
| Settings | [Reference](settings.jpg) | [Settings](implemented/settings.png), [Large text](implemented/settings-large.png) |
| Title | [Reference](title-screen.jpg) | [Title](implemented/title-screen.png) |

The original [gameplay](baseline/gameplay.png) and [settings](baseline/settings.png)
captures preserve the before state. No title baseline exists because the game had no
title screen. These still comparisons use seed 42, unchanged initial world state and
1920×1080 native rendering. UI transitions do not advance scripted simulation time.

## Changes

The tank now has deep teal water, warm sand and terracotta, tall asymmetric plants,
layered edge rocks, sparse voxel bubble rings, narrow metallic framing and visible
warm lamps. Removing the old solid roof lets the existing key light reach the tank.
The care rail puts identity and real state left, actual toy miniatures beside Feed,
and chat, Send, microphone and Settings right. Selected toys have pale world brackets
and a nearby card retaining Play and Inspect.

Settings uses a right-side plate, vertical Display/Sound/Controls categories, block
switches and compact inset values. All existing options remain available. Title adds
an extruded voxel Beastie logo and real Continue, Settings and Quit actions. Its taller
framing and portrait placement affect presentation only; Continue restores gameplay
projection and object picking. Settings closes back to its originating title screen.
The subtitle is centered beneath the logo using measured glyph advances at Normal
and Large text sizes, retaining its existing color and vertical placement.

Refinement adds curved narrow fronds, eroded rock courses, a rounded terracotta
arch, brighter bubble glints and thinner plate rims. Title toys meet the sand using
actual transformed mesh bounds; the smaller creature sits clear of the cave.
Gameplay framing places the sand closer to the compact rail, with unchanged pointer
coordinates. Utility icons retain action labels in the semantic hit plan.

The shader adds animated surface reflection pools, warped caustics, sand grain,
wavelength-dependent depth tint and sparse analytic shafts. Water effects add no
shadow or bounce rays. Contact darkening reuses the existing diffuse bounce,
without an additional occlusion traversal. The settings plate uses at most one cached world-only
continuation per pixel; its very small linear-light contribution keeps bright sand
from overwhelming the dark panel. Ordinary text and controls remain opaque.

## Deliberate differences and remaining gaps

- The original title reference reads “Mop”; the game logo reads “Beastie”. Mop remains
  the default creature name. The original reference is preserved unchanged.
- The existing articulated creature and authoritative toy/cave positions are preserved.
  There are no decorative fake fish, New Chat action, invented settings or empty slots.
- Body text keeps the existing shaped, ray-traced Atkinson outlines for Unicode and
  readability. Only the logo uses authored extruded voxel glyphs.
- Volume, surface reflection and caustics are procedural approximations. Physical
  glass refraction, a full scattering volume and DOF are absent. The water and scenery
  remain simpler and less naturally varied than the gold renders.
- Bloom was implemented experimentally and removed after it echoed fine text.
  Bright surface pools and local warm lighting provide the highlights instead.
- The reference has richer glass reflections, softer light transport and more detailed
  plant/rock silhouettes. Those remain the largest visual differences. Existing
  material reflections and contact shading are retained; this is not a claim of
  pixel-perfect reproduction or full path-traced transport.

## Verification and capture workflow

`cargo xtask verify` covers formatting, lint, tests, dialogue/STT fixtures and spoken
input replay. Shader validation translates to Metal, Vulkan and DirectX targets.
Focused regressions cover title/modal hit isolation, stable contextual placement,
rotated toy grounding and selection bounds, bounded pose-cache invalidation,
settings text bounds, camera/pointer
alignment across window aspect ratios and the water clock crossing one hour without
resetting.

Native macOS checks exercised toy-slot selection, Play, Settings, Large text,
category navigation, Escape, text entry/submission, title Settings, return to title,
Continue and window close. Independent code review found no outstanding code issues.
An eight-second scripted recording checks moving water, bubbles, selection, settings,
reduced motion, title animation and return to gameplay. The original frames and recording samples retain
Mop's identity label; an apparent clipped-name preview was investigated and rejected.

The [recorded timing report](implemented/render-report.json) is from the final debug
build on Apple M5 Max / Metal at 1280×720, with recording and capture overhead:
486 measured frames, median 16.74 ms, p95 33.58 ms, p99 514.83 ms. The same-scenario
[before report](implemented/render-report-before.json) measured 16.66 ms / 33.44 ms
median/p95. Peak geometry memory fell 29.2%, from 214.36 MB to 151.78 MB. Exact pose
caching and reused-bounce contact shading retain the visual refinements within the
previous timing envelope. These are wall-frame intervals, not GPU pass timings;
capture and UI transitions contribute large tail spikes. An additional 1920×1080
uncaptured debug comparison measured median 29.06 ms before / 29.61 ms after; see
[refinement evidence](refinement.md#final-measured-acceptance). Windows,
Linux, lower-end GPUs, physical microphone and controller hardware remain untested.
No release binary, installer or model bundle was rebuilt.

Capture all representative states without touching the player's save:

```sh
cargo xtask dev --fake-ai \
  --script fixtures/scenarios/gold-reference.jsonl \
  --capture-dir target/captures/gold-reference
```

That command follows the current window-size preference. For validated animated evidence with deterministic default settings, use a new
output directory:

```sh
cargo xtask feel --suite gold-reference --output target/feel/gold-reference
```

To include a wall-frame timing report in a direct capture:

```sh
cargo run -p beastie-game --locked -- --fake-ai \
  --script fixtures/scenarios/gold-reference-motion.jsonl \
  --capture-dir target/captures/gold-motion \
  --feel-dir target/feel/gold-motion --feel-script-only \
  --render-report target/feel/gold-motion/report.json
```

`--stay-open` permits manual input checks after the still scenario. Scripted time
advances through `wait` steps; normal unscripted play advances continuously. On macOS,
if app automation confuses old bundles, a uniquely identified temporary app wrapper
around the debug executable can select the correct process without release packaging.
