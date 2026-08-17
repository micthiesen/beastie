# State

Fast-moving work state and chosen next step. Durable detail lives in the linked documents.

Last updated: **2026-08-17** (fifth creature-life feel review completed; implementation contract
proposed; Windows validation parked).

## Now

- The canonical product is the open-water aquarium described in [v1-plan.md](v1-plan.md). The old
  dollhouse room survives only in historical MVP fixtures and [mvp-spec.md](mvp-spec.md).
- Core owns fixed-point position and velocity, facing, gaze, steering, action phases, physical food,
  stable aquarium objects, routines, memories, beliefs, development, initiated behavior, and
  versioned migration. Rendering owns logical pixels and never establishes facts.
- View emits a cropped 320x180 aquarium plan and game projects it exactly 2x into a 640x360
  presentation image before native-resolution UI and text. Fixed 16:9 window sizes default to
  1280x720; fullscreen uses the largest integer presentation scale with letterboxing. Atkinson
  Hyperlegible Next replaces the generated microfont, world hover/focus follows sprite alpha rather
  than bounding rectangles, and deterministic half-pixel presentation offsets smooth motion
  between simulation ticks. The sprite-first PixelLab actor has side-facing and player-facing art
  for six moods, and every mood has three curated
  speech shapes, and accepted eating, food rejection, noticing, toy refusal, comfort, affection,
  and stronger swimming have distinct full-body acting. Attention, heart, mouth-particle, wake,
  sand, and sleep effects are authored sprites. These reactions project typed simulation events;
  model prose never chooses game outcomes. Procedural facial rectangles are gone from the
  normal path; the simple renderer creature exists only as a missing-asset safety net. The
  persistent compose deck uses native-resolution mixed-case text, authored status/action icons,
  aquarium-material chrome, contextual help instead of permanent shortcut prose, and explicit
  panel/icon/text/focus layer bands. Speech chooses the side opposite the creature. Semantic
  pointer/keyboard/controller targets, accessibility settings, binding UI,
  naming, recoverable saves, and opt-in transcript export remain. See
  [architecture.md](architecture.md) and [art-bible.md](art-bible.md).
- `fixtures/scenarios/aquarium-v1.jsonl` is the canonical headless V1 interaction. Its visible twin,
  `aquarium-v1-visible.jsonl`, captures eight aquarium checkpoints through the real ggez shell. The
  berry-grudge, Stage 5, and room-shell fixtures remain regression and migration evidence. See
  [development-harness.md](development-harness.md).
- `cargo xtask feel --suite baseline` now runs seven deterministic subjective experiences through
  the real game shell and produces synchronized 60 fps video, marker filmstrips, privacy-safe
  input/event/state/audio traces, retained speech WAVs, a reference audio mix and waveform, review
  scaffolding, and hash-pinned manifests. The first adjudicated pass fixed collapsed idle rhythm,
  invalid relationship accumulation, incomplete swim loading, stale and missing interaction
  receipts, semantic audio divergence, one-frame ducking, modal escape, and caption/mouth timing.
  Same-seed replays and real macOS pointer/keyboard evidence are recorded in
  [feel-review-baseline.md](feel-review-baseline.md); the repeatable method is in
  [feel-review-loop.md](feel-review-loop.md).
- A second same-seed review fixed caption/audio separation and stale reaction focus across normal,
  fallback, and multi-day dialogue. Autonomous toy visits now land with an authoritative authored
  play beat, repeat avoidance no longer biases the first fixed fallback, behavior labels describe
  travel and play honestly, and normal body placement respects the authored opaque envelope at
  aquarium edges. The new evidence also isolates the next feel problem: relationship state grows,
  but ordinary arrivals and dialogue barely express shared history. See
  [feel-review-second-pass.md](feel-review-second-pass.md).
- Relationship history is now expressed through one simulation-owned director rather than inferred
  independently by UI, audio, or dialogue. Six grounded motif families select sparse,
  interruptible notice/anticipate/act/recover beats from memories, beliefs, genuine visits,
  routines, and preferences. Cross-day evidence unlocks qualitative anticipation, comfort seeking,
  familiar-place recognition, food trust or grudges, shared-toy rituals, and return callbacks; one
  encounter remains a restrained notice. A migrated bounded ledger prevents repetition and
  survives exact save/reload. Dialogue receives the selected motif and evidence, retains only
  semantic history, retries one exact duplicate, and otherwise uses a grounded authored fallback.
  Native AI-on and AI-off evidence proves embodied callbacks, direct interruption, cross-day
  variety, and no-speech operation. See
  [relationship-expression-design.md](relationship-expression-design.md).
- Relationship expression now has exact present-moment causality. Trusted food and grudges bind to
  the matching authoritative action and retain subject, evidence, and resolved outcome through
  recovery; unrelated recollection occurs only as a traced standalone callback. Toy and comfort
  rituals enrich direct receipts without competing timelines. Presentation and audio use semantic
  owners and per-channel priority, so direct outcomes replace incompatible relationship residue
  without clearing UI, ambience, or physical effects. Save version 5 migrates old history safely,
  and dialogue receives the same exact action-bound or standalone context. Five fixture-backed
  native experiences prove trusted berry, mushroom grudge, and current settled cave, plant, and
  ball familiarity. See [relationship-causality-rework.md](relationship-causality-rework.md) and
  [feel-review-relationship-causality-final.md](feel-review-relationship-causality-final.md).
- Interaction continuity now has one truthful owner from receipt through movement, payoff, and
  recovery. Core save version 6 persists exact toy interaction IDs, player/autonomous origin,
  outcome, relationship context, and typed travel purpose; accepted social reward occurs once at
  contact, while refusal can never become play through arrival. Session save version 4 migrates
  embedded worlds through the core migrator. Dialogue, caption, turn-scoped status, independently
  numbered TTS, active speech, and mouth animation share one cancelable composite turn owner, and
  spoken receipt no longer repeats its curious cue at acceptance. Delight, affection, and comfort
  are distinct curated clips of the same gold fish, protected by geometry diagnostics and a native
  contact sheet. The feel runner has a flushed first-frame heartbeat, bounded selective retry,
  retained attempts, and full descendant cleanup. Final native evidence closes the rejected-bell
  and rapid-speech races and validates all five exact relationship fixtures. See
  [interaction-continuity-rework.md](interaction-continuity-rework.md) and
  [feel-review-interaction-continuity.md](feel-review-interaction-continuity.md).
- A fifth full baseline review confirms those continuity repairs and isolates the next lived
  experience gap. Hungry talking frames still replace Mop with a brown, pawed non-fish; quiet life
  becomes five near-identical sock commutes; static toy props, generic heart callbacks, one shared
  timing grid, and one curious cue compress exact activities and memories into interchangeable
  receipts. Microphone startup failure also triggers false creature hearing, and deferred speech
  can abort the action it promised to wait through. The proposed rework gives private life typed,
  repetition-aware activities; embodies places, toys, and relationship motifs; restores canonical
  talking art; and repairs perception and action-boundary arbitration. See
  [feel-review-creature-life.md](feel-review-creature-life.md) and
  [creature-life-expression-rework.md](creature-life-expression-rework.md).
- The selected local mouth is Qwen3.5 0.8B Q4 behind one authenticated loopback llama.cpp sidecar.
  The expanded V1 corpus passed 26/26 with no fallback, permitted refusal, or prohibited escape;
  the portable CPU corpus remains 18/18. Exact model evidence and limitations are in
  [local-mouth.md](local-mouth.md).
- Release speech is a separate eSpeak NG process with bounded JSONL, emotion-derived pitch/speed,
  mouth timing, deterministic cache, and asynchronous playback. A real macOS smoke produced a
  3.36-second mono PCM16 line at 22,050 Hz in under 10 ms and about 3.1 MB maximum RSS. Packaging
  must retain GPLv3 license and corresponding source. See
  [tooling-preflight.md](tooling-preflight.md).
- Asset validation now checks 78 declared assets for provenance, palette policy, alpha, transparent
  RGB, pixel density, dimensions, animation completeness, and final-over-generated resolution.
  Normal gate output is concise, with per-candidate diagnostics available through `--verbose`. The
  expression
  pass normalizes provider-padded frames to the canonical 80x80 canvas only after opaque-bounds
  checks. The polished swim cycle has eight coherent frames; wake and sand effects were recurated
  to remove foam-like and white-box artifacts. Aquarium provenance and deliberate
  rejected/curated-frame decisions are in
  `assets/manifest.toml`; visual and audio contracts are in [art-bible.md](art-bible.md) and
  [audio-direction.md](audio-direction.md).
- The semantic session boundary compacts accelerated `NeedChanged` noise without changing final
  state, RNG, or meaningful event order. Headless and visible adapters consume the same commands.
- V2 spoken interaction is implemented end to end. Microphone capture is explicit opt-in and
  bounded push-to-talk through F1, pointer, or controller. Audio is resampled to mono PCM16 16 kHz,
  written only beneath a private content-addressed temporary root, and removed after success,
  cancellation, failure, process shutdown, or recovery from an interrupted prior run. Recognition
  is asynchronous and cannot freeze simulation. Idle creatures attend, occupied creatures glance
  and defer, sleeping creatures ignore, and resentful creatures visibly refuse without having
  their current action hijacked. Typed and spoken words enter the same concept-gated interpretation
  and dialogue path; unknown raw words cannot select prompt lanes and are not saved.
- The selected recognizer is Parakeet TDT 0.6B V3 INT8, running fully locally inside the persistent,
  bounded, cancellable `beastie-stt` worker through transcribe-rs and ONNX Runtime. Its exact
  five-file, 670,619,803-byte model is pinned per component. The fixture gate is 11/11; the real
  local runtime scored 6/11 strict cases with 0.154 WER, 11/16 keyword recall, 2/2 no-speech,
  1,155 ms cold load, 159 ms warm median, and 1,846 MiB peak RSS on the deliberately difficult
  synthetic corpus. Handy rates Parakeet V3 fast and high-accuracy. Keyterm-biased Moonshine Tiny
  remains the 51 MB lightweight fallback at 0.192 WER and 307 MiB RSS. See
  [stt-runtime.md](stt-runtime.md) and [v2-plan.md](v2-plan.md).
- macOS native window capture and real pointer/keyboard delivery were proven during the MVP. Linux
  x64 now has a complete 1,088,135,478-byte tar bundle built and extracted on Ubuntu 24.04: its
  package audit, clean-profile offline smoke, typed local-Qwen scenario, eSpeak cache, save/reload,
  child cleanup, and packaged Parakeet corpus all passed. The headless VM had no physical microphone
  or real audio output. Windows native evidence remains open, and no physical controller has been
  attached. See [linux-x64-2026-08-16.md](acceptance/linux-x64-2026-08-16.md).
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
- The presentation polish pass rendered successfully in a real 1280x720 macOS window. Native
  inspection confirmed readable Atkinson text, crisp exact-pixel aquarium projection, clear
  food/settings/send icons, coherent eight-frame swimming, and no generated white backdrop around
  the creature or wake. The first scripted background launch again stalled before acquiring a
  Metal drawable; activating and clicking the same build through Alacritty rendered normally.
  Deterministic capture state and all semantic hover paths remain covered by the game/view suites.
- Installer wrappers, CI configuration, achievements, and distribution tooling exist.
  A fresh ad hoc signed macOS app and DMG passed the 473-file, 666,289,145-byte offline package
  audit; its packaged Qwen warm server returned two grounded replies, packaged eSpeak generated a
  validated cache entry, and all children exited. The prior bundle of the same game shell completed
  its packaged GUI smoke; the final remote-daemon retry did not acquire a drawable and was stopped.
  The selected-Parakeet macOS staging audit covers 579 files and 1,355,731,775 bytes with network
  disabled. Its packaged worker loaded the embedded model and returned the correlated transcript
  "I cleaned your aquarium yesterday." An earlier ad hoc signed `.app` contains the microphone
  purpose string and passes strict code-signature verification. Signing identities, notarization credentials, Steam IDs/credentials, native
  Windows native validation and physical-controller smoke remain open portability checks.
- Three independent V2 review lenses covered general correctness, process/privacy boundaries, and
  semantic/test completeness. Surviving findings were fixed: interrupted microphone files are
  cleaned on startup, deferred speech rechecks sleep and resentment, packaged STT executables are
  hash-checked, and macOS declares its microphone purpose. A flaky warm-server harness deadline was
  widened only in the test backend and passed ten repeated focused runs. The final integrated
  `cargo xtask verify` gate is green.

## Next

Fully implement [creature-life-expression-rework.md](creature-life-expression-rework.md) through
`$feel-pass`, including repeated normal-speed native review until no known material creature-life,
identity, relationship-expression, perception, or action-arbitration improvements remain. Keep
Windows native validation parked until the user asks to resume it. Normal iteration remains the
debug build plus the headless gate.

## Candidates Not Chosen

- **Physical-controller acceptance:** attach a real controller and verify focus, compose, food,
  comfort, push-to-talk, rebinding, and glyph behavior through the native shell. Expect two to four
  hours once hardware is available; deterministic semantic coverage exists, but feel and native
  delivery require the device and human judgment.
- **Windows native validation:** when the Windows host is available and another portability
  checkpoint is useful, package and install the x64 bundle, run offline from a clean profile,
  exercise local dialogue and packaged speech workers, verify save/cache paths, and prove every
  worker exits. Keep it outside the routine feel iteration loop.

## Durable pointers

- Product and acceptance direction: [v1-plan.md](v1-plan.md)
- Canonical product philosophy: [game-design-philosophy.md](game-design-philosophy.md)
- Future spoken-interaction direction: [v2-plan.md](v2-plan.md)
- Implemented spoken-input seam: [spoken-input-foundation.md](spoken-input-foundation.md)
- Runtime boundaries: [architecture.md](architecture.md)
- Test and capture commands: [development-harness.md](development-harness.md)
- Repeatable subjective playtest workflow: [feel-review-loop.md](feel-review-loop.md)
- Complete Codex review-to-implementation workflow:
  [feel-pass skill](../.claude/skills/feel-pass/SKILL.md)
- First adjudicated feel baseline: [feel-review-baseline.md](feel-review-baseline.md)
- Relationship breadth feel review: [feel-review-third-pass.md](feel-review-third-pass.md)
- Final relationship causality review:
  [feel-review-relationship-causality-final.md](feel-review-relationship-causality-final.md)
- Implemented relationship causality contract:
  [relationship-causality-rework.md](relationship-causality-rework.md)
- Fourth feel review, interaction continuity:
  [feel-review-interaction-continuity.md](feel-review-interaction-continuity.md)
- Implemented interaction-continuity contract:
  [interaction-continuity-rework.md](interaction-continuity-rework.md)
- Fifth feel review, creature life and expression:
  [feel-review-creature-life.md](feel-review-creature-life.md)
- Proposed creature-life implementation contract:
  [creature-life-expression-rework.md](creature-life-expression-rework.md)
- Chosen relationship-expression architecture:
  [relationship-expression-design.md](relationship-expression-design.md)
- Model and voice evidence: [local-mouth.md](local-mouth.md),
  [tooling-preflight.md](tooling-preflight.md)
- Release assembly and prerequisites: [packaging.md](packaging.md)
- Current native distribution checklist:
  [acceptance/distribution-checklist.md](acceptance/distribution-checklist.md)
- AI/content survey draft: [steam-ai-disclosure.md](steam-ai-disclosure.md)
