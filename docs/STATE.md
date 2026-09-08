# State

Last updated: **2026-09-08**.

## Now

- All seven holistic feel findings are implemented: quieter composition and care discovery,
  microphone guidance, staggered recognizable toys, shelter clearance, semantic sound priority,
  faithful audio evidence and deferred words that respect current care. See
  [implementation review](feel-review-creature-presence-20260908.md) and
  [final contract](creature-presence-and-care.md).
- Iteration also corrected exact toy picking, autonomous dismissal of the care invitation, a
  permanent microphone notice and incomplete sub-frame audio evidence. Regression tests cover
  current ownership, moving/carried save continuity, geometry and audio cancellation.
- Native validation covers the seven baseline experiences, six saved-history cases, a second
  quiet seed, a subtitles-off relationship sequence and real Normal/Large pointer, keyboard and
  microphone acquisition/release. Independent visual, causal, audio and code findings are resolved.
  Exact outcomes, evidence provenance and rejected attempts live in the implementation review.
- The aquarium retains its tabletop interface and ordinary-GPU geometric renderer, including
  lettering. Simulation truth, direct care, old save coordinates, offline play and optional
  inference/TTS remain intact. See [architecture.md](architecture.md).
- The final gate passes 516 tests, 27 dialogue fixtures, 11 STT fixtures and spoken-input replay.
  Visible changes also passed native debug validation with fake AI.
- Validation uses macOS Metal debug rendering. Human full-speed perception, physical listening,
  spoken recognition quality, controller comfort and Windows/Linux/lower-end GPU behavior remain
  unclaimed. No release binary, installer or model bundle was rebuilt.

## Next

Measure the current renderer and input path on Windows, Linux and an available lower-end GPU using
ordinary debug builds. This closes a concrete driver/performance uncertainty before adding more
rendering work; record host specifications, frame cadence and interaction results, without treating
video's fixed recording rate as measured runtime performance. See
[renderer contract](raytraced-aquarium.md) and [MVP performance targets](mvp-spec.md#performance-budget).
This needs the actual hosts/devices; allow a few hours per platform, with driver behavior and GPU
cost as the largest unknowns. It does not require release packaging.

## Candidates Not Chosen

- **Human full-speed and listening calibration:** a short observation session can assess attachment,
  unaided discovery and mix taste beyond sampled evidence. It needs human perception; no remaining
  concrete defect from this review justifies another speculative implementation pass.
- **Further lighting or reconstruction features:** several days plus native comparisons. Their
  value depends on hardware measurements and a demonstrated visual need, so they wait.
- **Full 3D navigation and collision:** several days or more, with headless geometry tests and native
  readability review. It changes gameplay without unblocking the current care experience.

## Learned Recently

- Final findings, repeated native comparisons, audio reconstruction limits and rejected input
  diagnostics: [implementation review](feel-review-creature-presence-20260908.md).
- Original observations and implemented design: [holistic review](feel-review-holistic-20260908.md),
  [creature presence and care](creature-presence-and-care.md).
- Native capture and role-aware sound evidence: [feel-review-loop.md](feel-review-loop.md),
  [audio-direction.md](audio-direction.md).
- Renderer, mesh and art ownership: [architecture.md](architecture.md), [art-bible.md](art-bible.md),
  [raytraced-aquarium.md](raytraced-aquarium.md).
- Product authority and relationship semantics: [game-design-philosophy.md](game-design-philosophy.md),
  [relationship-causality-rework.md](relationship-causality-rework.md).
- Local inference and recognition choices: [local-mouth.md](local-mouth.md),
  [stt-runtime.md](stt-runtime.md).
