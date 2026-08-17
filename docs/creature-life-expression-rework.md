# Creature life and expression rework

Date: 2026-08-17
Status: proposed

## Outcome

Mop must read as one particular fish with a private life, not as a polished state machine waiting
between player commands. Preferences, places, needs, routines, and shared history should shape
recognizable activities without collapsing into one repeated favorite-toy transaction. Speech and
every emotional state must retain the canonical fish identity.

The implementation is complete only when all of these are true:

1. Every normal, talking, relationship, and AI-off frame is recognizably the same gold fish.
2. Quiet play contains several simulation-grounded activity families with distinct physical,
   temporal, and sonic contours.
3. A favorite may recur, but recent activity, recipe, subject, and session rhythm prevent it from
   monopolizing the lived experience.
4. Ball, bell, and sock play visibly involve the exact object in different ways.
5. Shared history is embodied through motif-specific action rather than a generic heart callback.
6. Technical microphone failure never becomes creature perception.
7. Deferred dialogue waits for a semantic action boundary or communicates a truthful change of
   mind. A fixed grace timer cannot silently abort the action it promised to respect.

This is a cohesive creature-life pass. It may refactor the authoritative activity, scheduling,
relationship, perception, dialogue-arbitration, asset, audio, save, fixture, and feel-evidence
seams. Preserve the simulation-first, offline, graceful-degradation, and no-permanent-ruin product
boundaries.

## Product rules

- The simulation owns what Mop is doing, why, with what subject, and whether it completed.
- Presentation can enrich an activity with transient motion and effects, but cannot invent a visit,
  object displacement, memory, preference, perception, or outcome.
- Preference changes probability and style. It does not grant a favorite permanent control of idle
  selection.
- Variation must arise from state, personality, place, routine, recent history, and bounded
  selection, not from a presentation-only random animation jukebox.
- Quiet is part of the experience. Do not fill every open interval with a payoff, cue, heart, or
  relationship callback.
- Places are activities, not only destinations. Cave, plant, bottom, open water, and toys must have
  different arrival and settlement reads.
- A memory callback must preserve its exact motif, subject, and evidence through selection,
  movement, action, audio, recovery, interruption, save, and trace.
- A device-control attempt is not sensory evidence. Creature perception begins only after capture
  succeeds or captured voice evidence otherwise exists.
- Dialogue may observe or defer to a current activity. It does not receive an untraced timer-based
  right to steal that activity.
- AI and TTS remain optional expression layers. Every authoritative activity and relationship beat
  remains mechanically and visually complete without them.

## Authoritative private-life activity

Introduce one typed, persisted owner for autonomous private life. Names may differ, but the
semantic shape is required:

```rust
struct PrivateLifeActivity {
    id: NonZeroU64,
    kind: PrivateLifeKind,
    subject: Option<ActivitySubject>,
    purpose: ActivityPurpose,
    phase: ActivityPhase,
    selected_from: ActivitySelectionEvidence,
}

enum PrivateLifeKind {
    ToyPlay(ToyId),
    CaveSettle,
    PlantInspect,
    BottomForage,
    OpenWaterDrift,
}

enum ActivityPhase {
    Notice,
    Approach,
    Act,
    Recover,
    Settle,
    Interrupted,
}
```

`selected_from` records the traits, need pressure, preference, routine, relationship evidence, and
recent-activity exclusions that made the choice legal. It is diagnostic grounding, not a promise
to expose scores in UI. IDs are nonzero, monotonically allocated, and never reused within a save
lineage. Phase transitions are legal, deterministic, and at-most-once.

The existing typed travel purpose remains the movement owner. A private-life approach carries its
exact activity ID, and arrival advances only that activity. Do not infer an activity from a bare
destination, current intention, status label, mood, or nearby object. Direct player toy actions,
food actions, refusals, relationship beats, initiatives, and private life remain distinguishable
owners even when they target the same object or location.

An interruption records the responsible owner and the phase it interrupted. No canonical payoff,
visit evidence, routine evidence, relationship change, or object displacement occurs unless its
defined semantic boundary was reached. Recovery may finish after payoff without repeating it.

## Selection and rhythm

Persist a bounded recent-activity ledger containing at least activity family, semantic subject,
authored recipe, selection time, completion time, interruption, and day/session context. Selection
must consider this ledger before weighted choice.

The selector follows these rules:

- needs, traits, routine windows, preferences, familiar places, relationship state, and current
  environment decide what is eligible and how it is weighted;
- recent repetition suppresses the same family, subject, and recipe independently, rather than
  only avoiding the immediately previous destination;
- a strong need may override suppression, but the resulting urgency must be legible in movement,
  action, or status rather than resembling an ordinary repeated idle pick;
- a favorite can return after contrast and quiet, while a nonfavorite remains possible when state
  supports it;
- settlement and unperformed observation remain valid choices, so selection does not manufacture
  continuous spectacle;
- fixed seeds remain exactly replayable, and save/reload preserves the next choice and active
  activity without consuming extra RNG.

Do not encode acceptance as one brittle count such as “at least N activities.” The test oracle
should combine structural repetition limits with native lived review. A seed-42 three-minute quiet
run must no longer be five sock trips, and cross-seed runs must show temperament-shaped breadth
without converging on the same authored playlist.

## Embodied activity families

Each family needs a recognizable notice, action, recovery, and optional settlement contour. It may
reuse shared locomotion and subtle poses, but its payoff cannot be a generic smile plus heart.

### Toy play

- Ball: make contact, nudge or roll the ball, follow its motion, then settle or reorient.
- Bell: approach with caution or interest, strike it, react to the ring and recoil, then decide
  whether to remain.
- Sock: tug, lift, trail, carry, or drop it with a soft physical response distinct from impact.

Any durable position, velocity, carried state, or final placement is core-owned and saved.
Presentation may add short squash, stretch, recoil, particles, or a motion trail that returns to
the authoritative state. Contact, object response, cue, relationship evidence, and completion use
the same interaction and activity IDs. Autonomous play must not grant direct-player relationship
reward.

### Places and water

- Cave: enter or shelter at the mouth, reduce movement, and sustain a bounded settled pose.
- Plant: inspect, orbit, brush, or rub against the exact plant, with localized leaf response if it
  is presentation-only and returns cleanly.
- Bottom: descend, forage or nose the sand, disturb a small localized patch, and rise or settle.
- Open water: drift, watch bubbles, hold in current, or perform a sparse exploratory turn without a
  transaction-like payoff.

These actions must be readable from body, target, and timing with the status hidden. They remain
interruptible by the same priority and causality rules as other owned actions.

## Relationship expression

Retain the existing exact relationship director and evidence model, but replace the shared generic
performance template with motif-specific recipes:

- `SharedToy` orients to and acts on the exact remembered ball, bell, or sock. It composes with the
  matching object activity rather than adding an unrelated heart nearby.
- `FamiliarPlace` travels to, recognizes, and settles at the exact cave or plant.
- `ComfortRitual` seeks or holds attention toward the player and uses the accepted comfort body,
  not a generic curious notice.
- `PlayerReturns` notices the player, approaches or holds a state-shaped distance, and may initiate
  grounded speech without replacing the fish body.
- food trust and grudge remain bound to the exact current food action when one exists. Standalone
  recollection stays sparse and cannot imitate a direct receipt.

Each motif defines its own phase ranges, body targets, environmental involvement, audio vocabulary,
and recovery. Deliberate silence is valid. Do not reuse one fixed one-second notice, two-second
anticipate, two-second act, and two-second recover grid for every motif.

Extend the relationship ledger so the same evidence, subject, motif, and performance recipe cannot
repeat at short lived intervals. Newly formed memory does not automatically require a callback
within tens of seconds. Selection should favor a later state-grounded opportunity, or no callback,
over a receipt-like echo. Preserve exact interruption, save/reload, AI-off, and dialogue-grounding
behavior from the implemented relationship contracts.

## Speech and perception boundaries

### Microphone acquisition

Do not submit `SpeechStarted` to core before the microphone is successfully acquired. The shell
must first establish a live bounded capture, then submit the authoritative start command that can
produce attention and `SpeechPerceived`. If platform constraints require an asynchronous
acquisition phase, represent it as technical shell state that cannot project creature behavior.

Acquisition failure produces only the existing technical UI status. It produces no gaze, curious
cue, action interruption, dialogue request, transcript entry, memory, belief, relationship change,
or canonical perception event. Recognizer failure after successful capture may retain a restrained
notice only when actual voice evidence was captured; the trace must distinguish capture evidence
from recognition success.

### Action-aware dialogue arbitration

Replace the fixed deferred-speech grace period with a semantic boundary supplied by the current
action owner. Each interruptible activity declares safe response points, for example after toy
contact, after food payoff, after a refusal act, during settlement, or at recovery completion.
Dialogue that says it is waiting remains pending until one of these occurs, the activity is
truthfully interrupted by a new simulation decision, or a bounded expiration is reached.

Expiration must not silently seize the body. It either keeps the utterance queued, discards it with
honest technical/session feedback, or creates a typed simulation-owned change-of-mind that is
visible before dialogue takes ownership. Sleeping and resentful refusal checks remain authoritative
at the handoff point. Loading, supersession, and a newer utterance invalidate the pending owner
without late caption, voice, mouth, or status residue.

## Canonical talking body

Replace or recurate all eighteen `talk/{mood}-south-{closed,half,open}` frames against the canonical
gold fish, not only the currently failing hungry row. Every frame must preserve:

- gold palette and lighting logic;
- fish anatomy, fins, tail, and absence of legs or paws;
- body mass, opaque bounds, scale, centroid, eye line, and mouth anchor;
- stable placement when entering, cycling, and leaving speech;
- mood legibility through expression and pose without species or silhouette drift.

Use the canonical source art and existing normalization/geometry tooling. Provider output is a
candidate, never acceptance evidence. Inspect one all-mood contact sheet at the real 2x aquarium
scale and transition strips for normal-to-talk, talk mouth cycling, and talk-to-normal. Add
automated palette, opaque-envelope, centroid, and anchor checks, while treating native visual
inspection as the final identity oracle.

Relationship callbacks and initiated AI-off speech must use this same talking family. No special
return, hunger, motif, or fallback route may substitute another creature body.

## Audio direction

Keep the existing ambient bed, headroom, ducking, and sparse layering. Expand semantic vocabulary
only where it makes an action or motif materially more legible.

- Ball, bell, and sock require different contact characters. A bell rings, a ball produces a soft
  nudge or roll, and a sock produces cloth movement rather than the shared impact receipt.
- Cave, plant, bottom, and open-water activities may be quiet. Use environmental detail and body
  timing before adding a creature cue.
- Relationship motifs may reuse a voice family, but not the identical sample, onset, seven-second
  grid, and heart punctuation for every meaning.
- Repeated private-life payoffs must not form an evenly spaced one-shot metronome.
- Microphone acquisition failure has no creature cue. Technical UI sound, if retained, stays on the
  UI channel and cannot imply hearing.
- Respect the existing per-channel priority, interruption, and maximum-layer rules. Do not solve
  variety by stacking more sounds or filling quiet.

All authored sounds retain provenance and license metadata. Reference mixes remain review aids,
not substitutes for host-output listening.

## Save migration and validation

If private-life activity, selection evidence, object state, recent-activity history, or new
relationship recipe history becomes authoritative, bump the core save version. Because production
session saves embed the core world, bump `SESSION_SAVE_VERSION` and route older embedded worlds
through the core migrator explicitly.

Migration is conservative:

- preserve needs, position, velocity, exact live food and toy owners, relationships, memories,
  routines, preferences, visits, beliefs, and completed outcomes;
- convert an unowned old idle destination to idle hover or a safely classifiable private-life
  approach without inventing contact, payoff, displacement, visit evidence, or relationship
  expression;
- retain a live direct toy, food, refusal, relationship, initiative, or dialogue owner rather than
  replacing it with private life;
- initialize new allocators above every migrated live and historical ID;
- seed recent ledgers from exact existing evidence when possible, otherwise start them empty rather
  than guessing a performed recipe;
- validate subject/target consistency, legal phase transitions, owner uniqueness, bounded ledgers,
  nonzero IDs, and object positions inside legal aquarium bounds.

Offline progression may complete authoritative private-life state deterministically, but resume
compaction must suppress stale impacts, hearts, captions, voice, and other hours-old performance.
Save/reload at every activity phase must reproduce final state, RNG, event order, object placement,
and next selection exactly.

## Deterministic coverage

Add focused core, session, view, game, asset, audio, fixture, and migration tests for at least:

- seed-42 three-minute quiet selection no longer choosing five sock activities, with deterministic
  replay and structural limits on repeated family, subject, and recipe;
- several seeds and temperament/preference fixtures showing state-shaped breadth rather than one
  universal sequence;
- cave, plant, bottom, open-water, ball, bell, and sock phase progression, interruption, payoff,
  recovery, traces, and save/reload;
- durable ball, bell, and sock object response, including no displacement before contact and no
  duplicate response after recovery or resume;
- autonomous versus direct-player rewards and relationship mutations remaining distinct;
- exact shared-toy, familiar-place, comfort, return, trust, and grudge motif performance, with
  repetition suppression by evidence and recipe;
- a newly formed memory not forcing a short-delay generic callback;
- microphone-unavailable from idle producing no creature perception event, attention, body change,
  cue, dialogue, or canonical mutation;
- successful microphone capture retaining immediate truthful acknowledgement, and captured-audio
  recognizer failure following its explicit policy;
- deferred speech waiting through an accepted toy's contact boundary, plus expiration,
  supersession, sleep, resentment, load, and active-TTS races;
- all eighteen talking frames passing declared asset checks and rendering in a native contact
  sheet without identity drift or transition jumps;
- nominal fixture dialogue returning a valid non-fallback turn in AI-on baseline sessions, while
  `bad-conditions` separately proves worker, recognizer, microphone, and TTS degradation;
- all previously accepted direct-food, rejected-toy, dialogue-supersession, AI-off, relationship
  causality, UI hierarchy, ambience, ducking, and graceful-degradation regressions.

Tests should assert semantic owners and state, not only status strings or screenshot hashes. Keep
the full `cargo xtask verify` gate independent of display, network, model, GPU, audio device, and
microphone.

## Native evidence contract

Implementation requires normal-speed native debug review before trace inspection. Capture to a new
immutable evidence root and retain failed attempts:

1. Run the complete seven-experience baseline with seed 42.
2. Run focused three-minute quiet sessions across at least three seeds, including seed 42 and at
   least two preference or temperament shapes.
3. Capture an all-mood talking gallery and transition filmstrip at the real 2x aquarium scale.
4. Capture object-specific ball, bell, and sock contact sequences with the status hidden during
   adjudication.
5. Capture relationship callbacks for shared toy, familiar place, comfort, return, trust, and
   grudge in AI-on and AI-off modes.
6. Capture microphone-unavailable, recognizer-unavailable-after-capture, deferred-speech boundary,
   supersession, and save/load races.
7. Produce synchronized video, events, state, audio plan, reference mix, inputs, filmstrips, and
   hash-pinned manifests for every promoted run.

Review continuous footage and reference audio first. Then use traces to explain ownership, timing,
selection, and mutation. Compare the seed-42 after run directly with
`target/feel/fifth-round-baseline-before`. Native approval requires independent visual,
causality/behavior, and audio/timing adjudication. A deterministic trace, unit test, or isolated
contact sheet cannot replace the lived-session verdict.

The evidence should answer these questions without consulting the status label:

- Is this always the same fish?
- What private activity is Mop doing, and why does its subject matter?
- Does a favorite feel like a preference rather than a scheduled job?
- Is this exact memory being expressed rather than generic affection?
- Did Mop actually perceive the player, and did dialogue respect the action it said it would wait
  through?

## Implementation sequence

1. **Close identity drift:** recurate all talking frames, add geometry diagnostics, and prove the
   all-mood native gallery before building more performances on them.
2. **Establish activity authority:** add the typed private-life owner, phases, selection evidence,
   repetition ledger, exact travel linkage, persistence, migration, and deterministic selection
   tests.
3. **Embodied private life:** implement place recipes and object-specific ball, bell, and sock
   behavior with authoritative object response and presentation embellishment.
4. **Specific relationship expression:** compose motifs with exact activities, diversify timing and
   audio, and suppress short receipt-like repetition.
5. **Repair perception and arbitration:** acquire the microphone before perception and replace the
   deferred-speech timer with semantic safe boundaries.
6. **Prove the complete experience:** repair the nominal fixture dialogue path, run focused and full
   native captures, adjudicate independently, fix every surviving regression, and update the
   implementation ledger and durable state.

Slices may be rearranged when a cleaner dependency order emerges, but do not close the pass with
only art replacement, scheduler tuning, or new animation. The authority, embodiment, expression,
perception, and evidence seams must agree.

## Acceptance criteria

- All seven baseline experiences validate and are independently adjudicated from a fresh native
  evidence root.
- Seed-42 quiet play no longer reads as a repeated sock commute, and cross-seed review demonstrates
  state-shaped variation without constant spectacle.
- Cave, plant, bottom, open-water, ball, bell, and sock activities are distinguishable from body,
  target, timing, and audio with status text hidden.
- Ball, bell, and sock physically involve their exact object and preserve authoritative state
  through interruption, save/load, and offline progression.
- Shared history callbacks preserve exact motif, subject, and evidence and no longer repeat as the
  same curious-cue, heart, and fixed phase grid.
- Every talking mood and relationship/initiative route remains the canonical gold fish through
  entry, mouth motion, and recovery.
- Microphone-unavailable creates no creature perception or cue.
- Deferred speech respects an explicit semantic safe boundary or a visible authoritative
  change-of-mind.
- Nominal AI-on fixture runs exercise healthy dialogue; degraded runs remain separate and explicit.
- Direct food, refusal causality, turn supersession, AI-off completeness, relationship causality,
  technical statuses, aquarium composition, UI readability, ambience, and ducking retain their
  accepted behavior.
- `cargo xtask verify` passes without display, network, model, GPU, audio device, or microphone.
- Native evidence uses normal debug builds. Release binaries, installers, and model bundles remain
  outside this pass.
- This document's implementation ledger, the fifth feel review, and `docs/STATE.md` describe the
  verified final state rather than the proposal.

## Non-goals

- Rewriting the aquarium composition, interaction deck, settings, type system, or global UI.
- Reopening accepted food timing, rejected-toy causality, dialogue-turn ownership, or technical
  fallback wording without new evidence.
- Adding online services, model-authored facts, cloud persistence, or required AI/TTS.
- Filling every idle interval with authored action, voice, particles, music, or callbacks.
- Balancing long-horizon attachment solely from accelerated fixture days.
- Rebuilding release packages or completing parked Windows and physical-controller acceptance.

## Implementation ledger

This contract is proposed and has no implementation entries yet. During implementation, record
each material slice with its authoritative change, focused tests, native evidence root, surviving
limitations, and independent adjudication. On completion, change `Status` to `implemented and
verified` and rewrite this section as the durable final-state ledger.
