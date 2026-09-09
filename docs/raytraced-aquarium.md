# Unified ray-traced aquarium

## Production contract

The player sees one crafted aquarium and care deck, with typography belonging to its materials.
All visible marks, including text, are geometric intersections in a custom ordinary-GPU compute
ray tracer. Bevy retains windowing, input, transforms, assets and capture; its old scene raster
passes are replaced, not a player-selectable fallback. A final fullscreen transfer presents the
computed image and is not a sprite or alternate scene rendering path.

## Implementation

Use cached local triangle acceleration structures and an instance hierarchy. Preserve the authored
bevel geometry and normals, continuous transforms, exact camera projection and interaction plane.
Geometry is shared with the current declarative scene, so no simulation or save migration is needed.
Glyphs are shaped from the bundled font and tessellated into clipped geometry with authored spacing,
material and weight; no font atlas or separate raster text overlay remains. Unsupported glyphs
must remain visible, and grapheme-aware text editing, wrapping and all content bounds must survive.

Use area-light visibility, authored environmental illumination, and useful bounded indirect light.
Lighting should make the animal more present without hiding its eyes or mouth. Prioritize stable
moving detail and immediate text response over temporal accumulation that smears expressions.
Measure CPU extraction, GPU render cost where available, full-frame cadence and memory. Keep
ordinary compute compatible with Metal, Vulkan and D3D12; do not require RT extensions or DLSS.

## Review checkpoints and acceptance

1. Complete geometry and glyph migration, traversal correctness and native framebuffer capture.
2. Tune native materials, typography, anti-aliasing, light, motion stability and frame time.
3. Independent code reviews of traversal, GPU/resource lifecycle and text/layout ownership.
4. Repeated native visual/temporal reviews covering quiet life, interactions, dialogue races,
   failures, all Normal/Large UI, reduced motion and actual pointer/keyboard text/drop controls.
5. Fresh final recaptures after accepted fixes; cargo xtask verify, docs, commit and push.

Existing native surface recordings are the comparison baseline. Capture differences in rendering
are intentional; seed, scenario and gameplay ownership remain unchanged. Dense frames and traces
support continuity and causality; they do not establish human full-speed perception or physical
speaker/controller feel. Record actual platform coverage without substituting headless checks for
Windows/Linux native execution. Every accepted material finding must be fixed before completion.

## Delivered implementation

The production path uses cached, incrementally uploaded mesh BLAS arenas and a per-frame instance
TLAS, with near-first traversal on ordinary compute. Six fixed area-light visibility samples
(refined to twelve for mixed visibility on upward rough surfaces),
a bounded diffuse bounce and selected glossy reflections create dimensional light without temporal
history. Two world and four UI coverage samples preserve stable silhouettes and geometric text.
Skin uses authored soft fill, and UI inlays use controlled studio illumination within the same ray
shader. No physical glass/refraction or true subsurface transport is claimed. The gold-reference pass
adds procedural surface reflection pools, warped caustics, wavelength-dependent depth tint,
analytic shafts and sand grain without extra world shadow or bounce rays. A faint transmitting
settings plate uses at most one cached world-only continuation per pixel, with inexpensive
lighting; controls and glyphs remain opaque. The frame has a separate metallic material.
Bright-pass bloom was tried and removed because it echoed fine lettering. See the
[gold comparison](style-reference/implementation.md).

The bundled Atkinson font is shaped with rustybuzz and its outlines tessellated with lyon, including
holes, kerning, ligatures, wrapping and clipping. Glyphs are planar inlaid geometry. Text and panels
share primary ray intersections; UI geometry is excluded from secondary world illumination.
Bevy retains CPU meshes/material descriptions, windowing and input, but no PBR scene render,
sprite, text atlas or separate UI raster path is active. There is no alternate renderer toggle.

Implementation review, native comparisons, timing methodology and coverage limits are recorded in
[feel-review-raytracing.md](feel-review-raytracing.md). The prior surface contract remains useful
for authored mesh construction, while this document owns final lighting and composition.
