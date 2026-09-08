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

Simulation positions are normalized fixed-point values. Geometry, camera projection, UI layout units,
and viewport scaling belong to presentation. Saves are human-readable, versioned JSON and
migrate historical room-era state into stable aquarium objects without retaining runtime room
fields.

Travel that can produce a semantic payoff carries a typed purpose rather than relying on intention
or proximity. Toy interactions persist one nonzero identity, exact toy, player/autonomous origin,
outcome, phase, and any selected relationship context from receipt through contact and recovery.
Accepted play mutates social history and relationship state once at physical contact; rejection
uses a refusal-stare owner and cannot resolve through idle arrival. Core save version 6 validates
the interaction and travel owner in both directions and migrates ambiguous older toy travel
conservatively without manufacturing another payoff.

## Session and harness boundary

`GameSession` sits above core and below Bevy. It owns authoritative state, RNG, semantic commands,
dialogue-request construction, observations, and in-memory checkpoints. Accelerated advancement
coalesces idempotent need notifications while preserving final state, RNG, and every meaningful
phase, memory, food, and social event.

```text
JSONL scenario --> headless adapter --+
                                      +--> GameSession --> trace / state / dialogue request
JSONL scenario --> visible adapter ---+          |
                                                 +--> ScenePlan --> native PNG capture
real devices -------------------------------> beastie-game input mapping
```

The canonical scenarios are `fixtures/scenarios/aquarium-v1.jsonl` and
`aquarium-v1-visible.jsonl`. They cover cursor attention, physical food, action phases, naming,
dialogue, play, comfort, save/advance/load, inspection, and eight visible checkpoints. Older
berry-grudge, Stage 5, and room-shell scenarios remain regression and migration fixtures.
`spoken-input-foundation.jsonl` proves the same hearing lifecycle used by the microphone path
without requiring a microphone or model. See
[spoken-input-foundation.md](spoken-input-foundation.md).

Native feel capture records the framebuffer, semantic input, authoritative events, presentation
state, owned audio, and markers on one 60 fps timeline. A flushed heartbeat is emitted only after
framebuffer and state are both recorded. The runner classifies early exit separately from a live
zero-frame startup, retains per-attempt diagnostics, retries only the latter, and recursively
reaps nested worker process groups before promotion. See [feel-review-loop.md](feel-review-loop.md).

## Declarative presentation

`beastie-view` converts authoritative state and short-lived presentation state into a serializable
`ScenePlan` and audio commands. Creature pose, gaze, mood, action phase, speech mouth timing,
private-life recipes and relationship expressions remain typed, with exact semantic ownership.
Object instances retain authoritative IDs, positions, movement, carrying and responses. Expressions
and effects never infer game outcomes from model prose.

Bevy manages the native window, input, asset descriptions and continuously articulated scene.
A custom ordinary-GPU compute ray tracer renders every visible mark, including glyph geometry.
Voxel meshes omit interior faces and retain connected bevels. Cached local triangle hierarchies
stay unchanged as parts move; a small instance hierarchy tracks their transforms. Revised mesh
chunks upload independently through bounded arenas. The camera is fixed, orthographic and pitched
twelve degrees; its same projection drives ray generation and native pointer mapping.

The camera runs a dedicated compute-and-present schedule. Bevy's PBR lights, raster scene and
UI/text render plugins are removed; StandardMaterial is only an authored CPU material description.
A fullscreen transfer presents computed radiance through Bevy's ordinary screenshot-compatible
output attachment. There are no sprites, font atlases or alternate raster scene paths.

Lighting uses deterministic area-light visibility, environmental fill, bounded diffuse bounce and
rough reflected scene/environment light. Authored skin fill softens local occlusion. Two world
coverage samples and four UI samples share locally coherent illumination without sharing coverage
or albedo. No temporal history, denoiser, ray-query extension or dedicated RT hardware is required.
UI geometry is camera-aligned with controlled studio light and is excluded from world secondary
rays. See [raytraced-aquarium.md](raytraced-aquarium.md) for the production contract.

The authoritative aquarium remains a 2D interaction plane inside the 3D tank. Normalized position
maps through one shared transform with margins for creature volume. Presentation depth cannot
change contact or invent interactions. The shell resolves pointer rays against world volumes and
maps food drops back onto the interaction plane. UI hit regions preserve keyboard/controller
ordering and modal exclusion.
Body pick ellipsoids use the exact cached rendered transforms, including curvature and taper,
and compare world ray distance with the head and forgiving toy/food volumes. Plants and the
shelter use cached mesh triangles after a local bounding-box test; their empty gaps cannot
intercept a click. Scenery transforms are captured after propagation, matching the preceding
presented frame. Input resolves before simulation and animation publish the next frame.

The compact persistent control rail, frame-mounted identity plate, shallow contextual actions,
food-drop mode, settings, input bindings,
save recovery/reset confirmation, transcript controls, and naming remain declarative UI. Exact
need, trust and resentment values remain absent from the player summary. Panels and icons use
geometry, including shaped outline lettering. Speech identifies its speaker, occupies the opposite
side of the creature and provides explicitly labeled reactions. Reduced motion and other accessibility settings remain part of presentation state.
Text commands carry their resolved content bounds, semantic type role and disabled treatment;
the outline tessellator shapes, fits and geometrically clips glyphs inside those bounds. Settings pages are ephemeral view
state, while their values retain the existing versioned settings persistence. Startup recovery
notices are typed technical feedback and never gain creature speech or reaction controls.

The sprite pipeline and image catalog are removed. Runtime assets contain only sounds, the Atkinson
font and its license, validated by the versioned asset manifest. Generated geometry requires no
asset credentials or network. The font is embedded at build time; missing optional sounds/workers degrade without stopping
simulation. See [art-bible.md](art-bible.md), [audio-direction.md](audio-direction.md), and the
[voxel migration contract](voxel-migration.md).

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

The game shell maps independently numbered dialogue and TTS requests through a composite dialogue
owner containing shell generation and dialogue request ID. Caption, reveal, turn-scoped status,
queued TTS, active speech, and mouth animation retain that owner. Direct semantic actions
supersede the owner before applying their own response, so late worker completions are drained
without creating new presentation while already accepted canonical dialogue history remains true.

Durable save files, backups, settings, transcript export, wall-clock absence, worker lifecycle,
windowing, input devices, and audio playback stay in `beastie-game`. Platform packages may differ,
but core state, protocols, assets, and worker behavior remain identical across macOS, Windows, and
Linux.
