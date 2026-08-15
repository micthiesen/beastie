# State

Fast-moving work state and chosen next step. This records the work, not machine state or
uncommitted changes. Durable findings live in the linked docs.

Last updated: **2026-08-15** (full-MVP build flow and agent control harness decided.)

## Now

- The product and technical direction is defined in [mvp-spec.md](mvp-spec.md): one persistent
  room, a deterministic creature simulation, and a tiny offline AI "mouth."
- The workspace boundaries are established: pure core, protocol, view, worker, game shell, and
  `xtask` tooling.
- macOS is a first-class target alongside the specification's Windows and Linux targets.
- Local LLM inference, local TTS generation, and authenticated PixelLab generation all work on the
  development Mac. Results, caveats, and exact smoke measurements are in
  [tooling-preflight.md](tooling-preflight.md).
- The current AI worker is still fixture-backed. No real model weight, native TTS runtime, or
  promoted generated art is integrated yet.
- The autonomous implementation target is the complete MVP. Its ordered slices and working rules
  are recorded in [build-plan.md](build-plan.md).
- Headless interaction and visible-game scripting will share one semantic command protocol. The
  required pre-build proof is recorded in [development-harness.md](development-harness.md).

## Next

Build and prove the shared development harness before widening the MVP: semantic JSONL commands,
headless observations, identical visible-game scenario replay, logical-framebuffer PNG capture, and
one genuine macOS input path. Preserve the proof as a fixture, then begin the berry-memory slice.

## Candidates Not Chosen

- **Berry-memory acceptance slice**: it remains the first product slice, but follows the small
  harness gate because every later interaction becomes faster to implement and validate.
- **Interactive room shell**: build after the harness and authoritative berry-memory spine.
- **Real local LLM benchmark**: the runtime smoke is complete; wait until the protocol fixture
  corpus can score candidates instead of selecting from one-off prompts.
- **Pixel-art generation**: first create the approved art direction's anchor assets and
  asset-validation contract.

## Learned Recently

- The MVP uses a fixed, slightly three-quarter dollhouse view; keyboard/mouse and controller share
  a minimal contextual interface; and the cozy-grotty pixel art uses warm light, deep shadow, awkward
  expressiveness, and the window as its main source of visual change → [mvp-spec.md](mvp-spec.md).
- The MVP now treats learned profanity, crudeness, spite and provocation as simulation-backed
  social habits; adds traceable mistaken beliefs, contextual player reactions, nonverbal spite,
  scarce conversation, ageless-creature framing, and a mature-but-not-Adult-Only content boundary
  → [mvp-spec.md](mvp-spec.md).
- The simulation/model authority boundary, target acceptance scenario, performance budgets, and
  explicit exclusions are documented in [mvp-spec.md](mvp-spec.md).
- The project uses the solo `/next` and `/wrap` continuity pattern adapted from `../stillair`.
- `llama.cpp` needs `--device none --no-op-offload -ngl 0` for a truly CPU-only run on the
  development Mac. Qwen3 0.6B and Qwen3.5 0.8B both ran but contradicted the first crude grounding
  prompt, so model choice remains open → [tooling-preflight.md](tooling-preflight.md).
- KittenTTS generated intelligible speech substantially faster than real time, and PixelLab's MCP
  completed a seeded transparent-sprite job. Native TTS packaging and canonical art selection are
  still explicit build tasks → [tooling-preflight.md](tooling-preflight.md).
- Ordinary gameplay validation will use semantic commands, not coordinate clicks. Coordinate input
  is reserved for targeted mouse-mapping and real-device tests; rendered scenarios save the logical
  framebuffer directly → [development-harness.md](development-harness.md).
- The initial full build is Mac-led but preserves portable headless boundaries. Windows and Linux
  native acceptance remains part of MVP completion, not a blocker for the tooling preflight or
  first autonomous implementation pass → [build-plan.md](build-plan.md).
- Codex is the only coding-agent environment required for the MVP. Claude Code/rulesync compatibility
  work is explicitly out of scope → [build-plan.md](build-plan.md).
