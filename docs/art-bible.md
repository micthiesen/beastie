# Beastie art bible

## V1 direction

Beastie is an aquarium creature, not a room prop. The frame is open water with a deep, cozy-grotty
palette: dirty teal and blue water, plum-black depth, worn sand, algae, plants, a cave, bubbles,
and a few warm or cyan accents. The aquarium should feel inhabited and slightly strange rather than
like a clean fish tank. The lower boundary gives the eye a place to rest, but the creature owns most
of the swimming volume.

The hero creature is one recognizable 64 to 80 logical pixels tall rig. Its body silhouette carries
swim, hover, turn, eat, sleep, recoil, and play. Layered face and effect planes carry gaze, blink,
suspicion, anger, delight, loneliness, bubbles, blush, stress marks, spit, crumbs, and ink-like
clouds. Gaze leads body turns. A bespoke hero reaction is preferable to a new generic pose when a
moment matters. Horizontal flips are allowed only for poses whose lighting and asymmetry remain
credible.

## Pixel contract

- The logical framebuffer remains 320x180. Draw sprites on whole logical pixels.
- Native aquarium body art targets 64 to 80 logical pixels in height. Face overlays must remain
  readable at that size without smoothing.
- Use nearest-neighbor sampling, integer viewport scales, and intentional letterboxing. Never bake
  a bilinear resize into a runtime PNG.
- Runtime pixel art has hard alpha edges: every alpha is exactly 0 or 255. Opaque backgrounds are
  fully opaque. Transparent sprites must contain transparent pixels.
- Every opaque RGB pixel must be in the declared palette or within its explicitly declared
  `palette_tolerance`. Nearby provider colors are a documented exception, not a reason to loosen
  the V1 defaults.
- A candidate must stay under its declared `max_colors`. Isolated interpolation colors, excessive
  unique colors, partial-alpha fringes, and inconsistent animation frames fail the asset gate.
- `native_pixel_density`, when known, is positive and must be the same policy for every frame in an
  animation. Frames keep identical dimensions and the same declared palette policy.

The canonical V1 policy for new aquarium art is:

```toml
max_colors = 32
palette_tolerance = 0
alpha_policy = "hard"
native_pixel_density = 1
```

Provider-generated MVP art can use a measured, asset-specific tolerance and color ceiling when its
manifest records that exception. Do not copy that exception into new aquarium entries.
The selected V1 aquarium background has one measured exception: 34 exact declared colors instead
of 32. Its props use 10 to 15 colors and the creature base uses exactly 32. The generated motion
frames use 34 to 40 colors per frame to preserve expression and consistent identity. Every aquarium
runtime asset still uses an exact declared palette with zero tolerance, hard alpha, and native
density 1; these measured ceilings are not permission for later assets to grow their palettes.
Large `status = "reference"` concept images are provenance inputs rather than runtime candidates;
they still need valid dimensions and transparency metadata but are not subject to the runtime color
ceiling.

The asset checker returns color and alpha diagnostics for each decoded candidate, including opaque
pixel count, transparent and partial-alpha counts, unique colors, maximum palette distance, and
isolated interpolation pixels. These numbers are the first evidence to inspect when a capture looks
soft.

## Manifest and promotion

Every shippable PNG has an entry in `assets/manifest.toml`. An ID such as `creature/idle` resolves
to `assets/final/creature/idle.png` when present, otherwise to `assets/generated/creature/idle.png`.
Final art therefore replaces generated art without changing runtime IDs. Multi-frame assets use
zero-based files such as `creature/swim-0.png` and `creature/swim-1.png`; `frames` declares the
required count. Each frame is validated, including a generated frame hidden by a final promotion.

Use `status = "runtime"` only after every required frame exists and has been deliberately selected.
Use `status = "generated"` while reviewing generated output, `status = "planned"` before
generation, and `status = "reference"` for direct style images that are not runtime-loaded. The
final verification gate rejects planned or generated runtime entries.

Each non-planned entry records the generator, date, prompt summary, seed (or an honest
`provider-managed` marker), terms snapshot, reference images, and human modifications. Keep
provider job IDs whenever the service exposes them. A final file never excuses a corrupt generated
source left in the asset history.

Run the contract after any art change:

```bash
cargo xtask asset check
cargo xtask verify
```

The check validates manifest structure, unique safe IDs, referenced palettes and style files,
available generated and final candidates, exact dimensions, hard alpha, palette distance, color
counts, interpolation artifacts, frame completeness, provenance, final-over-generated resolution,
and WAV format. `cargo xtask verify` runs the same strict runtime gate without requiring a display,
model, GPU, audio device, network connection, or PixelLab credential.

## MVP provenance

The MVP established the original cozy-grotty language: warm lamplight, deep plum shadows, scuffed
furniture, dirty neutrals, and sharp cyan electronic accents. Its fixed, mostly straight-on
dollhouse view and the window as the main source of environmental variation remain useful history,
not the V1 composition. The approved MVP references are `assets/style/room-concept.png` and
`assets/style/furniture-anchor.png`; they establish material wear, lighting, and scale and are not
loaded by the game. The selected unanimated creature pose remains a provenance reference while
runtime animation and the V1 layered rig replace it.
