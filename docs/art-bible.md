# Beastie art bible

## Direction

Beastie is **cozy-grotty** pixel art: warm lamplight, deep plum shadows, scuffed furniture,
dirty neutrals, and a few sharp cyan electronic accents. The room should feel cared for but never
polished. The creature is appealing because it is awkward and expressive, not because it looks like
a mascot. Sulky, ugly, or faintly unsettling poses are welcome when they communicate state clearly.

The view is a fixed, mostly straight-on dollhouse composition with a slight three-quarter floor. The
window supplies most environmental variation. Day, night, weather, birds, and odd silhouettes may
change the mood, but not the composition.

## Pixel contract

- Logical framebuffer: 320x180. The room background is exactly this size and fully opaque.
- Creature sprites: 48x48 with genuine transparent pixels and a stable silhouette across poses.
- Working palette: `assets/style/main.hex`, one RGB hex colour per line. New generated assets must
  reference it even when anti-aliasing or provider output introduces nearby colours.
- Animation is restrained. Prefer a held stare, a foot shift, or a small shove over constant motion.
- Keep face, posture, and interactive props readable at native resolution. Do not rely on smoothing.

The approved reference images are `assets/style/room-concept.png` and
`assets/style/furniture-anchor.png`. They establish composition, material wear, lighting, and scale;
they are not loaded by the game.

## Manifest and promotion

Every shippable PNG has an entry in `assets/manifest.toml`. An ID such as `creature/idle` resolves
to `assets/final/creature/idle.png` when present, otherwise to
`assets/generated/creature/idle.png`. Final art therefore replaces generated art without changing
runtime IDs. The selected unanimated creature pose remains a provenance reference while the runtime
idle ID uses its animation frames. Multi-frame assets use zero-based files such as `creature/walk-0.png` and
`creature/walk-1.png`; `frames` declares the required count.

Use `status = "runtime"` only after every required frame exists and has been deliberately selected.
Use `status = "generated"` while reviewing generated output, `status = "planned"` before generation,
and `status = "reference"` for direct style images that are not runtime-loaded. The final verification
gate rejects planned or generated runtime entries.

Each non-planned entry records the generator, date, prompt summary, seed (or an honest
`provider-managed` marker), terms snapshot, reference images, and human modifications. Keep provider
job IDs whenever the service exposes them.

Run the contract after any art change:

```bash
cargo xtask asset check
```

The check validates manifest structure, unique safe IDs, referenced palettes and style files, every
available generated and final candidate, exact dimensions, transparency, provenance, frame
completeness, and final-over-generated runtime resolution. `cargo xtask verify` runs the same strict
runtime gate.
