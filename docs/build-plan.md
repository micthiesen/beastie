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

Exact measurements and provisional local-mouth decisions are in
[local-mouth.md](local-mouth.md). The preflight deliberately did not perform native
Windows or Linux verification. Portable architecture and headless tests remain mandatory, but the
initial full build is allowed to progress on macOS before the later native-platform acceptance runs.

Codex is the only coding-agent environment the MVP needs to support. Do not spend MVP time adding
Claude Code compatibility, rulesync plumbing, or duplicated agent instructions.

## Ordered build flow

### 0. Use the proven host feedback loop

Native launch, window discovery, screenshot capture, pointer movement, and clicking are already
proven on the development Mac. Begin product work immediately. Add semantic commands, structured
traces, and direct framebuffer capture incrementally when each real interaction makes them useful;
do not build a broad harness upfront.

### 1. Complete the authoritative berry-memory spine (complete)

Build the acceptance scenario through pure logic first: preference formation, concrete event
memory, candidate selection, sleep, save/reload, later recall, reaction-driven provocation, and a
nonverbal expression of dislike. Add multi-day invariants and deterministic replay. Add only the
smallest headless controls needed to replay this slice.

### 2. Make the enclosure playable with fixture AI (complete)

Complete the fixed dollhouse room, creature movement/intention projection, five contextual verbs,
mouse and keyboard/controller focus, dialogue UI, contextual reactions, save/continue, and the
three-day progression arc. Keep fixture AI as a permanent backend.

### 3. Establish canonical art and sound presentation (complete)

Create a tiny art bible before bulk generation: palette plus creature, furniture, and UI anchors.
Use PixelLab references for later assets, validate dimensions/alpha/provenance, and promote only
deliberately selected results. Add restrained animation, room lighting/window variation, UI sounds,
and authored nonverbal creature noises.

### 4. Integrate the replaceable local mouth (provisional boundary complete)

The checked-in corpus provisionally selects Qwen3.5 0.8B Q4 behind the bounded worker interface.
The game keeps the outer worker alive, while the current evaluation adapter still reloads
`llama-cli` for every attempt. Native Kitten nano speech and its cache are proven without Python,
but the optional sherpa-onnx path is blocked from release by its GPLv3 espeak-ng dependency and is
wired into gameplay only behind a non-default experimental feature. See
[local-mouth.md](local-mouth.md).

### 5. Harden and finish the full vertical slice (next)

Exercise the complete three-day acceptance arc in headless, visible-fixture, and full-local loops.
Replace the cold-per-attempt evaluation runtime with a warm packaging candidate and wire a
license-compatible native voice. Verify graceful AI/TTS failure, content boundaries, save
migrations, offline non-lethal progression, asset/model provenance, CPU-only inference, native
Windows/Linux acceptance, packaging inputs, and the MVP definition of done.

## Working rules

- `cargo xtask verify` stays green and independent of display, models, audio, network, and PixelLab.
- For visible shell changes, also run `cargo xtask dev --fake-ai` and capture reviewable evidence.
- Do not let model output establish facts or directly mutate the simulation.
- Do not select models, voices, or generated art from a single attractive smoke result.
- Commit durable prompts, scenarios, traces, reports, provenance, and gotchas to the repository.

If a fresh session needs one instruction, it is: read `docs/STATE.md`, execute its **Next** item,
and use this flow to continue toward the complete MVP.
