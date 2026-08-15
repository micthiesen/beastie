# Beastie art bible

## V1 direction

Beastie is an aquarium creature, not a room prop. The frame is open water with a deep, cozy-grotty
palette: dirty teal and blue water, plum-black depth, worn sand, algae, plants, a cave, bubbles,
and a few warm or cyan accents. The aquarium should feel inhabited and slightly strange rather than
like a clean fish tank. The lower boundary gives the eye a place to rest, but the creature owns most
of the swimming volume.

The hero creature is one recognizable 80x80 source rig rendered at an exact 2x logical scale. Its
whole sprite carries mood as well as motion: content, curious, hungry, sleepy, lonely, and resentful
each have side-facing and player-facing art, while swim, accepted eating, food rejection, sleep,
play, noticing, toy refusal, comfort, affection, and speech use their own curated full-body frames.
The face must never look pasted onto a neutral body. Eyes, mouth, fins, tail, posture, outline,
palette, and shading act together in every normal runtime pose.
Small bubbles, hearts, attention marks, crumbs, sand, wake, and sleep marks are authored sprites,
not rectangle overlays. Gaze leads body turns; attention toward the player uses the south-facing
state. A bespoke hero reaction is preferable to a generic pose when a moment matters. Horizontal
flips are allowed only for side poses whose lighting and asymmetry remain credible.

Animation projects authoritative semantic state. A food-consumed event selects accepted-eating
acting, a food rejection selects disgust, a toy rejection selects toy refusal, and accepted comfort
selects its own trusting transition. Presentation never guesses those outcomes from dialogue or
from a generic mood. Low-priority punctuation such as crumbs may yield to a new conversation;
direct refusal and comfort remain immediate. Every short reaction starts on its first authored
frame, holds its last frame, and is kept inside the visible aquarium even when world movement has
reached a permissive edge.

Generated art is curated as if it were handcrafted. A provider result is source material, not an
automatic acceptance. Inspect every frame at native size and at the actual 2x aquarium scale;
reject identity drift, accidental props, expression changes, invented colors, edge clipping, and
loops whose silhouette jumps. It is valid to use a clean static state with deterministic buoyancy
when animation generation damages a fragile expression. Curated frame repetition is preferable to
a more varied but incoherent loop. Repeated prompt violations are a reason to retain the stronger
existing animation, not to promote the least-bad reroll. The renderer's simple code-native
creature remains only a missing-asset safety net and is never layered over shipped hero art.

## Pixel contract

- The logical framebuffer remains 320x180. Draw sprites on whole logical pixels.
- Native hero state and speech art uses an 80x80 transparent canvas. Provider padding is cropped
  symmetrically only after proving every opaque pixel remains inside that canonical canvas.
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
of 32. Its props use 10 to 15 colors and the creature base uses exactly 32. Generated motion and
expression frames use measured per-asset ceilings to preserve cohesive shading and identity. Every
aquarium runtime asset still uses an exact declared palette with zero tolerance, hard alpha, and
native density 1; these measured ceilings are not permission for later assets to grow their
palettes.
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
