# Architecture

Beastie's central rule is enforced by dependency boundaries: the simulation creates truth, while
the model and presentation create expression.

```text
player input --> beastie-session --> beastie-core --> GameEvent --> beastie-view --> beastie-game
                         |                |
                         |                +--> curated facts --> beastie-protocol
                         |                                      --> beastie-ai-worker
                         +--> versioned saves / observations
```

## Authoritative aquarium

`beastie-core` is deterministic and sans-I/O. It owns needs, traits, relationships, preferences,
memories, beliefs, development, routines, aquarium objects, physical food, and the creature's
fixed-point two-dimensional position and velocity. Facing, gaze, semantic destinations, steering,
and anticipation/action/recovery phases make movement legible without importing a physics or
rendering engine. Time, randomness, cursor position, and player actions are injected.

Simulation positions are normalized fixed-point values. Pixel coordinates, sprite sizes, source
crops, and viewport scaling belong to presentation. Saves are human-readable, versioned JSON and
migrate historical room-era state into stable aquarium objects without retaining runtime room
fields.

## Session and harness boundary

`GameSession` sits above core and below ggez. It owns authoritative state, RNG, semantic commands,
dialogue-request construction, observations, and in-memory checkpoints. Accelerated advancement
coalesces idempotent need notifications while preserving final state, RNG, and every meaningful
phase, memory, food, and social event.

```text
JSONL scenario --> headless adapter --+
                                      +--> GameSession --> trace / state / dialogue request
JSONL scenario --> visible adapter ---+          |
                                                 +--> RenderPlan --> logical PNG capture
real devices -------------------------------> beastie-game input mapping
```

The canonical scenarios are `fixtures/scenarios/aquarium-v1.jsonl` and
`aquarium-v1-visible.jsonl`. They cover cursor attention, physical food, action phases, naming,
dialogue, play, comfort, save/advance/load, inspection, and eight visible checkpoints. Older
berry-grudge, Stage 5, and room-shell scenarios remain regression and migration fixtures.

## Declarative presentation

`beastie-view` converts authoritative state and short-lived presentation state into serializable
render and audio plans. It maps normalized coordinates to whole logical pixels, selects the shipped
hover/swim/eat-recoil animation sets, layers deterministic gaze and expression geometry, queues
important reactions, and emits semantic hit regions shared by pointer, keyboard, and controller.

The game shell executes plans on a 320x180 logical framebuffer with nearest sampling, whole-pixel
sprite placement, integer sprite scaling, and integer viewport scaling. The selected background is
cropped at 1:1 to remove its generated surface opening. Optional PNG overlays may enrich the scene,
but code-native face, mouth, gaze, bubbles, caustics, particles, and feedback remain visible when an
optional asset is missing.

The persistent compose bar, shallow contextual actions, food-drop mode, settings, input bindings,
save recovery/reset confirmation, transcript controls, and naming are declarative UI. Exact need,
trust, and resentment values never appear in the player summary.

Runtime sprite IDs resolve through the checked asset manifest, preferring `assets/final` over
`assets/generated`. The shell decodes assets once at startup. Missing image, sound, speech, model,
or audio-device paths degrade without blocking deterministic play. See [art-bible.md](art-bible.md)
and [audio-direction.md](audio-direction.md).

## Local mouth and voice

`beastie-protocol` is the narrow trust boundary. Dialogue requests expose curated facts, bounded
recent turns, selected memories and beliefs, authoritative emotion/action context, and explicit
content-lane permissions. Replies contain bounded text and allow-listed presentation metadata.
They cannot mutate simulation state.

`beastie-ai-worker` is a supervised JSONL child process. The release dialogue adapter keeps one
authenticated loopback llama.cpp sidecar warm. eSpeak NG remains a separate offline executable,
receives validated text plus authoritative voice settings, and returns cached WAV metadata and
mouth timing. Fixture dialogue and silent speech fallbacks remain permanent test and failure modes.

Durable save files, backups, settings, transcript export, wall-clock absence, worker lifecycle,
windowing, input devices, and audio playback stay in `beastie-game`. Platform packages may differ,
but core state, protocols, assets, and worker behavior remain identical across macOS, Windows, and
Linux.
