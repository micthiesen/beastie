# State

Last updated: **2026-09-08**.

## Now

- The four-area voxel craft pass is complete: a cohesive enamel care deck and grouped settings,
  coordinated facial/body acting, layered scenery under a fixed 12-degree overhead camera, and
  reusable UI, creature and environment art definitions. Bevy 0.19 renders solid geometry except
  font-rendered text. See [voxel-craft-pass.md](voxel-craft-pass.md), [art-bible.md](art-bible.md)
  and [architecture.md](architecture.md).
- Normal/Large typography, modal focus, controller keyboard presentation, speech sizing and native
  pointer mapping are exercised. Technical startup recovery stays in status UI. Core simulation,
  private life, relationship ownership, saves and offline optional-worker behavior remain intact;
  authoritative navigation is still a 2D plane inside the 3D aquarium.
- Multiple independent code and native feel reviews fixed every accepted material finding in the
  exercised scope. The seven-experience baseline, final 28-panel UI pass, final interaction replay
  and actual macOS pointer/keyboard evidence are in [feel-review-voxel-craft.md](feel-review-voxel-craft.md).
  `cargo xtask verify` passes 460 tests, 27 dialogue fixtures, 11 recognition fixtures and spoken-input replay.
- Evidence covers macOS debug rendering, dense sampled frames, synchronized traces and actual
  input. Human full-speed perception, physical listening/controller feel and Windows/Linux native
  windows remain unclaimed. CI retains all three desktop targets. Distribution bundles predate
  Bevy and were not rebuilt during routine iteration.
- Local runtime choices remain Qwen3.5 0.8B Q4, eSpeak NG and Parakeet TDT 0.6B V3 INT8, with
  Moonshine Tiny as the lightweight recognition fallback. See [local-mouth.md](local-mouth.md),
  [stt-runtime.md](stt-runtime.md) and [creature-life-expression-rework.md](creature-life-expression-rework.md).

## Next

Run a focused human presentation calibration of the finished aquarium at full speed, with actual
speaker output and the intended viewing size. This closes the largest remaining evidence gap for
fixed-camera readability and nonverbal behavior before adding visual scope; allow one short play
session plus a bounded correction pass, with subjective motion/audio judgment as the main unknown.
Use [feel-review-voxel-craft.md](feel-review-voxel-craft.md) and [feel-review-loop.md](feel-review-loop.md).

## Candidates Not Chosen

- **Windows/Linux native and physical-controller acceptance:** a few hours per available host/device;
  unblocks desktop release confidence and complete primary-input coverage. Hardware/runtime setup
  is the largest unknown and cannot be replaced by headless tests. Keep this release checkpoint
  explicit; it does not justify rebuilding bundles during ordinary presentation iteration.
- **Full 3D navigation and collision:** several days or more; would enable depth movement, but its
  gameplay value remains unresolved. Pure-core tests could cover geometry, while readability would
  need native review. The existing 3D scene satisfies the visual direction without taking on an
  unobserved simulation problem.

## Learned Recently

- Design and evidence: [voxel-craft-pass.md](voxel-craft-pass.md), [feel-review-voxel-craft.md](feel-review-voxel-craft.md).
- Renderer and art ownership: [architecture.md](architecture.md), [art-bible.md](art-bible.md).
- Native capture/input procedures: [development-harness.md](development-harness.md), [feel-review-loop.md](feel-review-loop.md).
- Prior migration evidence: [feel-review-voxel-migration.md](feel-review-voxel-migration.md).
- Product authority: [game-design-philosophy.md](game-design-philosophy.md), [v1-plan.md](v1-plan.md).
- Save/action/relationship semantics: [relationship-causality-rework.md](relationship-causality-rework.md),
  [relationship-expression-design.md](relationship-expression-design.md).
- Distribution obligations: [packaging.md](packaging.md), [distribution checklist](acceptance/distribution-checklist.md).
