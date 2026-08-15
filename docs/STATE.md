# State

Fast-moving work state and chosen next step. This records the work, not machine state or
uncommitted changes. Durable findings live in the linked docs.

Last updated: **2026-08-15** (Mac-led MVP vertical slice complete.)

## Now

- The product and technical direction is defined in [mvp-spec.md](mvp-spec.md): one persistent
  room, a deterministic creature simulation, and a tiny offline AI "mouth."
- The workspace boundaries are established: pure core, protocol, view, worker, game shell, and
  `xtask` tooling.
- macOS is a first-class target alongside the specification's Windows and Linux targets.
- Local LLM inference, local TTS generation, and authenticated PixelLab generation all work on the
  development Mac. Results, caveats, and exact smoke measurements are in
  [tooling-preflight.md](tooling-preflight.md).
- The fixture dialogue backend remains the deterministic default. The local backend uses strict
  bounded JSONL, one retry, factual authored fallback, deterministic content lanes, and the selected
  Qwen3.5 0.8B Q4 model. Exact evaluation and runtime caveats are in
  [local-mouth.md](local-mouth.md).
- The game reuses one long-lived worker and one authenticated loopback `llama-server` sidecar. The
  packaged runtime discovers its sibling worker, model, and server without environment variables.
- Release speech uses a separate eSpeak NG process through a persistent worker, deterministic cache,
  and asynchronous playback. The aggregate package preserves GPLv3 licensing and corresponding
  source while Beastie's Rust binaries remain MIT. Kitten remains a non-default experiment.
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
- The Stage 5 acceptance fixture drives the entire three-day berry grudge through play, talk,
  reactions, sleep, checkpoint restoration, concept growth, individual idiolect, and silent food
  rejection. Headless, visible fixture, and full packaged local paths all pass.
- The compact toy chooser exposes Ball, Bell, and Sock identically to pointer and focus input. Exact
  v1 saves migrate into schema v2 without invented absence time and preserve deterministic futures.
- `cargo xtask package` builds and audits a complete offline platform layout. The final macOS proof
  is 405 files and 632,822,390 bytes with release binaries, Qwen, llama.cpp, eSpeak NG, voice data,
  licenses, corresponding source, assets, and per-file hashes.

## Next

Run the deferred native acceptance pass on Windows and Linux, then verify a physical controller and
macOS pointer delivery on a host not affected by the Alacritty/yabai focus stack. After that, make
the distribution wrapper and Steam-facing compliance material from the proven package manifest.

## Candidates Not Chosen

- **Qwen3 0.6B Q8**: 5/12 with six fallbacks and 0/3 permitted-sharpness cases, versus Qwen3.5 Q4 at
  11/12 with one fallback and 3/3 permitted-sharpness cases → [local-mouth.md](local-mouth.md).
- **Shipping sherpa-onnx 1.13.5**: the native spike works, but its static espeak-ng dependency is
  GPLv3 and is blocked from the MIT-only release path → [local-mouth.md](local-mouth.md).
- **Piper Plus v1.13**: the current 38 MB model aborts before load on a multi-codepoint phoneme key,
  and its model redistribution terms remain ambiguous → [tts-release-candidate.md](tts-release-candidate.md).
- **Flite 2.2**: tiny and extremely fast on macOS, but its stale Win32 build path is not a credible
  first-class Windows route → [flite-release-candidate.md](flite-release-candidate.md).

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
- Qwen3.5 0.8B Q4 passes 12/12 corpus cases with no fallback and an 882 ms cold-process median on
  Metal. The packaged warm sidecar returned two valid turns in 1,156 ms total including load;
  already-loaded probes took 107 ms and 52 ms → [local-mouth.md](local-mouth.md).
- Native Kitten nano TTS loads in 174 to 206 ms and synthesizes at RTF 0.21 on the development Mac.
  Feature-gated game wiring works, but release licensing remains open → [local-mouth.md](local-mouth.md).
- eSpeak NG 1.52 produces a 3.36-second crude-voice line in under 10 ms at 3.1 MB RSS. The real
  Beastie TTS worker and final package both generated and played validated cached speech.
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
- AI worker watchdogs contain and kill the complete child process group on macOS/Linux. Windows
  currently reaps the direct worker only; Job Object containment remains a Stage 5 release blocker
  so a worker crash or forced shutdown cannot orphan a runtime subprocess.
- The reviewed local-mouth boundary bounds input, output, channels, cache files, and WAV duration;
  rejects unsolicited replies and protected-class violence; invalidates stale speech; and reaps
  worker process groups on macOS/Linux. A fresh native Kitten smoke and the three-capture visible
  fixture path both passed after hardening → [local-mouth.md](local-mouth.md).
- On this Mac, yabai can intermittently leave a ggez release window blank before its first frame.
  Stopping the service for the automated run and restarting it afterward makes the identical package
  complete immediately → [development-harness.md](development-harness.md).
