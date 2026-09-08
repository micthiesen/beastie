# Beastie art bible

## Voxel aquarium

Beastie is a small three-dimensional aquarium viewed through a fixed orthographic camera. The
composition leaves open swimming water around one recognizable creature. Dirty teal and blue
water, plum-black depth, worn sand, algae, plants, a cave, and restrained warm accents make the
tank feel inhabited and slightly strange. The lower boundary gives the eye somewhere to rest;
the creature remains the visual center.

All world forms are solid geometry built from colored voxel shapes. The creature, scenery,
food, toys, bubbles, and reaction effects use meshes. There are no sprites, sprite sheets,
painted face textures, image planes, or raster background art. Small facial features may use
finer geometry than the body. Text is the exception to the geometric presentation contract.
The renderer uses Bevy; the simulation continues to own every authoritative fact.

Voxel shapes move and rotate continuously through space. They do not snap to a world voxel
grid. Rendering at the window resolution lets geometry provide the chunky appearance without
forcing the face and text through the former low-resolution pixel canvas. Lighting should
reveal the creature's form while preserving readable eyes and mouth against the water.

## Creature identity and acting

The hero is one coherent animal with an expressive head, articulated body, and trailing tail.
Head silhouette, eye spacing, warm body colors, fin placement, and body proportions establish
identity across all moods. The face should read at ordinary gameplay size. Extra tiny cubes
that disappear at that size do not count as additional expression.

Gaze, pupils, eyelids, brows, mouth, head tilt, fins, tail, and posture work together. Content,
curious, hungry, sleepy, lonely, and resentful states should have distinct readable expression
without replacing the creature's identity. Speech opens and closes geometric mouth features;
it does not swap an image onto the head. Looking toward the player may lead the body's turn.

Body segments follow the creature's motion smoothly with consistent spacing and restrained
lag. Turning, stopping, settling, and idle pauses matter as much as swimming. Tail and fin
motion should support the action rather than continue at one busy amplitude in every state.
Reduced-motion settings should preserve semantic expression while reducing decorative motion.

Animation projects authoritative state and typed events. Accepted eating, food rejection,
toy refusal, play, comfort, affection, and sleep need distinguishable acting. Presentation
must not infer an outcome from generated prose or a generic mood. Direct interaction feedback
must remain immediate and interrupt incompatible decorative reactions. Keep the full creature
inside the visible tank at simulation boundaries, including its trailing body and effects.

## Interface and text

The interface uses dark teal surfaces, warm restrained highlights, and clear keyboard,
pointer, and controller focus. Controls and panels use geometry and text, never image icons.
Dialogue must remain readable without covering the face. Settings, food and toy choices,
bindings, data management, and confirmation surfaces share the same material and spacing rules.
Readability takes priority over making letters look like voxel blocks.

The bundled text face is Atkinson Hyperlegible Next Medium at
`assets/generated/ui/atkinson-hyperlegible-next-medium.ttf`. Its open counters and distinct
letter shapes support mixed-case dialogue and controls. The font is distributed under SIL
Open Font License 1.1, preserved in `assets/licenses/atkinson-hyperlegible-next-OFL.txt` and
`THIRD_PARTY_NOTICES`. It came from the upstream `googlefonts/atkinson-hyperlegible-next`
repository. The font SHA-256 is
`dd50b08b3c560846097d23baaaf6a97ffa20dd077115d23c59df68083b9ea05e`; the license SHA-256 is
`aca6a428580965d2297d1b718042dd427c2a9443ece3b0d02d758e161e0c4030`.

## File-backed asset contract

`assets/manifest.toml` version 2 lists sounds and text fonts. World art is procedural geometry
and does not appear in this manifest. The former sprite catalog, palettes, image files,
PixelLab integration, frame curation, and image promotion commands have been removed.
Historical reviews describe past builds and are not instructions to restore their assets.

Sound IDs retain their stable paths. For example, `audio/ui/select` resolves to
`assets/final/audio/ui/select.wav` when present, otherwise to
`assets/generated/audio/ui/select.wav`. The gate validates both existing candidates, including
a generated sound shadowed by a final file. Each sound records its PCM format, runtime status,
and a repository provenance document. Every shippable sound must have status `runtime`.
Fonts declare their file and license paths.

Run the contract after changes:

```sh
cargo xtask asset check
cargo xtask verify
```

The asset check validates safe unique sound IDs, provenance, WAV encoding and format, font
table bounds, and nonempty font licenses. It rejects image files anywhere under `assets/`,
even when absent from the manifest. It requires no display, model, GPU, audio device, or network.
Steam store imagery and operating-system app icons live separately under `steam/assets/`;
they are distribution artwork and never world-rendering inputs.

## Visual acceptance

Exercise the native game after visible changes with `cargo xtask dev --fake-ai`. Inspect the
actual gameplay window, not only isolated geometry or enlarged screenshots. Review ordinary
idle life, smooth turns and stops, the face during speech, direct food and toy outcomes,
relationship expression, overlays, and edge positions. A frame can look attractive while
motion or interruption feels wrong, so inspect both still captures and sustained play.

Keep simulation truth, offline operation, and degraded AI/audio operation intact. Lighting,
face detail, body motion, and composition are successful when they make this particular
creature easier to understand and care about.
