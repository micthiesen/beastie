# State

Last updated: **2026-09-07**.

## Now

- Beastie is fully migrated to Bevy 0.19: a fixed-camera 3D voxel tank, expressive geometric face,
  smooth creature motion and an articulated trailing body. World art, effects, icons and UI chrome
  are meshes; only text uses font rendering. The old ggez renderer, sprites, image catalog and
  PixelLab pipeline are removed. See [architecture.md](architecture.md), [art-bible.md](art-bible.md)
  and the completed [voxel migration contract](voxel-migration.md).
- Core/session truth, fixed-point aquarium coordinates, semantic interactions, private life,
  relationship performances, versioned saves and optional offline dialogue/TTS/STT remain intact.
  The authoritative interaction plane is 2D inside the 3D scene. Settings migrate to version 4;
  scripted native tests now preserve saved preferences as well as ordinary game state.
- Multiple code and native review rounds fixed lighting, grounding, captions, text bounds, gaze,
  capture ordering/timing/cleanup, head continuity and tail stability. The full seven-experience
  baseline, final quiet/interaction recaptures, native pointer/keyboard controls and eight aquarium
  checkpoints pass. `cargo xtask verify` passes 451 tests and dialogue/recognition fixtures.
  Evidence and limitations are in [feel-review-voxel-migration.md](feel-review-voxel-migration.md).
- Native evidence is macOS debug, with dense frame review and actual input. Human full-speed
  perception, physical listening, controller hardware and Windows/Linux Bevy windows are not
  claimed. CI retains all three desktop targets with explicit Linux build libraries. Earlier
  distribution bundles predate Bevy; no release bundle was rebuilt during migration.
- Existing local runtime choices remain Qwen3.5 0.8B Q4, eSpeak NG, and Parakeet TDT 0.6B V3 INT8,
  with Moonshine Tiny as the lightweight recognition fallback. Detailed behavior/runtime contracts
  remain in [creature-life-expression-rework.md](creature-life-expression-rework.md),
  [interaction-continuity-rework.md](interaction-continuity-rework.md),
  [local-mouth.md](local-mouth.md), and [stt-runtime.md](stt-runtime.md).

## Next

A short hands-on calibration of the completed voxel presentation, before selecting another feature.
This checks personal taste and perceived motion/audio against the sampled native evidence; it
beats a new content or navigation system because the engine migration is complete and these are
human perception questions. Use the [native feel workflow](feel-review-loop.md) to ground any new
finding; there is no known implementation fix waiting behind this follow-up.

## Candidates Not Chosen

- **Windows/Linux native and physical-controller acceptance:** a few hours per available host/device;
  unblocks renewed portability evidence, with hardware/runtime setup the largest unknown. Keep the
  previously parked Windows/controller checkpoint explicit rather than rebuilding release bundles
  during routine iteration.
- **Full 3D navigation and collision:** several days or more, with the product value of depth motion
  unresolved. The fixed-camera aquarium already uses real 3D geometry; changing authoritative
  navigation adds simulation scope without resolving an observed gameplay problem.

## Learned Recently

- Motion, capture and review evidence: [feel-review-voxel-migration.md](feel-review-voxel-migration.md).
- Current renderer and asset contract: [architecture.md](architecture.md), [art-bible.md](art-bible.md).
- Product authority: [game-design-philosophy.md](game-design-philosophy.md), [v1-plan.md](v1-plan.md).
- Save/action/relationship semantics: [relationship-causality-rework.md](relationship-causality-rework.md),
  [relationship-expression-design.md](relationship-expression-design.md).
- Commands and review method: [development-harness.md](development-harness.md),
  [feel-review-loop.md](feel-review-loop.md).
- Spoken input and distribution: [v2-plan.md](v2-plan.md), [packaging.md](packaging.md),
  [distribution checklist](acceptance/distribution-checklist.md).
