# State

Fast-moving work state and chosen next step. This records the work, not machine state or
uncommitted changes. Durable findings live in the linked docs.

Last updated: **2026-08-15** (playable fixture-AI enclosure complete.)

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
- Headless interaction and visible-game scripting share one semantic `GameSession` command
  boundary. Its current commands, evidence path, and remaining real-device checks are recorded in
  [development-harness.md](development-harness.md).
- The deterministic berry-memory acceptance slice is implemented: seeded preferences, concrete
  memories, traceable beliefs, save/reload with RNG continuity, grounded recall, reaction-shaped
  social habits, persistent nonverbal rejection, and accelerated active days.
- `beastie-session` now provides the reusable semantic command boundary. `cargo xtask play` accepts
  bounded versioned JSONL, survives malformed commands, and replays the checked-in berry-grudge
  fixture with deterministic fake AI. The replay is part of `cargo xtask verify`.
- The fixture-AI enclosure is playable at a fixed 320×180 logical resolution: one-room projection,
  semantic creature movement, contextual feeding/play/comfort/tidy/talk actions, reactions,
  keyboard/controller focus, text entry and on-screen keyboard, and asynchronous dialogue fallback.
- Durable session saves use schema version 2, preserve RNG/request continuity, recover an interrupted
  replacement from backup, apply bounded non-lethal absence once, and keep offline time out of the
  active-play development clock. Scripted visible runs start fresh and never touch the player save.
- `fixtures/scenarios/room-shell.jsonl` drives the visible shell through ordinary session commands
  and produces deterministic 320×180 PNG evidence. Native macOS keyboard Talk submission is proven;
  pointer action delivery and physical-controller input still need a focused host check.

## Next

Establish canonical art and sound presentation: define the tiny art bible, generate and deliberately
select the creature/furniture/UI anchors, add asset validation and provenance, then replace the
geometric room with restrained animation, lighting/window variation, UI sounds, and authored
nonverbal creature noises.

## Candidates Not Chosen

- **Real local LLM benchmark**: the runtime smoke is complete; wait until the protocol fixture
  corpus can score candidates instead of selecting from one-off prompts.
- **Local mouth integration**: first build the dialogue eval corpus so model and voice selection are
  evidence-based rather than chosen from one-off prompts.

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
- The native macOS game launches reliably through Alacritty and produces direct logical-framebuffer
  captures. Real keyboard navigation and Talk submission work. Pointer events currently appear to
  fall through the visible game window in the Alacritty/Metal window stack despite successful focus;
  controller hardware has not yet been exercised → [development-harness.md](development-harness.md).
- Fixed simulation ticks make visible-frame and accelerated headless advancement equivalent. Room
  movement is semantic in core and pixel-positioned only in view; offline absence uses bounded
  analytic changes and does not consume RNG or advance active-play days.
- The initial full build is Mac-led but preserves portable headless boundaries. Windows and Linux
  native acceptance remains part of MVP completion, not a blocker for the tooling preflight or
  first autonomous implementation pass → [build-plan.md](build-plan.md).
- Codex is the only coding-agent environment required for the MVP. Claude Code/rulesync compatibility
  work is explicitly out of scope → [build-plan.md](build-plan.md).
- Flattened internally tagged Serde enums cannot also use `deny_unknown_fields` on the outer
  envelope. Keep strictness on `SessionCommand`; the outer `CommandEnvelope` must allow the
  flattened `command` tag.
