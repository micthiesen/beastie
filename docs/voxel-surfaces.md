# Voxel surface and light contract

The surface geometry decisions remain current. The subsequent unified renderer migration in
[raytraced-aquarium.md](raytraced-aquarium.md) supersedes the PBR lighting and raster text path
recorded below; this document preserves the comparison history.

## Outcome

Make the aquarium feel like a tactile miniature world: continuous voxel forms, small exposed-edge
bevels, legible material differences and soft dimensional light. Preserve the stepped silhouette
and expressive face. All non-text visuals remain solid 3D geometry. Bevy remains the platform
foundation; custom shading is justified by visible results rather than novelty.

## Complete scope

1. Diagnose grid lines with identical unlit, shadow-free and single-color captures. Fix geometric
   discontinuities, accidental gaps and overlapping faces. Shared voxel boundaries use consistent
   lattice arithmetic.
2. Compare sharp continuous surfaces, connected geometric bevels and deliberately separated blocks
   on the same seed/scenario. Prefer bevels only at shape edges, not every coplanar color boundary.
   Keep geometric topology closed on joined and concave forms.
3. Author material roles for satin creature skin, readable eyes, rough stone/substrate, plants,
   rubber/cloth toys and brass. Preserve the current expression timing and ownership.
4. Add softer directional shading, warm key/cool fill and restrained environmental/contact light.
   Contact shading must reveal concavity without painting a black grid. No required ray-tracing
   hardware, sprite shadows or external runtime assets.
5. Keep intentional seams at selected construction boundaries. The creature remains continuous;
   the floor must not look like a fine accidental checkerboard.

## Evidence and sequencing

Baseline is `371d3af`, with native same-seed comparisons from `voxel-surfaces.jsonl`. Root owns
native windows and integrated rendering; parallel workers own surface geometry and substrate.
First review validates topology and materials, second reviews integrated code and native appearance.
Run representative interaction, quiet-life, fallback, reduced-motion and Normal/Large UI evidence.
After the final material change, complete two fresh native review passes with independent visual
and temporal review. Actual pointer input verifies the retained projection. Full-speed human
perception and physical audio/controller/other-platform evidence must remain honestly distinguished
from sampled-frame and trace inspection.

## Acceptance

- No unintended visible hairline grid, cracks or coplanar flicker in exercised scenes.
- Bevels catch light while flat faces remain continuous across cells and material-color boundaries.
- The face stays readable in affection, refusal, speech, sleep and moving profiles.
- Materials and lighting reveal depth without losing color or creating harsh sparkle/shadow noise.
- Motion, scene/UI picking, saves and authoritative behavior stay intact; no sprites are introduced.
- Multiple code and feel reviews leave no accepted material issue unresolved. The headless gate
  and native fake-AI path pass, durable evidence is recorded, and finished work is committed/pushed.

## Selected implementation

The selected treatment is continuous bevel geometry on rounded animal parts, toys and larger
stone forms. A shared integer boundary lattice is converted to floating point once per position.
Narrow subdivided crease bands relax simultaneously into actual connected chamfers; coplanar
boundaries and color changes remain joined. Tests include convex/concave topology, winding,
finite normals and long straight terrace creases. Ordinary solid occupancy is assumed; edge-only
or point-only contact is inherently nonmanifold and is not used by the authored models.

Animal lighting blends 75% ellipsoid-volume normal with 25% geometric normal, retaining voxel
silhouettes and highlights without a fine corrugated grid. The fine substrate deliberately uses
sharp continuous geometry because its small contour bevels competed with the animal. Gentle
rear-corner banks leave the central bed flat. Cave tones vary in broad patches. Construction detail
remains at sock stitching, toy bands and tank framing; there are no global per-voxel gaps. UI icons
also use joined lattice surfaces rather than 95%-sized separated cubes.

`appearance.rs` owns satin skin, glossy eyes, rough stone, plant, rubber, cloth, brass and food
material roles. Bevy's ordinary PBR path uses four-sample MSAA, a bounded single directional-shadow
cascade, Gaussian filtering, explicit bias and a lower-resolution shadow map for soft edges at this
fixed scale. Warm key, cool fill and a procedural six-texel hemispherical environment map provide
directional ambient light and reflections. This is an authored bounce approximation, not traced
global illumination. The map is lighting data, never a sprite or world image plane.

The noisy screen-space contact-shadow experiment was rejected. No experimental PCSS, TAA,
hardware ray tracing, custom full-screen pipeline or new GPU feature dependency is required.
The native comparisons supported meshing, normals and material/light changes within Bevy instead.
UI, simulation, animation ownership, world projection and save formats are unchanged.

The built game exposes script-only `--surface-treatment sharp|beveled|separated` and
`--lighting-study finished|unlit|no-shadows|clay` for repeatable comparisons. These never become
player preferences. Clay neutralizes world colors and PBR roughness/metalness; UI and the deliberate
unlit face marks/background retain their separate rendering roles. The substrate's sharp production
choice applies in both continuous treatments; separated mode remains an intentionally contrasting
construction-block diagnostic.

The implementation and repeated review pass completed on 2026-09-08. Native comparison,
interaction, quiet-life, failure and UI evidence is recorded in
[feel-review-voxel-surfaces.md](feel-review-voxel-surfaces.md).
