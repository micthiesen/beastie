# State

Fast-moving work state and chosen next step. Durable detail lives in the linked documents.

Last updated: **2026-08-15** (aquarium UI polish and layer pass complete).

## Now

- The canonical product is the open-water aquarium described in [v1-plan.md](v1-plan.md). The old
  dollhouse room survives only in historical MVP fixtures and [mvp-spec.md](mvp-spec.md).
- Core owns fixed-point position and velocity, facing, gaze, steering, action phases, physical food,
  stable aquarium objects, routines, memories, beliefs, development, initiated behavior, and
  versioned migration. Rendering owns logical pixels and never establishes facts.
- View and game implement a cropped 320x180 aquarium and an integer-scaled, sprite-first PixelLab
  actor. Six moods have side-facing and player-facing full-body art, every mood has three curated
  speech shapes, and accepted eating, food rejection, noticing, toy refusal, comfort, affection,
  and stronger swimming have distinct full-body acting. Attention, heart, mouth-particle, wake,
  sand, and sleep effects are authored sprites. These reactions project typed simulation events;
  model prose never chooses game outcomes. Procedural facial rectangles are gone from the
  normal path; the simple renderer creature exists only as a missing-asset safety net. The
  persistent compose deck now uses a PixelLab mixed-case font, authored status/action icons,
  aquarium-material chrome, contextual help instead of permanent shortcut prose, and explicit
  panel/icon/text/focus layer bands. Speech chooses the side opposite the creature. Semantic
  pointer/keyboard/controller targets, accessibility settings, binding UI,
  naming, recoverable saves, and opt-in transcript export remain. See
  [architecture.md](architecture.md) and [art-bible.md](art-bible.md).
- `fixtures/scenarios/aquarium-v1.jsonl` is the canonical headless V1 interaction. Its visible twin,
  `aquarium-v1-visible.jsonl`, captures eight aquarium checkpoints through the real ggez shell. The
  berry-grudge, Stage 5, and room-shell fixtures remain regression and migration evidence. See
  [development-harness.md](development-harness.md).
- The selected local mouth is Qwen3.5 0.8B Q4 behind one authenticated loopback llama.cpp sidecar.
  The expanded V1 corpus passed 26/26 with no fallback, permitted refusal, or prohibited escape;
  the portable CPU corpus remains 18/18. Exact model evidence and limitations are in
  [local-mouth.md](local-mouth.md).
- Release speech is a separate eSpeak NG process with bounded JSONL, emotion-derived pitch/speed,
  mouth timing, deterministic cache, and asynchronous playback. A real macOS smoke produced a
  3.36-second mono PCM16 line at 22,050 Hz in under 10 ms and about 3.1 MB maximum RSS. Packaging
  must retain GPLv3 license and corresponding source. See
  [tooling-preflight.md](tooling-preflight.md).
- Asset validation now checks 76 declared assets for provenance, palette policy, alpha, pixel
  density, dimensions, animation completeness, and final-over-generated resolution. The expression
  pass normalizes provider-padded frames to the canonical 80x80 canvas only after opaque-bounds
  checks. Aquarium provenance and deliberate rejected/curated-frame decisions are in
  `assets/manifest.toml`; visual and audio contracts are in [art-bible.md](art-bible.md) and
  [audio-direction.md](audio-direction.md).
- The semantic session boundary compacts accelerated `NeedChanged` noise without changing final
  state, RNG, or meaningful event order. Headless and visible adapters consume the same commands.
- macOS native window capture and real pointer/keyboard delivery were proven during the MVP. No
  physical controller has been attached. Windows and Linux native install, launch, save-path, and
  child-cleanup evidence has not been produced. CI or cross-compilation is not native proof.
- The final aquarium shell completed all eight logical captures from an Alacritty GUI child; the
  sprite pass repeated that run several times and the action-animation pass completed another
  eight-frame foreground run. Its inspected notice, swim, accepted-eating, dialogue, and comfort
  frames remained crisp and fully legible at 2x. The dialogue frame no longer reuses stale eating
  punctuation, and important reaction bodies stay inside the aquarium at permissive world edges.
  The earlier inspected dialogue frame showed a complete,
  fully visible player-facing creature rather than a cropped procedural face. Six-mood and
  six-talking-state galleries were inspected at the real 2x aquarium scale. Direct affection and
  forceful rejection now preempt stale low-priority presentation cues. The final packaged app also
  rendered and completed its earlier smoke run with yabai temporarily stopped, then exited without
  leaving game, model, or speech descendants.
- The UI polish pass completed a fresh eight-frame foreground shell capture at
  `target/captures/ui-polish-proof`. Native inspection confirmed crisp mixed-case text, an icon-led
  interaction deck, and no permanent keyboard-instruction line. That proof exposed speech covering
  the creature, so speech now anchors to the opposite side. A subsequent modal-only GUI attempt did
  not receive a drawable and was stopped once; deterministic view/game tests cover every modal and
  the allow-listed UI scenario remains for a later foreground proof.
- Installer wrappers, CI configuration, Steam inputs, achievements, and store-asset tooling exist.
  A fresh ad hoc signed macOS app and DMG passed the 473-file, 666,289,145-byte offline package
  audit; its packaged Qwen warm server returned two grounded replies, packaged eSpeak generated a
  validated cache entry, and all children exited. The prior bundle of the same game shell completed
  its packaged GUI smoke; the final remote-daemon retry did not acquire a drawable and was stopped.
  Signing identities, notarization credentials, Steam IDs/credentials, native Windows/Linux runs,
  physical-controller smoke, final store screenshots, and trailer remain release prerequisites.
- Three independent review lenses covered general correctness, runtime/content boundaries, and V1
  acceptance. Their surviving findings and the subsequent release-script, capture, dialogue, and
  staging findings were fixed. The final integrated `cargo xtask verify` gate is green.

## Next

V1 game work is complete. Use `/next` to choose between post-V1 creature depth and release work.
Release work must still collect native Windows/Linux install and launch evidence, a physical
controller smoke, Developer ID and Windows signing, notarization, final native store captures and
trailer, and Steam publication credentials. Do not turn those unperformed external checks into
prose claims.

## Durable pointers

- Product and acceptance direction: [v1-plan.md](v1-plan.md)
- Canonical product philosophy: [game-design-philosophy.md](game-design-philosophy.md)
- Runtime boundaries: [architecture.md](architecture.md)
- Test and capture commands: [development-harness.md](development-harness.md)
- Model and voice evidence: [local-mouth.md](local-mouth.md),
  [tooling-preflight.md](tooling-preflight.md)
- Release assembly and prerequisites: [packaging.md](packaging.md)
- Current native distribution checklist:
  [acceptance/distribution-checklist.md](acceptance/distribution-checklist.md)
- AI/content survey draft: [steam-ai-disclosure.md](steam-ai-disclosure.md)
