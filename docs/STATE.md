# State

Last updated: **2026-09-08**.

## Now

- Detail polish is implemented: separated binding rows, focus restored after rebinding, continuous
  water color and calmer toy shadows. The existing creature, care loop and composition remain.
  See [detail review](feel-review-details-20260908.md) and [contract](detail-polish.md).
- More expensive coverage experiments were rejected after native measurement. The selected local
  shadow refinement preserves the original world/UI coverage counts; benchmark evidence and
  sampling limits are recorded in the review.
- Controlled feel recordings now isolate scripted input and reject black video frames. Rejected
  attempts and the repaired evidence procedure are documented in [feel-review-loop.md](feel-review-loop.md).
- The prior creature-presence and care work remains complete. Its broader relationship, sound,
  microphone and saved-history evidence lives in the
  [presence review](feel-review-creature-presence-20260908.md).
- The gate passes 524 tests, 27 dialogue fixtures, 11 STT fixtures and spoken-input replay. Native
  comparisons use macOS Metal debug rendering. Human full-speed perception, physical listening,
  controller comfort and Windows/Linux/lower-end GPU behavior remain unclaimed. No release
  binary, installer or model bundle was rebuilt.

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

- Detail findings, measured rendering tradeoffs and capture integrity: [detail review](feel-review-details-20260908.md).
- Broader native comparisons and audio limits: [presence review](feel-review-creature-presence-20260908.md).
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
