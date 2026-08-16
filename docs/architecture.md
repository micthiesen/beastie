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
`spoken-input-foundation.jsonl` proves the same hearing lifecycle used by the microphone path
without requiring a microphone or model. See
[spoken-input-foundation.md](spoken-input-foundation.md).

## Declarative presentation

`beastie-view` converts authoritative state and short-lived presentation state into serializable
render and audio plans. It maps normalized coordinates to whole logical pixels; selects authored
full-body mood, action, speech, and reaction sprites; queues important feedback with direct-reaction
preemption; and emits semantic hit regions shared by pointer, keyboard, and controller.
Accepted eating, food rejection, toy refusal, noticing, and comfort are distinct projections of
typed simulation events rather than visual guesses derived from model text. Cue-relative playback
starts each reaction at frame zero and holds the final frame. Dialogue supersedes stale ambient
punctuation, while direct refusal and comfort remain immediate.

The game shell projects 320x180 plan coordinates directly onto a 640x360 presentation image. World
pixels remain exact 2x blocks, while text is rasterized natively at 16 presentation pixels and
silhouette focus outlines may use one presentation pixel. The finished image is nearest-scaled at
integer sizes into a fixed 16:9 window or letterboxed fullscreen. The selected background is
cropped at 1:1 before projection to remove its generated surface opening. Shipped hero acting and
effects are sprite art; the renderer keeps only a simple missing-asset creature and geometry
fallback so a corrupt or absent optional file never blocks play.

The persistent compose bar, shallow contextual actions, food-drop mode, settings, input bindings,
save recovery/reset confirmation, transcript controls, and naming are declarative UI. Exact need,
trust, and resentment values never appear in the player summary. The compose deck is icon-led and
always focused, while help is contextual to hover or controller focus rather than a permanent line
of shortcut prose. Modal chrome, text, authored icons, status, and focus rings occupy explicit
layer bands. Speech panels choose the side opposite the creature.

Runtime sprite IDs resolve through the checked asset manifest, preferring `assets/final` over
`assets/generated`. The shell decodes image and alpha data once at startup. Sprite-linked hit
regions sample that alpha mask and fall back to their semantic rectangle when art is absent.
Missing image, sound, speech, model, font, or audio-device paths degrade without blocking
deterministic play. Atkinson Hyperlegible Next loads once beside the sprite catalog, with a
code-native bitmap fallback. See
[art-bible.md](art-bible.md) and [audio-direction.md](audio-direction.md).

## Local mouth and voice

`beastie-protocol` is the narrow trust boundary. Dialogue requests expose curated facts, bounded
recent turns, selected memories and beliefs, authoritative emotion/action context, and explicit
content-lane permissions. Replies contain bounded text and allow-listed presentation metadata.
They cannot mutate simulation state.

`beastie-ai-worker` is a supervised JSONL child process. Dialogue, TTS, and STT use bounded,
cancellable worker transports while retaining separate protocol validation and fallback policy.
The STT worker validates a private content-addressed WAV before running the selected Parakeet model
inside its persistent contained process. The optional Moonshine fallback retains its native
sidecar boundary. The release dialogue adapter keeps one authenticated loopback
llama.cpp sidecar warm. eSpeak NG remains a separate offline executable,
receives validated text plus authoritative voice settings, and returns cached WAV metadata and
mouth timing. Fixture dialogue and silent speech fallbacks remain permanent test and failure modes.

Durable save files, backups, settings, transcript export, wall-clock absence, worker lifecycle,
windowing, input devices, and audio playback stay in `beastie-game`. Platform packages may differ,
but core state, protocols, assets, and worker behavior remain identical across macOS, Windows, and
Linux.
