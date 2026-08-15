# V1 build plan

The MVP established the deterministic creature, offline mouth, save boundary, room-era shell, and
Mac-led package proof. V1 replaces that presentation and interaction model with the aquarium in
[v1-plan.md](v1-plan.md). The MVP specification remains historical context, not the current scene.

## Preconditions and constraints

- The simulation is authoritative; model, TTS, and presentation cannot establish facts.
- Runtime play is offline and remains playable when AI, speech, audio, or optional art fails.
- Core tests and semantic scenarios are portable and display-free.
- macOS, Windows, and Linux remain release targets. macOS-led implementation does not count as
  native Windows/Linux acceptance.
- Codex is the only required coding-agent environment. Do not add Claude/rulesync compatibility.

## Implemented V1 slices

### 1. Aquarium truth and interaction

Complete. Room anchors were replaced by fixed-point two-dimensional position and velocity, facing,
gaze, depth, steering, semantic destinations, action phases, and authoritative aquarium objects.
Food is dropped at a real water coordinate, moves by typed buoyancy, has bounded population and
lifetime, and can be noticed, approached, consumed, rejected, or settled. Historical saves migrate
without keeping room runtime fields.

### 2. Expression and movement

Complete. The selected 80x80 creature art renders at exact 2x nearest scale. Hover, swim,
eat/recoil, sleep, and play combine with whole-pixel buoyancy, six side/player-facing full-body mood
sets, six three-shape speech sets, a bespoke affection loop, and authored wake, sand, attention,
heart, mouth-particle, and sleep effects. Direct reactions preempt stale presentation cues. The
code-native creature is a missing-asset safety net, not a layer over shipped art.

### 3. Persistent interaction and accessibility

Complete. Text entry remains visible and focused outside modal choices. Pointer, keyboard, and
controller operate stable semantic regions. Food-drop, creature context, naming, settings, input
bindings, on-screen keyboard, broad creature summary, save recovery/reset confirmation, and local
transcript controls are declarative and display-free testable. Reduced motion, flashes, shake, text
size/speed, window scale, fullscreen, effect/speech volume, voice, and pixel-grid controls exist.

### 4. Creature depth and local expression

Complete for the V1 target. Development, routines, favorite locations, belief contradiction,
object naming, absence/reunion, initiated behavior, bounded recent-turn context, idiolect, authored
fallback variety, authoritative voice settings, and transcript records extend the MVP creature.
Qwen3.5 0.8B Q4 remains the selected warm local model; eSpeak NG remains the separate release voice.

### 5. Content and release tooling

Implemented, pending final gate and commit. Aquarium art and audio have checked provenance and stricter
pixel/WAV validation. Platform staging, macOS app/DMG, Windows installer configuration, Linux
AppImage/tar wrapper, CI matrix, Steam depot/input/achievement files, and store-asset tooling exist.
They consume an offline package and do not download runtime dependencies.

## Final acceptance

The integrated tree has passed the visible aquarium, real Qwen, real eSpeak, final staged macOS
package integrity and worker checks, plus a packaged GUI smoke of the immediately preceding game
bundle. Three independent review lenses and the final integrated diff review are complete, all
surviving findings were fixed, and `cargo xtask verify` passes on the final tree. Performed native
and release evidence is recorded in the dated acceptance material.

Native Windows/Linux install and launch, physical-controller input, Apple signing/notarization,
Windows signing, Steam credentials/publication, final native screenshots, and trailer require the
corresponding host hardware or private release account. CI definitions and artifacts are not a
substitute for those checks.

## Working rules

- Keep `cargo xtask verify` independent of display, model, GPU, audio, network, and generation
  credentials.
- For visible shell changes, run the aquarium visible fixture with fake AI before real-runtime work.
- Select models, voices, and generated art from repeatable evaluation, not one attractive sample.
- Commit prompts, scenarios, traces, reports, provenance, licensing, and platform gotchas.
- Use [STATE.md](STATE.md) for the current next step and this document for the durable V1 sequence.
