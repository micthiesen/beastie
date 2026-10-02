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
Toy locomotion resolves to a deterministic surface envelope around the authoritative object
anchor. Gaze still targets the object itself. Contact holds through the act; a small bounded
retreat and constrained idle drift retain separation afterward. Ball impulses follow contact
direction. A released sock keeps its held anchor and falling velocity, with continuous depth
projection from that same velocity. Mesh tests keep the envelope consistent with authored art.
Contact selection retains the original surface when it clears other uncarried toys. A blocked
surface uses a bounded integer direction search around the selected toy, with each candidate's
asymmetric contact radius and neighboring quiet-rest clearance. Arrival rechecks the actual head
position. If the search finds no clear in-tank contact, the approach interrupts once without a
payoff or fabricated refusal, clears its matching activity/relationship and movement owners,
and returns to ordinary idle scheduling. A blocked familiar-place beat cannot keep reinstating
an impossible approach or hold the dialogue handoff indefinitely.
This is deterministic contact selection, not continuous-space obstacle routing.
Direct and autonomous play share the same physical response mutation, guarded by their own
contact phases. `ToyInteractionResponded` carries a direct interaction ID; `ToyObjectResponded`
retains private-life ownership. Their counters are separate namespaces. Direct recovery survives
a deferred talk handoff, releases any held sock on the next fixed tick, and does not replay the
response after saving or offline continuation. Presentation queues physical object effects
separately from creature expression, so a bell strike and delight can coexist without duplicate
audio or one effect delaying the other.
The renderer transfers observed carry/release changes from the actual preceding object transform
to the current target over 200 ms. This ephemeral interpolation changes no contact or reward time.
Effects use the displayed transform, as do mesh picking and selection. Loaded carried objects and
clock/title discontinuities start at their current pose without replaying a pickup; an early release
continues from the visible partial transfer. Reduced motion retains this essential continuity.
Timed affection releases only its own player-directed journey, never newer travel ownership.
Bottom foraging selects the nearest deterministic clear column at the existing bottom height,
using the same toy surface envelopes and quiet-rest clearance. Carried toys do not obstruct it.
Bottom uses the precise arrival tolerance of toy destinations, so an activity cannot enter its
stationary act while still short of the clear endpoint. Other destination rules, rewards and
authored act/recovery durations are unchanged; this is not general collision avoidance.

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
A custom ordinary-GPU renderer combines raster visibility with shared compute lighting for every
visible mark, including glyph geometry. Compute traversal remains the capability fallback.
Voxel meshes omit interior faces and retain connected bevels. Cached local triangle hierarchies
stay unchanged as parts move; a small instance hierarchy tracks their transforms. Revised mesh
chunks upload independently through bounded arenas. The camera is fixed, orthographic and pitched
twelve degrees; its same projection drives ray generation and native pointer mapping.

The camera runs a dedicated visibility, lighting and presentation schedule. Bevy's PBR lights, raster scene and
UI/text render plugins are removed; StandardMaterial is only an authored CPU material description.
A fullscreen transfer presents computed radiance through Bevy's ordinary screenshot-compatible
output attachment. There are no sprites, font atlases or alternate raster scene paths.

Lighting uses deterministic area-light visibility, environmental fill, bounded diffuse bounce and
rough reflected scene/environment light. Authored skin fill softens local occlusion. Two world
coverage samples and four UI samples share locally coherent illumination without sharing coverage
or albedo. No temporal history, denoiser, ray-query extension or dedicated RT hardware is required.
Six visibility directions refine mixed shadows to twelve on upward rough receivers, concentrating
soft-shadow detail on the bed. The subdued backdrop uses one continuous vertex-color gradient.
Persistent static lighting caches and static depth maps reuse unchanged scenery. Dynamic shadow
geometry uses an indexed, simplified position stream in one twelve-view atlas pass. Visible meshes
retain full detail. A bounded background job prepares shadow indices while original geometry draws;
camera-independent mesh retention avoids repeating work when objects return. Map density follows
viewport density through 4K, with original rays beyond reviewed sizes or adapter limits.
Explicitly occluded windows reduce host updates to 10 Hz while the game clock continues normally;
visible and automated runs retain their existing cadence.
UI geometry is camera-aligned with controlled studio light and is excluded from world secondary
rays. See [raytraced-aquarium.md](raytraced-aquarium.md) for the production contract.

The authoritative aquarium remains a 2D interaction plane inside the 3D tank. Normalized position
maps through one shared transform with margins for creature volume. Presentation depth cannot
change contact or invent interactions. The shell resolves pointer rays against world volumes and
maps food drops back onto the interaction plane. UI hit regions preserve keyboard/controller
ordering and modal exclusion.
Body pick ellipsoids use the exact cached rendered transforms, including curvature and taper,
and compare world ray distance with the head and forgiving food volumes. Toys, plants and the
shelter use cached mesh triangles after a local bounding-box test; their empty gaps cannot
intercept a click, and carried cloth or a swaying bell uses its actual silhouette. Object transforms
are captured after propagation, matching the preceding
presented frame. Input resolves before simulation and animation publish the next frame.

The articulated body follows a distance-sampled trail with fixed joint lengths. Joint targets
respect parent curvature while each achieved heading turns at a bounded rate. Tank-boundary
projection chooses a feasible wall-circle intersection nearest the previous heading, avoiding
an instantaneous switch between opposite floor or wall tangents. Settled hover preserves the
achieved curve. This solver changes presentation only, including the matching body pick volumes.

The compact persistent care dock, integrated identity summary, shallow contextual actions,
food-drop mode, settings, input bindings,
save recovery/reset confirmation, transcript controls, and naming remain declarative UI. Exact
need, trust and resentment values remain absent from the player summary. Panels and icons use
geometry, including shaped outline lettering. Speech identifies its speaker, occupies the opposite
side of the creature and provides explicitly labeled reactions. Reduced motion and other accessibility settings remain part of presentation state.
Text commands carry their resolved content bounds, semantic type role and disabled treatment.
The display-free `beastie-view::typography` module owns both bundled faces, fallback selection,
shaping, exact wrapping and pagination. View layout and the outline renderer share those metrics;
the renderer retains tessellation, clipping and outline caches. A 64-entry, 256 KiB layout cache
avoids repeated shaping during planning without changing results. Settings pages are ephemeral view
state, while their values retain the existing versioned settings persistence. Startup recovery
notices are typed technical feedback and never gain creature speech or reaction controls.

The sprite pipeline and image catalog are removed. Runtime assets contain sounds, Atkinson
interface lettering and a monochrome Noto Emoji fallback with their licenses, validated by the
versioned asset manifest. Both fonts are embedded. Complete emoji graphemes select a fallback
face and adjacent same-face runs shape together; measurement and cached outlines use each face's
own metrics. Unsupported scripts retain an explicit missing glyph. Generated geometry requires no
asset credentials or network; missing optional sounds/workers degrade without stopping
simulation. See [art-bible.md](art-bible.md), [audio-direction.md](audio-direction.md), and the
[voxel migration contract](voxel-migration.md).

The UI uses retained rounded vector faces/rims and utility contours alongside real voxel toy/food
miniatures. A shared semantic depth stride bounds miniature depth and keeps higher panels above
all lower primitive types. Disabled pointer blockers cover opaque panel backgrounds; covered
controls are removed from navigation while each panel's own controls remain active. Speech sits
below modal surfaces. Inspection is a live qualitative projection of the selected target, never
a simulation mutation. Rename and controller entry have dedicated fields with canonical bounds.
The dock names accepted food/toy attention only when current gaze and authoritative interaction
ownership agree. This can acknowledge a new target while an earlier payoff recovers, without
replacing its pose/effect or claiming movement, contact or enjoyment prematurely.
Editable values preserve the current draft's end using measured, grapheme-safe single-line
fitting; labels retain their normal wrapped layout. Native and controller deletion remove a
complete grapheme while canonical name and message limits continue to count Unicode scalars.
Focused native message/name fields support primary-modifier Select All and Paste. A retained
selection highlight or steady end caret follows measured visible text. Ordered raw window events
preserve modifier chords even when their keys arrive and release within one frame. Asynchronous
clipboard results carry field/revision ownership and cannot modify a submitted or displaced draft.
Long dialogue uses compact lossless continuation pages opposite the creature, with a separate
navigation row and complete reaction controls. Settings reserves the clear left column; other
obstructing modals hide the complete group and retain its reading deadline without pausing the
simulation or audio. Panels size to the complete page, staying stable during reveal. Reveal advances
by complete Unicode graphemes; unread pages remain available and the final page receives its own
reading interval. Page boundaries use five exactly measured lines at Large size, so changing
text size does not lose the reader's place. Pagination is presentation state and does not alter
the accepted dialogue. Notices choose clear space around interaction panels; entering reset
confirmation dismisses the prior transient receipt while preserving new error reporting.
Scripted saves, settings and transcripts always use capture-local disposable storage, including
explicit export paths. Reset preservation and recovery validate a full prior generation before
promotion and retain failed recovery inputs for retry.

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
Mouth phases sample the exact active audio player's consumed playback position, including the
TTS request ID; they do not use the one-second simulation clock. Stopped, completed, missing or
displaced audio closes the mouth. Reply arrival preserves another open page and its focus;
without a visible caption, compose retains a usable text focus rather than hidden reactions.

Durable save files, backups, settings, transcript export, wall-clock absence, worker lifecycle,
windowing, input devices, and audio playback stay in `beastie-game`. Platform packages may differ,
but core state, protocols, assets, and worker behavior remain identical across macOS, Windows, and
Linux.
