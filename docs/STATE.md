# State

Last updated: **2026-09-08**.

## Now

- The voxel surface pass replaces accidental floor/icon gaps with continuous meshes and adds
  connected geometric bevels, rounded animal lighting normals, distinct material roles and softer
  dimensional light. The calm central bed, rear-corner banks and broad cave colors keep Mop
  central. All non-text world/UI visuals remain real geometry. See [voxel-surfaces.md](voxel-surfaces.md)
  and [art-bible.md](art-bible.md).
- Bevy 0.19 remains the cross-platform foundation. The renderer uses PBR, four-sample MSAA,
  filtered shadow maps and procedural directional environment lighting. No new GPU feature,
  hardware ray tracing, external art asset or custom full-screen pipeline is required. No simulation,
  save format, animation ownership or world-picking rule changed.
- Repeated independent code and native visual/temporal reviews cover surface comparisons,
  interaction chains, dialogue races, three-minute quiet life, failures, Normal/Large UI and actual
  macOS input. The headless gate passes 468 tests, 27 dialogue fixtures, 11 recognition fixtures
  and spoken-input replay. Detailed evidence and rejected experiments live in
  [feel-review-voxel-surfaces.md](feel-review-voxel-surfaces.md).
- The enamel care deck, grouped settings and expressive creature acting remain intact. Evidence
  covers native macOS debug rendering, sampled frames, synchronized traces and actual input;
  human full-speed perception, physical audio/controller feel and Windows/Linux native windows
  remain unclaimed. Distribution bundles predate Bevy and were not rebuilt.
- Local runtime choices remain Qwen3.5 0.8B Q4, eSpeak NG and Parakeet TDT 0.6B V3 INT8, with
  Moonshine Tiny as the lightweight recognition fallback. See [local-mouth.md](local-mouth.md),
  [stt-runtime.md](stt-runtime.md) and [creature-life-expression-rework.md](creature-life-expression-rework.md).

## Next

Run a focused human presentation calibration of the finished aquarium at full speed, with actual
speaker output and the intended viewing size. This closes the largest remaining evidence gap for
motion, shading stability and emotional readability before adding more visual scope; allow one
short play session plus a bounded correction pass. Use [feel-review-voxel-surfaces.md](feel-review-voxel-surfaces.md)
and [feel-review-loop.md](feel-review-loop.md) to record concrete observations.

## Candidates Not Chosen

- **Windows/Linux native and physical-controller acceptance:** a few hours per available host/device;
  unblocks desktop release confidence and complete primary-input coverage. Hardware/runtime setup
  is the largest unknown and cannot be replaced by headless tests. Keep this release checkpoint
  explicit; it does not justify rebuilding bundles during ordinary presentation iteration.
- **A ray-traced rendering pipeline:** several days or more, with hardware portability, temporal
  stability and visual payoff as the main unknowns. The native comparisons supported ordinary
  Bevy meshing/material/light changes; revisit only for a concrete lighting result this path cannot
  deliver. A technology rewrite by itself would not close the perception gap.
- **Full 3D navigation and collision:** several days or more; would enable depth movement, but its
  gameplay value remains unresolved. Pure-core tests could cover geometry, while readability would
  need native review. The existing scene satisfies the visual direction without a new simulation rule.

## Learned Recently

- Surface contract and evidence: [voxel-surfaces.md](voxel-surfaces.md), [feel-review-voxel-surfaces.md](feel-review-voxel-surfaces.md).
- Renderer/art ownership: [architecture.md](architecture.md), [art-bible.md](art-bible.md).
- Native capture/input procedures: [development-harness.md](development-harness.md), [feel-review-loop.md](feel-review-loop.md).
- Prior UI/acting work: [voxel-craft-pass.md](voxel-craft-pass.md), [feel-review-voxel-craft.md](feel-review-voxel-craft.md).
- Product authority: [game-design-philosophy.md](game-design-philosophy.md), [v1-plan.md](v1-plan.md).
- Save/action/relationship semantics: [relationship-causality-rework.md](relationship-causality-rework.md),
  [relationship-expression-design.md](relationship-expression-design.md).
- Distribution obligations: [packaging.md](packaging.md), [distribution checklist](acceptance/distribution-checklist.md).
