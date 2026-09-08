# State

Last updated: **2026-09-08**.

## Now

- The holistic feel review is complete as a sampled visual/causal/audio audit: seven fresh native
  experiences, 46,476 frames and an additional actual-input session. Seven accepted findings cover
  care discoverability, voice guidance, toy composition, shelter fit, semantic ducking, reference
  audio fidelity and deferred-language ownership. See
  [feel-review-holistic-20260908.md](feel-review-holistic-20260908.md). Full-speed human perception,
  listening and completely caption-free relationship assessment remain explicit gaps.
- The aquarium now uses a compact tabletop interface: a frame-mounted nameplate, a 33-unit care
  rail, explicit text actions, a quiet settings control and a distinct enamel Feed button. Speech
  names its speaker and labels reactions; rename and sound settings explain their purpose.
  See [tabletop-aquarium.md](tabletop-aquarium.md).
- Broad leaves, asymmetric planting, a terracotta shelter and more recognizable toys replace the
  mirrored piles and disconnected decorations. Continuous geometric surfaces and all lettering
  remain on the ordinary-GPU ray tracer within Bevy. No sprites, font atlas or RT hardware required.
- Native input testing exposed missing body picking and context menus covering the face. Body
  volumes now use exact presented transforms, plants/shelter use precise mesh intersections, and
  context placement avoids the head at opening while keeping controls stationary afterward. Simulation, saves,
  audio ownership, offline behavior and optional worker boundaries are preserved.
- Multiple independent code and native visual/causal reviews cover Normal/Large UI, interactions,
  dialogue races, failure recovery and actual macOS input. The gate passes 492 tests, 27 dialogue
  fixtures, 11 recognition fixtures and spoken-input replay. Evidence, rejected findings and
  perception limits live in [feel-review-tabletop.md](feel-review-tabletop.md).
- Native evidence remains macOS/Metal debug rendering. Windows/Linux native drivers, lower-end
  GPUs, physical audio/controller feel and human full-speed aesthetic judgment remain unclaimed.
  No distribution bundle was rebuilt. Local runtime choices remain Qwen3.5 0.8B Q4, eSpeak NG and
  Parakeet TDT 0.6B V3 INT8, with Moonshine Tiny as the lightweight recognition fallback; see
  [local-mouth.md](local-mouth.md) and [stt-runtime.md](stt-runtime.md).

## Next

Finish the authorized implementation of
[creature-presence-and-care.md](creature-presence-and-care.md). The user approved all changes and
requested no further input. All seven changes are integrated, the headless gate passes, and native
review is in progress. See [implementation review](feel-review-creature-presence-20260908.md) for
comparisons and the rejected stale-cooldown scenario capture. No Codex goal was requested or set.
Keep the current renderer; human aesthetic/listening and Windows/Linux/lower-end GPU claims remain
separate from the automated and sampled evidence.

## Candidates Not Chosen

- **Windows/Linux and lower-end native acceptance:** a few hours per available host/device;
  closes driver/performance uncertainty but does not answer this aesthetic calibration. Keep it
  explicit before release.
- **Further lighting or reconstruction features:** several days and fresh native comparisons.
  They could add visual richness, but current reviews found no remaining material rendering defect;
  broad hardware measurements should identify the next concrete need first.
- **Full 3D navigation and collision:** several days or more; enables depth movement but changes
  gameplay and readability. Pure-core fixtures can cover geometry, while perceived value needs
  native review. The current presentation does not require a new simulation rule.

## Learned Recently

- Holistic findings, evidence limits and next contract:
  [feel-review-holistic-20260908.md](feel-review-holistic-20260908.md),
  [creature-presence-and-care.md](creature-presence-and-care.md).
- Tabletop direction and user calibration: [tabletop-aquarium.md](tabletop-aquarium.md), [feel-review-tabletop.md](feel-review-tabletop.md).

- Unified rendering contract/evidence: [raytraced-aquarium.md](raytraced-aquarium.md), [feel-review-raytracing.md](feel-review-raytracing.md).
- Mesh construction and prior comparisons: [voxel-surfaces.md](voxel-surfaces.md), [feel-review-voxel-surfaces.md](feel-review-voxel-surfaces.md).
- Renderer/art ownership: [architecture.md](architecture.md), [art-bible.md](art-bible.md).
- Native capture/input procedures: [development-harness.md](development-harness.md), [feel-review-loop.md](feel-review-loop.md).
- Prior UI/acting work: [voxel-craft-pass.md](voxel-craft-pass.md), [feel-review-voxel-craft.md](feel-review-voxel-craft.md).
- Product authority: [game-design-philosophy.md](game-design-philosophy.md), [v1-plan.md](v1-plan.md).
- Save/action/relationship semantics: [relationship-causality-rework.md](relationship-causality-rework.md),
  [relationship-expression-design.md](relationship-expression-design.md).
- Distribution obligations: [packaging.md](packaging.md), [distribution checklist](acceptance/distribution-checklist.md).
