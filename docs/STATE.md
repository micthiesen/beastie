# State

Fast-moving work state and chosen next step. This records the work, not machine state or
uncommitted changes. Durable findings live in the linked docs.

Last updated: **2026-08-15** (canonical art and authored sound integrated.)

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
- The canonical presentation pass is integrated. PixelLab-generated room and identity-consistent
  creature poses replace geometry when valid, while corrupt or missing art falls back cleanly.
  Deterministic idle/walk frames, pose offsets, and three-phase room/window lighting derive from
  authoritative game time. The latest room-shell capture proves the full 320×180 art path.
- `cargo xtask asset check` validates the manifest, palette, provenance, PNG dimensions/alpha,
  animation completeness, final-over-generated resolution, and WAV format. The art bible is in
  [art-bible.md](art-bible.md); sound provenance and measurements are in
  [audio-direction.md](audio-direction.md).
- Five original sounds cover selection, confirmation, comfort/noise, annoyance, and sleep. Runtime
  playback is event-driven and degrades silently if assets, decoding, playback, or an output device
  are unavailable.

## Next

Integrate the replaceable local mouth: build the checked-in dialogue evaluation corpus, benchmark
small local model candidates for grounding, protocol compliance, latency, permitted sharpness, and
prohibited-output escape, then integrate the winner behind the worker boundary. Compare the proven
KittenTTS path with a practical pinned sherpa-onnx/Kokoro alternative using actual Beastie lines and
ship the selected local voice without Python.

## Candidates Not Chosen

- **Real local LLM benchmark**: the runtime smoke is complete; wait until the protocol fixture
  corpus can score candidates instead of selecting from one-off prompts.
- **Final hardening**: defer native Windows/Linux acceptance, save migrations, packaging inputs, and
  the unresolved macOS pointer/controller checks until the real local worker exists end to end.

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
- Runtime art resolves `assets/final/<id>.png` before `assets/generated/<id>.png`; animation frames
  use `<id>-<frame>.png`. Both candidates are validated so a promoted final cannot conceal a corrupt
  generated source → [art-bible.md](art-bible.md).
- The initial full build is Mac-led but preserves portable headless boundaries. Windows and Linux
  native acceptance remains part of MVP completion, not a blocker for the tooling preflight or
  first autonomous implementation pass → [build-plan.md](build-plan.md).
- Codex is the only coding-agent environment required for the MVP. Claude Code/rulesync compatibility
  work is explicitly out of scope → [build-plan.md](build-plan.md).
- Flattened internally tagged Serde enums cannot also use `deny_unknown_fields` on the outer
  envelope. Keep strictness on `SessionCommand`; the outer `CommandEnvelope` must allow the
  flattened `command` tag.
