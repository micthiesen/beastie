# State

Fast-moving work state and chosen next step. This records the work, not machine state or
uncommitted changes. Durable findings live in the linked docs.

Last updated: **2026-08-15** (local AI, speech, and PixelLab tooling preflight completed.)

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

## Next

Build the deterministic berry-memory acceptance slice in `beastie-core`: preference formation,
sleep/save/reload, candidate-memory selection, and multi-day invariant simulation. This proves
the authoritative game loop before renderer or model work widens the surface.

## Candidates Not Chosen

- **Interactive room shell**: wait until core events and intentions are stable enough to render.
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
