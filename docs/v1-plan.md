# Beastie V1 plan

The MVP proved the deterministic creature, local-mouth boundary, persistence, presentation shell,
and offline package. V1 should now make the creature feel alive.

The primary product direction is **The Aquarium & Expression Pass**: replace the enclosed dollhouse
room with a full-screen slice of water, let the creature move naturally through most of the frame,
make its face and attention readable, and reduce interaction friction with a persistent bottom
interface.

This is not an art reskin. The aquarium changes movement, interaction, animation, presentation, and
the relationship between the creature and player. The authoritative simulation and local-model
boundary remain intact.

## Product outcome

V1 succeeds when:

> The creature swims through a living aquarium, visibly notices and reacts to the player and
> objects, can be fed by dropping food into the water, expresses its internal state through face
> and motion, and is controlled through a crisp persistent interface.

Naturalness and expression outrank adding a large number of disconnected features. A creature that
looks, turns, anticipates, reacts, and recovers is more valuable than ten new menu actions.

## Product invariants

- The simulation remains the brain and the local model remains the mouth.
- Every important motion has a legible cause, even when that cause is curiosity or boredom.
- Conversation stays scarce. Beastie does not become an endless chatbot with a fish attached.
- Internal state is communicated primarily through behavior, expression, and speech, not exact bars.
- Neglect never causes death or irreversible ruin.
- The player cannot edit or reroll away an inconvenient personality.
- Runtime play remains offline and model, TTS, or audio failure never stops the game.
- One highly expressive creature remains the focus.

## Aquarium composition

The frame shows open water rather than the top and sides of a literal tank. Only the lower portion
of the aquarium is visible as a physical boundary.

### Visual layers

1. A deep-water backdrop with a restrained lighting cycle and water color variation.
2. Slow caustics, suspended particles, and distant bubbles that do not obscure the creature.
3. The creature and active objects in the main swimming volume.
4. Sand, plants, a cave, toys, food, and movable clutter along the bottom.
5. Glass-adjacent effects and the persistent bottom interface.

The environment should remain cozy, grotty, and slightly strange rather than becoming a clean
aquarium simulator. The bottom can accumulate buried food, shifted objects, algae, and evidence of
the creature's habits.

### Semantic world model

Core should preserve deterministic, meaningful movement rather than adopting general-purpose
physics. Replace room spots with aquarium affordances and bounded continuous movement:

- a deterministic two-dimensional position and velocity;
- facing, gaze target, and optional depth lane;
- semantic destinations such as cave, plant, bottom, player, toy, and a specific food item;
- bounded steering modes such as hover, approach, flee, orbit, inspect, and settle;
- deterministic object buoyancy and sinking behavior.

Pixel coordinates remain presentation-owned. Core may use normalized aquarium coordinates or typed
semantic positions, but it must not depend on ggez, the display, or art dimensions.

## Movement that reads as intentional

The current MVP interpolates between a small number of room anchors. In the aquarium, locomotion
needs a visible behavioral grammar.

### Locomotion vocabulary

- hover, drift, bob, coast, brake, and settle;
- dart, turn, bank, rise, dive, and circle;
- follow, avoid, inspect, nudge, rub, and face away;
- sleep in a cave, plant, or learned favorite location;
- disturb sand, leave bubbles, and create small wake effects.

Randomness may choose among meaningful behaviors, but it must not create arbitrary floating. Idle
movement should reflect temperament, mood, energy, curiosity, current gaze target, and recent events.

### Animation phrasing

Actions should pass through anticipation, action, and recovery rather than snapping from intention
to result. Feeding is the reference sequence:

```text
notice food
-> stop drifting
-> turn eyes toward it
-> rotate body
-> approach
-> slow down
-> inspect
-> eat or reject
-> show a visible emotional recovery
```

Equivalent sequences should exist for playing, sleeping, seeking comfort, greeting the player,
showing affection, hiding, refusing, and acting out.

### Continuous micro-behaviors

Between major actions the creature should:

- track food, toys, bubbles, and the cursor with its eyes;
- blink, yawn, twitch, flare, scratch, or blow bubbles;
- change hover height and body angle with mood;
- investigate recently moved or introduced objects;
- revisit favorite areas without returning to a universal center.

The cursor acts as a lightweight representation of the player. Trust can make the creature follow
it. Loneliness can make it wait nearby. Resentment can make it turn away, retreat, or watch from a
distance. Repeated glass tapping may annoy it.

## Feeding as a physical interaction

Feeding should borrow the tactile pleasure of games such as Insaniquarium without making every
ordinary click feed the creature.

1. The player selects a food from the persistent action area.
2. The cursor enters a clearly marked food-drop mode.
3. Clicking open water creates one authoritative food item at that position.
4. The item floats, falls, or drifts according to its typed behavior.
5. The creature notices, ignores, approaches, eats, or rejects it according to need and preference.

A hated food can be spat out, slapped away, buried, guarded, or stared at suspiciously. These are
simulation-backed consequences, not model-authored facts.

Food objects need bounded lifetimes and population limits. Old food may settle into the sand or be
cleaned up without creating punishment spirals.

## Expression system

The creature should occupy enough of the 320x180 frame for its eyes, mouth, body angle, and immediate
emotion to be readable. A target height around 64 to 80 logical pixels is a useful starting point,
subject to visual testing.

### Hybrid animation rig

Use composable presentation where it improves breadth without sacrificing pixel-art coherence:

- animated body silhouettes for hover, swim, turn, eat, sleep, recoil, and play;
- face overlays for gaze, blink, suspicion, anger, delight, loneliness, and smugness;
- effect overlays for bubbles, blush, stress marks, spit, crumbs, and ink-like clouds;
- horizontal flipping only when the pose and lighting are safely symmetric;
- bespoke hero reactions for the most important emotional moments.

The face must know what the creature is looking at. Gaze should lead body turns and actions. Eye
direction is expected to provide a disproportionate amount of perceived life.

### Expression inputs

Presentation should derive expression from authoritative state including:

- current intention and movement phase;
- mood, resentment, trust, and dominant need;
- focused object, remembered object, or player cursor;
- recent accepted and rejected interactions;
- dialogue, silence, and nonverbal acts.

Transient expressions need a deterministic presentation queue so a meaningful recoil, stare, or
affection response cannot be overwritten before the player sees it.

## Pixel-perfect rendering

The MVP already renders through a fixed 320x180 logical canvas, nearest-neighbor sampling, and an
integer viewport scale. Visible softness therefore requires diagnosis rather than merely enabling a
nearest-neighbor flag.

Likely sources include semi-transparent pixels baked into generated assets, inconsistent apparent
pixel density, fractional animation positions, Retina drawable sizing, and operating-system window
resampling.

### V1 rendering contract

- Sprites land on whole logical pixels with no fractional scaling or arbitrary rotation.
- Opaque pixel-art assets use deliberate palettes and hard alpha edges.
- Windowed sizes snap to exact multiples of 320x180.
- Fullscreen uses the largest integer scale with intentional letterboxing.
- Text uses a legible pixel font at an appropriate logical scale.
- Asset validation flags excessive colors, semi-transparent fringes, and likely resampling.
- Automated captures cover representative 1x, 2x, 3x, and fullscreen viewport calculations.
- A development-only pixel grid can reveal off-grid drawing and mixed pixel densities.

The first task in this track is to determine whether current blur originates in source assets,
logical rendering, Retina sizing, or final window composition. Do not replace working renderer code
until the capture evidence identifies the failing layer.

## Persistent interface

Typing should simply type. The bottom interface remains visible and the text field is the default
focus whenever a modal chooser is not open.

```text
[ mood face ] [ current behavior ] [ message........................ ] [ actions ] [ send ]
```

### Input rules

- Printable keyboard input always enters the message field.
- Enter sends, Escape clears or closes, and Backspace edits.
- Gameplay shortcuts cannot steal ordinary letters from text entry.
- Controller input reaches the same semantic targets through focus and the on-screen keyboard.
- Pointer, keyboard, and controller produce the same authoritative session commands.

### World interactions

- Hovering an object gives it a crisp outline, cursor change, and short label.
- Clicking the creature opens a compact action strip for comfort, play, or inspection.
- Selecting food enters food-drop mode instead of opening a chain of nested modals.
- Contextual actions appear beside the text area or directly beside their target.
- Idle UI recedes without hiding the persistent text field.

### Visible creature information

The bottom bar may show:

- name;
- broad mood as a face and one word;
- current visible behavior such as `watching you` or `hunting berry`;
- development stage or approximate age;
- one discovered preference or relevant recent fact.

Do not show exact hunger, trust, resentment, comfort, or relationship values. The player should
infer those through behavior and conversation. A later journal may record only facts the player has
actually discovered.

## Deeper creature systems

The aquarium and expression pass is the first V1 batch, not the limit of V1 ambition.

### Development and memory

- Extend development beyond the accelerated three-day proof.
- Add real naming interactions and recognition of names for objects.
- Let beliefs strengthen, weaken, contradict, and visibly change.
- Develop daily routines, learned expectations, and favorite locations.
- Make absence and reunion responses richer without adding permanent harm.

### Environment and objects

- Decorations affect comfort, curiosity, movement, and habits.
- Objects may float, sink, move, hide, break harmlessly, or become territorial targets.
- Plants, cave spaces, currents, lighting cycles, weather shadows, and glass reflections alter play.
- The creature can rearrange, bury, guard, or deliberately misuse objects.
- Special events may introduce algae, strange objects, power flickers, or outside tapping.

### Social behavior

- Provocation gains a target, visible setup, follow-through, and player response.
- Jealousy can attach to a toy, decoration, routine, or player action.
- Apologies, grudges, recurring jokes, and rituals persist across sessions.
- The creature may initiate scarce conversation when a strong motive exists.
- Nonverbal requests become behaviors the player learns to interpret.

### Dialogue and voice

- Preserve short conversational continuity without allowing endless chat.
- Expand authored fallback and nonverbal repertoires to reduce repetition.
- Vary speech speed, pitch, pauses, and vocal noises from authoritative emotion and identity.
- Add simple mouth or face movement during speech.
- Provide an opt-in local playtest transcript export for evaluating dialogue quality.

### Aquarium-specific long-term ideas

- The creature discovers its reflection and forms a mistaken belief about another creature.
- It buries disliked food and later denies doing so.
- Floating and sinking toys reveal preferences through movement and chosen territory.
- It learns that player clicks predict food and attempts to manipulate drop locations.
- Favorite objects, hiding places, and repeated routes turn the aquarium into a visible memory map.
- Long absences alter the aquarium without death or irreversible ruin.
- Older creatures acquire save-specific rituals, vocabulary, and territorial habits.

## Product polish and release work

V1 also needs ordinary game polish after the expression foundation is convincing:

- settings for pixel scale, fullscreen, text size, volume, reduced motion, and rebinding;
- first-run model/runtime installation instead of hand-assembled packages;
- native Windows/Linux acceptance and physical-controller verification;
- visible save backup, recovery, and reset flows;
- Steam assets, compliance review, achievements, and release automation.

Accessibility should include readable focus states, scalable text, subtitles, reduced motion,
separate speech/effects volume, controller glyphs, and a fully keyboard-operable interface.

## Explicitly preserved exclusions

V1 should not dilute the attachment experiment merely because the MVP is complete:

- no multiple creatures or breeding before one creature is deeply expressive;
- no open world before the aquarium has enough behavioral depth;
- no endless chat mode;
- no exact need or relationship bars;
- no personality editor or consequence-free hatch reroll;
- no death, permadeath, or permanent ruin from neglect;
- no model tool calling or model-authored authoritative state;
- no required online service during play.

## First implementation batch

The first autonomous V1 pass should land four connected tracks together:

1. Diagnose and enforce pixel-perfect rendering.
2. Replace the room projection with the aquarium and deterministic swimming foundation.
3. Add layered face, body, gaze, and reaction presentation.
4. Replace modal-heavy interaction with the persistent text and action interface.

This is approximately one substantial 6 to 12 hour autonomous engineering pass, followed by a
separate visual iteration loop for character scale, expression readability, motion timing, and art
selection.

### Acceptance scenario

A useful first V1 scenario is:

```text
creature hovers while tracking the player's cursor
-> player selects berry and drops it high in the water
-> berry sinks visibly
-> creature notices, turns, and swims toward it
-> creature slows and inspects it
-> preference causes eating or a strongly animated rejection
-> face, body, bubbles, and bottom-bar status agree with the reaction
-> player immediately types a message without focusing a field
-> creature answers briefly and retains the authoritative food memory
```

The scenario must run through headless semantic commands and the visible game. Host checks should
also prove crisp integer scaling, pointer hover, food placement, always-focused typing, controller
focus when hardware is available, and direct logical-framebuffer captures.

## Later ordering

Once the first aquarium slice feels good, prefer this order:

1. Broaden movement and expression before adding many objects.
2. Add object variety and aquarium reactivity.
3. Deepen memory, belief revision, rituals, and creature-initiated behavior.
4. Expand authored dialogue, voice variation, ambience, and sound design.
5. Complete platform acceptance, installer work, settings, accessibility, and Steam release polish.

`docs/STATE.md` owns the immediate next action. This document owns the V1 product direction and
should change when playtesting disproves a decision, not merely when implementation advances.
