# MVP build plan

The next autonomous build targets the full MVP described in [mvp-spec.md](mvp-spec.md), not a throwaway
prototype or only the deterministic core. Work in vertical, testable slices and keep the game
playable with fixture AI throughout.

## Preconditions already satisfied

- macOS is a first-class target and the current development host is known.
- `llama.cpp` can run local sub-billion-parameter candidates on the development Mac.
- KittenTTS can synthesize suitable local speech substantially faster than real time.
- PixelLab MCP authentication and asynchronous transparent-PNG generation work.
- The workspace, pure-core boundary, JSONL AI protocol, fixture worker, `RenderPlan`, save format,
  verification gate, and `/next`/`/wrap` continuity system exist.

Exact measurements and remaining selection work are in
[tooling-preflight.md](tooling-preflight.md). The preflight deliberately did not perform native
Windows or Linux verification. Portable architecture and headless tests remain mandatory, but the
initial full build is allowed to progress on macOS before the later native-platform acceptance runs.

Codex is the only coding-agent environment the MVP needs to support. Do not spend MVP time adding
Claude Code compatibility, rulesync plumbing, or duplicated agent instructions.

## Ordered build flow

### 0. Prove the agent feedback loop

Implement and pass the five-step gate in [development-harness.md](development-harness.md). This is
the first task because every later slice benefits from semantic control, structured traces, and
direct framebuffer capture.

### 1. Complete the authoritative berry-memory spine

Build the acceptance scenario through pure logic first: preference formation, concrete event
memory, candidate selection, sleep, save/reload, later recall, reaction-driven provocation, and a
nonverbal expression of dislike. Add multi-day invariants and deterministic replay.

### 2. Make the enclosure playable with fixture AI

Complete the fixed dollhouse room, creature movement/intention projection, five contextual verbs,
mouse and keyboard/controller focus, dialogue UI, contextual reactions, save/continue, and the
three-day progression arc. Keep fixture AI as a permanent backend.

### 3. Establish canonical art and sound presentation

Create a tiny art bible before bulk generation: palette plus creature, furniture, and UI anchors.
Use PixelLab references for later assets, validate dimensions/alpha/provenance, and promote only
deliberately selected results. Add restrained animation, room lighting/window variation, UI sounds,
and authored nonverbal creature noises.

### 4. Integrate the replaceable local mouth

Build the checked-in dialogue eval corpus before selecting a model. Benchmark official Qwen family
candidates for grounding, valid protocol output, latency, permitted profanity/innuendo refusal,
prohibited-output escape, and generic-assistant voice. Integrate the winner behind the worker
interface, then integrate a pinned sherpa-onnx TTS path after comparing KittenTTS and Kokoro with
actual Beastie lines. The shipped game contains no Python.

### 5. Harden and finish the full vertical slice

Exercise the complete three-day acceptance arc in headless, visible-fixture, and full-local loops.
Verify graceful AI/TTS failure, content boundaries, save migrations, offline non-lethal progression,
asset/model provenance, CPU-only inference, packaging inputs, and the MVP definition of done.

## Working rules

- `cargo xtask verify` stays green and independent of display, models, audio, network, and PixelLab.
- For visible shell changes, also run `cargo xtask dev --fake-ai` and capture reviewable evidence.
- Do not let model output establish facts or directly mutate the simulation.
- Do not select models, voices, or generated art from a single attractive smoke result.
- Commit durable prompts, scenarios, traces, reports, provenance, and gotchas to the repository.

If a fresh session needs one instruction, it is: read `docs/STATE.md`, execute its **Next** item,
and use this flow to continue toward the complete MVP.
