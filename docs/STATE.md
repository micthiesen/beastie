# State

Last updated: **2026-09-08**.

## Now

- The complete aquarium, creature, care deck and text use one custom ordinary-GPU ray tracer.
  Bevy 0.19 supplies the application infrastructure; the old PBR scene, sprite and raster text/UI
  paths are removed. Text is shaped and tessellated geometry. See
  [raytraced-aquarium.md](raytraced-aquarium.md) and [architecture.md](architecture.md).
- Cached mesh acceleration, near-first traversal, stable coverage samples, soft area lighting and
  restrained indirect/reflected light preserve the authored voxel surfaces and expressive face.
  There is no temporal accumulation, hardware RT requirement or alternate renderer toggle.
  Simulation, saves, continuous animation and picking retain their existing ownership.
- Multiple independent code and native visual/causal reviews cover world poses, all Normal/Large
  UI, interactions, dialogue races, three-minute quiet life and failure recovery. The headless gate
  passes 483 tests, 27 dialogue fixtures, 11 recognition fixtures and spoken-input replay. Actual
  input and frame-time evidence, iteration findings and coverage limits are recorded in
  [feel-review-raytracing.md](feel-review-raytracing.md).
- Native evidence covers macOS/Metal debug rendering. Shader translation covers MSL, HLSL and
  SPIR-V without optional RT capabilities; Windows/Linux native drivers, lower-end GPUs, human
  full-speed perception and physical audio/controller feel remain explicit acceptance checkpoints.
  Distribution bundles predate Bevy and were not rebuilt.
- Local runtime choices remain Qwen3.5 0.8B Q4, eSpeak NG and Parakeet TDT 0.6B V3 INT8, with
  Moonshine Tiny as the lightweight recognition fallback. See [local-mouth.md](local-mouth.md),
  [stt-runtime.md](stt-runtime.md) and [creature-life-expression-rework.md](creature-life-expression-rework.md).

## Next

Run native renderer acceptance on Windows/Linux and a lower-end ordinary GPU, alongside live
perception and physical audio/controller checks. This closes the largest remaining release unknown
before adding rendering complexity; use [distribution checklist](acceptance/distribution-checklist.md)
and the evidence boundaries in [feel-review-raytracing.md](feel-review-raytracing.md).
Allow a few hours per available host/device; driver and hardware availability are the main unknowns.

## Candidates Not Chosen

- **Further lighting or reconstruction features:** several days and fresh native comparisons.
  They could add visual richness, but current reviews found no remaining material rendering defect;
  broad hardware measurements should identify the next concrete need first.
- **Full 3D navigation and collision:** several days or more; enables depth movement but changes
  gameplay and readability. Pure-core fixtures can cover geometry, while perceived value needs
  native review. The current presentation does not require a new simulation rule.

## Learned Recently

- Unified rendering contract/evidence: [raytraced-aquarium.md](raytraced-aquarium.md), [feel-review-raytracing.md](feel-review-raytracing.md).
- Mesh construction and prior comparisons: [voxel-surfaces.md](voxel-surfaces.md), [feel-review-voxel-surfaces.md](feel-review-voxel-surfaces.md).
- Renderer/art ownership: [architecture.md](architecture.md), [art-bible.md](art-bible.md).
- Native capture/input procedures: [development-harness.md](development-harness.md), [feel-review-loop.md](feel-review-loop.md).
- Prior UI/acting work: [voxel-craft-pass.md](voxel-craft-pass.md), [feel-review-voxel-craft.md](feel-review-voxel-craft.md).
- Product authority: [game-design-philosophy.md](game-design-philosophy.md), [v1-plan.md](v1-plan.md).
- Save/action/relationship semantics: [relationship-causality-rework.md](relationship-causality-rework.md),
  [relationship-expression-design.md](relationship-expression-design.md).
- Distribution obligations: [packaging.md](packaging.md), [distribution checklist](acceptance/distribution-checklist.md).
