# Presentation polish plan

Status: implemented and verified on macOS on 2026-08-15.

This pass preserves the 320x180 aquarium composition and existing creature art while moving the
final presentation surface to 640x360. The renderer projects world pixels at an exact 2x scale,
then draws typography, interface chrome, interaction feedback, and captions at presentation
resolution. The final surface is nearest-neighbor scaled to fixed 16:9 window sizes.

## Chosen behavior

- Default window size is 1280x720. Supported windowed sizes are integer multiples of 640x360.
- Freeform resizing is disabled. Fullscreen uses the largest fitting integer viewport with
  letterboxing and never stretches or crops the aquarium.
- Runtime text uses Atkinson Hyperlegible Next Medium at a 16-pixel presentation baseline. The
  bundled bitmap face remains a missing-asset fallback.
- World sprites and simulation coordinates stay unchanged. Presentation may extrapolate visible
  motion between fixed simulation ticks without mutating authoritative state.
- Pointer hover and controller focus follow the opaque silhouette of interactive sprites. Missing
  or corrupt sprite data falls back to the semantic rectangular target.
- Body animation starts from frame zero when an action begins. Continuous movement, bubbles, and
  restrained bobbing provide smoothness between authored sprite frames.
- Generic optical-flow interpolation is not used. Additional authored frames are accepted only
  when identity, palette, silhouette, and anchor remain coherent.

## Asset corrections

- Food and settings controls use simple 20x20 hard-alpha PixelLab sprites with bold silhouettes.
- Wake and sand effects must remain sparse transparent marks with no white foam, scum, backdrop,
  or hidden white RGB.
- Hard-alpha runtime PNGs use canonical black RGB wherever alpha is zero, preventing white fringes
  if a future compositor samples beyond opaque edges.
- Animation review includes native and presentation-scale playback in addition to dimensions,
  palette, alpha, and transparent-RGB validation.

## Acceptance

- Text and controls are readable in a 640x360 capture without zooming.
- The 640x360, 1280x720, and fullscreen paths preserve the same composition and semantic targets.
- Hovering the creature, food, toys, cave, and plants produces a clean silhouette treatment rather
  than a bounding rectangle.
- Creature and falling-food movement updates smoothly between deterministic simulation ticks.
- No captured frame contains white boxes, foam-like wake artifacts, garbled action icons, clipped
  text, fractional viewport blur, or resize feedback loops.
- `cargo xtask verify` remains display, model, audio-device, network, and credential independent.
