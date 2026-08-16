# State

Fast-moving work state and chosen next step. Durable detail lives in the linked documents.

Last updated: **2026-08-15** (V2 spoken interaction implemented; recognizer acceptance provisional).

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
- The selected local mouth is Qwen3.5 0.8B Q4 behind one authenticated loopback llama.cpp sidecar.
  The expanded V1 corpus passed 26/26 with no fallback, permitted refusal, or prohibited escape;
  the portable CPU corpus remains 18/18. Exact model evidence and limitations are in
  [local-mouth.md](local-mouth.md).
- Release speech is a separate eSpeak NG process with bounded JSONL, emotion-derived pitch/speed,
  mouth timing, deterministic cache, and asynchronous playback. A real macOS smoke produced a
  3.36-second mono PCM16 line at 22,050 Hz in under 10 ms and about 3.1 MB maximum RSS. Packaging
  must retain GPLv3 license and corresponding source. See
  [tooling-preflight.md](tooling-preflight.md).
- Asset validation now checks 77 declared assets for provenance, palette policy, alpha, transparent
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
- The provisional recognizer is Moonshine Voice 0.1.2 Tiny Streaming English behind a persistent,
  bounded, cancellable worker and native sidecar. The seven-file model is 51,441,771 bytes and is
  pinned per component. The checked-in fixture gate is 11/11; the actual runtime scored 4/11 with
  0.308 WER on the redistributable synthetic corpus, 122 ms cold, 57 ms warm median, and 304 MiB
  peak process-tree RSS. This proves plumbing and speed, not human accuracy. See
  [stt-runtime.md](stt-runtime.md) and [v2-plan.md](v2-plan.md).
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
- The presentation polish pass rendered successfully in a real 1280x720 macOS window. Native
  inspection confirmed readable Atkinson text, crisp exact-pixel aquarium projection, clear
  food/settings/send icons, coherent eight-frame swimming, and no generated white backdrop around
  the creature or wake. The first scripted background launch again stalled before acquiring a
  Metal drawable; activating and clicking the same build through Alacritty rendered normally.
  Deterministic capture state and all semantic hover paths remain covered by the game/view suites.
- Installer wrappers, CI configuration, Steam inputs, achievements, and store-asset tooling exist.
  A fresh ad hoc signed macOS app and DMG passed the 473-file, 666,289,145-byte offline package
  audit; its packaged Qwen warm server returned two grounded replies, packaged eSpeak generated a
  validated cache entry, and all children exited. The prior bundle of the same game shell completed
  its packaged GUI smoke; the final remote-daemon retry did not acquire a drawable and was stopped.
  The final V2 macOS staging audit covers 584 files and 759,039,770 bytes with network disabled.
  An ad hoc signed `.app` contains the microphone purpose string, passes strict code-signature
  verification, and its packaged STT worker loads the bundled engine and returns a correlated
  transcript. Signing identities, notarization credentials, Steam IDs/credentials, native
  Windows/Linux runs, physical-controller smoke, final store screenshots, and trailer remain
  release prerequisites.
- Three independent V2 review lenses covered general correctness, process/privacy boundaries, and
  semantic/test completeness. Surviving findings were fixed: interrupted microphone files are
  cleaned on startup, deferred speech rechecks sleep and resentment, packaged STT executables are
  hash-checked, and macOS declares its microphone purpose. A flaky warm-server harness deadline was
  widened only in the test backend and passed ten repeated focused runs. The final integrated
  `cargo xtask verify` gate is green.

## Next

Build the local real-human STT acceptance harness and use it to close the provisional recognizer
decision. Capture consented clips through the same microphone/resampling path as the game into an
ignored private corpus, describe speaker and acoustic conditions without identity data, extend
`cargo xtask stt eval` to score that external corpus, and produce one redacted report covering at
least three speakers, quiet speech, room/game noise, competing speech, proper names, aquarium
vocabulary, disfluency, profanity, silence, and cancellation.

Calibrate confidence only on a designated tuning split, then judge Moonshine on a held-out split
and rerun pinned whisper.cpp 1.9.2 tiny/base through the same clips if it misses the acceptance bar.
This one-to-two-day engineering batch plus human recording time beats open-microphone work because
recognition accuracy is now the load-bearing unknown; it decides whether the current 51 MB runtime
ships, needs narrow vocabulary adaptation, or is replaced. See [stt-runtime.md](stt-runtime.md).

## Candidates Not Chosen

- **Open-microphone or wake-word activation:** bounded push-to-talk is intentionally the V2
  activation model. Background capture would materially widen privacy, endpointing, and accidental
  activation risk before recognition quality is accepted. Expect two to four days plus sustained
  real-device and human judgment.
- **Prosody, laughter, and nonverbal acoustic cues:** useful future perception signals, but they
  must not infer player emotion as fact or bypass the shared language path. Expect one to three
  days for a fixture-backed signal boundary, with model/device evaluation as the main unknown.
- **Release-platform closure:** several days plus external credentials and Windows/Linux hardware.
  It remains necessary, but packaging a recognizer whose human accuracy is still unknown would
  lock in the wrong runtime. Required evidence still includes native Windows/Linux install and
  launch, a physical controller, signing, notarization, final store captures, trailer, and Steam
  credentials.

## Durable pointers

- Product and acceptance direction: [v1-plan.md](v1-plan.md)
- Canonical product philosophy: [game-design-philosophy.md](game-design-philosophy.md)
- Future spoken-interaction direction: [v2-plan.md](v2-plan.md)
- Implemented spoken-input seam: [spoken-input-foundation.md](spoken-input-foundation.md)
- Runtime boundaries: [architecture.md](architecture.md)
- Test and capture commands: [development-harness.md](development-harness.md)
- Model and voice evidence: [local-mouth.md](local-mouth.md),
  [tooling-preflight.md](tooling-preflight.md)
- Release assembly and prerequisites: [packaging.md](packaging.md)
- Current native distribution checklist:
  [acceptance/distribution-checklist.md](acceptance/distribution-checklist.md)
- AI/content survey draft: [steam-ai-disclosure.md](steam-ai-disclosure.md)
