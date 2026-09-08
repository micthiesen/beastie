# Bevy voxel migration

## Authorized outcome

Replace the complete ggez presentation with a cross-platform Bevy application. The aquarium,
creature, food, toys, scenery, effects, and icons use geometry. No game sprite assets, sprite
animation, image-plane creatures, or legacy renderer remain. Font rendering is allowed; facial
features may use finer or non-cubic geometry when it improves expression.

The intended experience is looking into a small, living tank: a warm gold creature with readable
eyes and mouth, smooth swimming and turning, an articulated trailing body, quiet purposeful life,
and clear embodied responses to care. The fixed camera and lighting make its volume readable.

## Architecture and preservation

- Keep deterministic core and session truth, saves, memories, development, behavior, action contact,
  private life, relationship causality, and bounded local dialogue/TTS/STT.
- Keep the existing authoritative 2D interaction plane inside a real 3D scene. Decorative depth
  never changes contact truth. Full 3D navigation is not required to replace the renderer.
- `beastie-view` emits a typed, engine-independent scene, expressive pose, geometric UI, and audio
  plan. Bevy executes it. No engine dependency enters core or view.
- Replace the game event loop, rendering, pointer picking and frame capture. Preserve semantic UI
  commands, controller/keyboard paths, microphone lifecycle, safe shutdown and original save paths.
- Generate colored voxel surface meshes and animate parts continuously. A bounded continuous head
  position guides fixed-distance body joints from path tangents; hover/settled steering retains
  curvature instead of feeding joint movement back into the path. Facial controls include gaze,
  lids, brows, mouth and head orientation. Reduced motion remains a complete playable presentation.
- Remove sprite assets, catalog, alpha picking, frame selectors, PixelLab generation/curation code,
  and obsolete sprite checks. Retain sounds, fonts, their licenses and package integrity checks.

## Implementation checkpoints

1. Typed scene/view migration, engine-neutral shell, asset/tooling cleanup and Bevy dependencies.
2. Complete voxel tank, creature face/body rig, objects, semantic effects and all UI states.
3. Native input, frame capture, worker/audio synchronization and integrated headless verification.
4. Independent code reviews, actual native play, synchronized scenario review and improvements.
5. Two valid native review passes after the final material change, final gate, docs and shipping.

Checkpoints are progress markers, not reduced deliverables. Add discovered material improvements
and their evidence here or in the linked review record throughout implementation.

## Acceptance

- A new player can feed, comfort, play, inspect, talk/type, rename, use settings and recover saves.
  Pointer, keyboard and controller semantic routes remain present and tested.
- Face expressions distinguish ordinary attention, curiosity, affection, reluctance/refusal,
  discomfort, sleep and speech at normal gameplay size, with exact semantic ownership.
- Swimming, turning, stopping and appendage following are continuous, bounded and coherent.
- Physical food and toys reflect authoritative contact and movement. Quiet activities and
  relationship performances remain distinct and truthful with AI disabled.
- All optional worker/audio/device failures degrade without halting play or corrupting saves.
- The normal `cargo xtask verify` remains headless/offline and passes. Native debug
  `cargo xtask dev --fake-ai` and the adapted feel suites produce real nonempty evidence.
- Multiple independent code-review rounds and native visual/feel rounds have no unresolved known
  material finding. Unavailable platform hardware is recorded honestly; no release rebuild ritual.
- No legacy sprite renderer, sprite assets or abandoned compatibility implementation remains.

## Progress

The migration and its native-input, motion, lighting, capture and UI corrections are implemented.
Multiple independent code reviews and native review rounds are recorded in
[feel-review-voxel-migration.md](feel-review-voxel-migration.md). The full headless gate passes.
The seven-experience native baseline completed. Final native controls, quiet-life and interaction
recaptures pass, followed by all eight canonical aquarium checkpoints. No known material finding
remains in the exercised scope; evidence limits are explicit in the review record.

The head uses roughly 24 voxels across its main silhouette, with finer geometric eyes, brows,
mouth, fins and accents. There are twelve tapering body segments plus a tail. The fixed camera
keeps the expression readable while lighting and continuous part transforms reveal depth.

Linux CI installs the window, audio, controller and Wayland/XKB development libraries required by
the selected Bevy features. Native hardware acceptance in this session is macOS debug only;
Windows/Linux windows, physical controllers and subjective physical listening remain explicit
evidence limits. No release binaries or model bundles are part of this migration's iteration gate.
