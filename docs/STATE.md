# State

Fast-moving work state and chosen next step. This records the work, not machine state or
uncommitted changes. Durable findings live in the linked docs.

Last updated: **2026-08-15** (personality, belief, conversation and content design recorded.)

## Now

- The product and technical direction is defined in [mvp-spec.md](mvp-spec.md): one persistent
  room, a deterministic creature simulation, and a tiny offline AI "mouth."
- The workspace boundaries are established: pure core, protocol, view, worker, game shell, and
  `xtask` tooling.
- macOS is a first-class target alongside the specification's Windows and Linux targets.
- The current AI worker is fixture-backed. No real LLM, TTS runtime, model weight, or generated
  art is integrated yet.

## Next

Build the deterministic berry-memory acceptance slice in `beastie-core`: preference formation,
sleep/save/reload, candidate-memory selection, and multi-day invariant simulation. This proves
the authoritative game loop before renderer or model work widens the surface.

## Candidates Not Chosen

- **Interactive room shell**: wait until core events and intentions are stable enough to render.
- **Real local LLM benchmark**: wait until the protocol fixture corpus can score candidates.
- **Pixel-art generation**: first approve the tiny art bible and asset-validation contract.

## Learned Recently

- The MVP now treats learned profanity, crudeness, spite and provocation as simulation-backed
  social habits; adds traceable mistaken beliefs, contextual player reactions, nonverbal spite,
  scarce conversation, ageless-creature framing, and a mature-but-not-Adult-Only content boundary
  → [mvp-spec.md](mvp-spec.md).
- The simulation/model authority boundary, target acceptance scenario, performance budgets, and
  explicit exclusions are documented in [mvp-spec.md](mvp-spec.md).
- The project uses the solo `/next` and `/wrap` continuity pattern adapted from `../stillair`.
