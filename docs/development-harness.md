# Development harness

The game should become controllable and inspectable by Codex without relying on coordinate clicks
for every interaction. Grow one versioned semantic command protocol alongside the MVP slices rather
than building the entire harness before gameplay. Coordinate-driven automation remains useful for
host smoke tests, coordinate mapping, and actual device input.

## Host control derisk

Completed on the development Mac on 2026-08-15:

- launched `cargo xtask dev --fake-ai` as a native ggez window;
- found and focused the Beastie window through macOS Accessibility and yabai;
- captured the real rendered window by window ID to a valid RGBA PNG;
- inspected the captured fixture dialogue and placeholder creature;
- submitted a real Talk interaction through the native keyboard path and observed its durable
  dialogue request ID advance.

Alacritty required Screen Recording/Accessibility approval before capture and focus worked. Launching
the Metal window directly from the automation backend can produce a blank or occluded window, while
launching through Alacritty renders correctly. The current interactive room accepts pointer input in
code and its logical coordinate mapping is unit-tested, but macOS host-driven clicks appeared to fall
through to the Alacritty/loginwindow stack even after raising the Beastie window and temporarily
stopping yabai. Treat real pointer delivery as unverified, not as a gameplay failure. Physical
controller input is also still unverified.

## Shared session boundary

Orchestration that sits above `beastie-core` and below ggez lives in reusable `GameSession`. It owns
authoritative state, RNG, semantic commands, and dialogue-request construction. The game shell owns
wall-clock timestamps, durable file replacement, worker lifecycle, and plan execution. Neither
harness adapter gets a second implementation of game rules.

Commands are high-level player or test intentions, not renderer events:

```jsonl
{"version":1,"command":"feed","food":"berry"}
{"version":1,"command":"advance","minutes":5}
{"version":1,"command":"talk","text":"Did you like it?"}
{"version":1,"command":"react","reaction":"laugh"}
{"version":1,"command":"inspect"}
{"version":1,"command":"capture","name":"after-second-berry"}
```

The current command enum covers the primary interactions and deterministic control used by the
implemented scenarios:

- player actions: feed, play, comfort, tidy, talk, and contextual speech reaction;
- deterministic control: tick or advance game time and create/load an in-memory checkpoint;
- observation: inspect authoritative state and emitted events.

Visible scenario files add named framebuffer capture as a shell-level step. Waiting for pending
AI/TTS, quitting, and unified trace artifacts should be added only when their corresponding runtime
features need them.

Do not expose arbitrary state mutation as a normal command. Purpose-built fixture setup may load a
versioned save, but gameplay scenarios should reach states through ordinary actions whenever
practical.

Every accepted command emits a versioned observation. It should contain enough evidence to debug a
failure without opening the game:

```json
{
  "version": 1,
  "sequence": 5,
  "game_time": "day-1 09:05",
  "accepted": true,
  "events": ["creature_refused_food"],
  "speech": "red shit again.",
  "referenced_memories": ["memory-12"],
  "state_digest": "...",
  "render_plan_digest": "...",
  "pending": []
}
```

Full state and full plans belong in explicit `inspect` responses or trace artifacts rather than in
every small acknowledgement. Protocol traffic is untrusted input: cap line size and string length,
reject unknown versions/commands/fields where appropriate, and return structured errors without
crashing the session.

## Headless adapter

The primary interaction laboratory is:

```bash
cargo xtask play --seed 42 --fake-ai
```

It reads JSONL from standard input and writes observations to standard output. It must not require
a window, display server, GPU, audio device, local model, PixelLab credential, or network. A scenario
file can drive the same adapter non-interactively:

```bash
cargo xtask play --scenario fixtures/scenarios/berry-grudge.jsonl --fake-ai
```

Scenario transcripts are deterministic fixtures. They should be usable for focused assertions and
for long behavioral runs that compress hours or days of play into seconds.

## Visible adapter

The real game shell accepts the same scenario commands at deterministic update boundaries:

```bash
cargo xtask dev --fake-ai \
  --script fixtures/scenarios/room-shell.jsonl \
  --capture-dir target/captures/room-shell
```

`capture` saves the 320×180 logical framebuffer directly as PNG before display scaling. This is
stable evidence for visual review and avoids OS screenshot permissions, window overlap, and fragile
screen coordinates. Captures supplement `RenderPlan` snapshots; neither replaces the other.

Scripted visible runs implicitly start from seed 42, do not load a prior user save, and do not write
the persistent save. This makes the documented command deterministic and safe to rerun.

Start with scenario files, not a socket. They are deterministic, reviewable, and sufficient for an
agent to edit, run, and inspect the game. Add an opt-in localhost JSONL control socket only if real
development shows that live poking would materially shorten the loop. A development socket must
never be enabled in release builds.

## Real input coverage

Semantic commands intentionally bypass platform input translation, so a thin set of host-driven
tests must still exercise real input:

- mouse selection and logical-coordinate mapping through nearest-neighbor scaling/letterboxing;
- keyboard focus navigation, contextual actions, and Talk text entry;
- controller focus navigation, action selection, and the modal on-screen keyboard;
- window launch, resize behavior, and one real screenshot/capture comparison.

Coordinate clicks are correct for these tests because pointer mapping is the behavior under test.
They are not the default way to test creature interactions.

## Incremental harness acceptance

This is an MVP outcome, not a prerequisite to build in one upfront block. Add commands when the
corresponding real interaction is implemented. By the end of the relevant slices, this loop passes:

1. Feed a berry and advance time through the headless adapter.
2. Run the identical scenario through the visible ggez game with fixture AI.
3. Produce and inspect a framebuffer PNG.
4. Correlate authoritative state, emitted events, dialogue, and `RenderPlan` in one trace.
5. Launch the real window and verify at least one genuine pointer or keyboard path. Keyboard Talk is
   proven on macOS; pointer delivery and physical controller input remain outstanding host checks.

Keep completed scenarios as permanent fixtures and include their headless checks in
`cargo xtask verify`. Visible capture and host-input smoke tests remain explicit host checks when a
display is unavailable. Do not delay useful gameplay work to implement commands for features that
do not exist yet.
