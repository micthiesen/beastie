# Relationship expression design

Status: chosen direction, not yet implemented

Date: 2026-08-16

## Purpose

Beastie already records relationship values, structured memories, beliefs, favorite places,
routines, development, and learned concepts. The second feel review showed that these systems can
advance substantially while ordinary play still looks nearly unchanged. The missing layer is not
more history storage. It is a simulation-owned way to express selected history through behavior.

This design introduces a **relationship expression director**. It answers one bounded question:

> Given this creature's authoritative history and present situation, is there one familiar thing it
> should do differently right now?

The intended result is that the player notices “we have a history” through movement, anticipation,
attention, routine, refusal, and occasional language. The result must not be a relationship meter,
milestone feed, or chatbot memory recital.

## Product boundaries

- The simulation owns every motif, supporting fact, eligibility rule, selection, cooldown, and
  mechanically meaningful outcome.
- A local model may phrase a selected callback. It cannot decide which history is true, create a
  motif, select a gameplay consequence, or mutate relationship state.
- Relationship expression is embodied first. Most callbacks should work with dialogue and TTS
  unavailable.
- Callbacks are sparse and interruptible. They enrich independent creature life without stealing
  control or demanding conversation.
- Development remains qualitative. Do not add relationship numbers, progress bars, milestone
  popups, history dashboards, or explicit completion rewards.
- A changed or absent cognition backend must leave the same creature, history, routines, and
  embodied callbacks intact.

## Why the current shape is insufficient

The existing systems contain useful evidence, but no component composes it into present behavior:

- Memories are individual structured events. Selection mainly follows salience, recency, concepts,
  and food references.
- Favorites and routines influence destinations, but arriving there does not say why the place has
  become meaningful.
- Relationship values affect thresholds and mood, but a higher bond or trust value rarely creates a
  recognizable shared behavior.
- Dialogue requests can carry candidate memories, but ordinary turns do not maintain enough
  semantic expression history to reliably prevent repetitive callbacks.
- Generated wording can repeat exactly even while authoritative development advances.

Adding more memories or improving prompts alone would preserve this disconnect. The rework must
connect history selection to creature action before language.

## Architecture

```text
structured memories + beliefs + routines + preferences + relationship + present context
                                      |
                                      v
                         relationship motif derivation
                                      |
                                      v
                  eligibility, salience, cooldown, player priority
                                      |
                                      v
                         one active relationship beat
                          /                       \
                         v                         v
        intention / gaze / movement / cue       selected dialogue motif
                         |                         |
                         v                         v
                embodied callback          model phrases bounded fact
```

The design has three new concepts: motifs, an expression ledger, and active relationship beats.

### Relationship motifs

A motif is a typed, authoritative pattern supported by existing state. Initial motif kinds should
stay deliberately small:

```rust
enum RelationshipMotifKey {
    SharedToy(ToyId),
    ComfortRitual,
    TrustedFood(FoodId),
    FoodGrudge(FoodId),
    PlayerReturns,
    FamiliarPlace(SemanticDestination),
}
```

A derived motif includes its key, strength, supporting memory IDs or routine evidence, last
supporting day, and the present triggers it can answer. Strength is calculated from explicit
evidence such as repeated shared play, repeated comfort, food preference plus feeding history,
belief support, genuine cross-day visits, and returns after absence.

Motifs should normally be derived from canonical state rather than persisted as duplicate truth.
The same save and evidence must always produce the same motif set.

### Expression ledger

The creature needs a small persisted ledger describing expression, not history:

```rust
struct RelationshipExpressionState {
    recent: Vec<ExpressedMotif>,
    active: Option<RelationshipBeat>,
}

struct ExpressedMotif {
    key: RelationshipMotifKey,
    expressed_at_ms: u64,
    expression_kind: RelationshipExpressionKind,
}
```

The ledger prevents the strongest motif from firing repeatedly. It is bounded, versioned, and safe
to default during migration. Deleting it may alter presentation variety, but cannot erase memories,
relationships, routines, or anything that happened.

### Active relationship beats

A selected motif becomes a short authoritative beat with an explicit trigger, target, phase, and
supporting evidence:

```rust
struct RelationshipBeat {
    motif: RelationshipMotifKey,
    trigger: RelationshipTrigger,
    expression_kind: RelationshipExpressionKind,
    evidence: Vec<MemoryId>,
    target: Option<SemanticDestination>,
    phase: RelationshipBeatPhase,
    started_at_ms: u64,
}
```

The initial phases should reuse the game's successful action grammar: notice, anticipate, act, and
recover. A beat may set intention, gaze, destination, or a bounded body cue. Presentation projects
that state but does not infer a motif from relationship numbers.

An active beat yields immediately to direct care, danger-free player interaction, speech attention,
sleep, or another higher-priority authored action. It must never make player input appear lost.

## Selection rules

The director evaluates motifs only at meaningful trigger points, not continuously every frame:

- player return after an absence;
- player focus, pickup, or use of a familiar object;
- arrival at a learned routine time or place;
- a need or mood state associated with prior care;
- a relevant understood utterance;
- the end of another action, when a callback can occur without interruption.

Eligible motifs receive deterministic scores from evidence strength, relevance to the trigger,
recency of new evidence, temperament, relationship, and time since last expression. The director
selects at most one. Cooldowns operate per motif and globally. A familiar behavior may recur, but it
must not dominate every return or every interaction with the same object.

Selection randomness, if used, comes from an isolated deterministic domain so expression variety
cannot perturb needs, preferences, action outcomes, or other simulation behavior.

## Initial embodied callbacks

### Shared toy

- Early: Beastie notices the toy during ordinary play.
- Familiar: when the player returns or focuses it, Beastie looks toward or approaches it before the
  play command.
- Established: at an appropriate quiet moment, Beastie draws attention to it as a shared ritual,
  then returns to independent life if the player does not engage.

### Comfort ritual

- Early: comfort improves state and produces the existing affection aftermath.
- Familiar: when uncomfortable and the player is present, Beastie approaches or waits near the
  player instead of only changing mood.
- Established: recovery uses a stable creature-specific affection routine whose timing and
  willingness still depend on temperament, trust, and resentment.

### Trusted food or food grudge

- Early: Beastie accepts or rejects based on preference.
- Familiar: it recognizes the food sooner and anticipates or hesitates before the normal outcome.
- Established: the response carries a particular learned ritual, while the underlying acceptance
  rule remains authoritative and unchanged.

### Return pattern

- Early: the player return is noticed.
- Familiar: repeated reliable returns produce a bounded approach, wait, or attention beat selected
  by temperament.
- Established: the creature may connect the return to another supported motif, such as waiting near
  a shared toy. Absence never causes permanent harm.

### Familiar place

- Early: a location is one destination among several.
- Familiar: repeated genuine cross-day visits create a recognizable settled pose or inspection.
- Established: the creature performs the routine near its learned time, but skips or varies it when
  needs, player action, or another strong motive takes priority.

## Dialogue integration

Dialogue should receive the motif already selected by the simulation, not an undifferentiated pile
of memories. A bounded projection can expose:

```text
motif: shared_toy(sock)
expression: familiar_callback
evidence: [memory 4, memory 7]
recently_expressed: false
```

The model may choose phrasing and an allowed gesture. It must reference only offered evidence and
return the selected memory ID when recalling a fact.

The session should also maintain a bounded semantic expression history containing recent topic,
motif, recalled memory or belief, and fallback lane. This is not raw chat history and contains no
player text. It allows the planner to suppress accidental consecutive selection of the same motif
or fact.

Exact duplicate generated replies should receive one bounded retry with the recent normalized reply
fingerprint supplied as a prohibition. If retry fails, use a grounded authored fallback for the
selected motif. Generated wording remains derived expression and never becomes canonical history.

Conversation remains optional. A relationship beat must have a coherent nonverbal form when the
model, TTS, or microphone is absent.

## Development should feel qualitative

The system should not expose universal “levels.” A motif becomes more expressive only when its own
evidence grows. Different saves therefore develop differently:

| Motif | Early | Familiar | Established |
|---|---|---|---|
| Shared toy | notices it | anticipates shared play | initiates a bounded ritual |
| Comfort | accepts care | seeks the player when distressed | performs a distinctive recovery |
| Trusted food | accepts it | anticipates it | waits near a learned feeding place or time |
| Food grudge | rejects it | recognizes it immediately | performs the learned grudge behavior |
| Familiar place | visits | settles recognizably | returns near a learned time |

Strength changes eligibility and expression range, not the truth of memories or guaranteed
obedience. Mood, needs, temperament, and player priority still decide whether a callback happens.

## Implementation slices

### Slice 1: authoritative motifs and planner

- Add typed motif derivation from existing memories, routines, beliefs, and preferences.
- Add a bounded migrated expression ledger and active beat state.
- Implement trigger evaluation, deterministic scoring, cooldowns, player priority, and exact replay
  tests.

### Slice 2: three embodied proofs

- Implement shared-toy anticipation, comfort seeking, and one return callback.
- Reuse existing action phases, body sprites, gaze, movement, cue, and audio vocabulary where it is
  semantically correct.
- Verify that every callback returns to independent life and remains interruptible.

### Slice 3: dialogue continuity

- Project the selected motif and evidence through the protocol.
- Persist bounded semantic recent turns outside raw player text.
- Add duplicate suppression, one retry, and grounded motif-specific fallbacks.

### Slice 4: development breadth

- Add trusted-food, grudge, and familiar-place expressions only after the first three beats feel
  distinct and restrained.
- Tune strength thresholds and cooldowns from full-length evidence rather than unit tests alone.

Estimated implementation and evidence time for the first three slices is 8 to 12 focused hours.
Breadth and taste tuning should be a separate pass rather than expanding the first implementation
until every memory kind has a bespoke animation.

## Feel-review acceptance

Extend `relationship-over-time` so the same seed proves the following:

1. Each active day contains at most a bounded number of relationship beats.
2. Day-two, day-three, and day-four arrival sections show qualitatively different callbacks backed
   by accumulated evidence.
3. At least one callback is entirely nonverbal and remains legible with AI and TTS unavailable.
4. Direct player interaction interrupts or supersedes an ambient callback without lost input,
   duplicate payoff, or stale presentation.
5. Consecutive talks do not repeat the same motif, memory, or exact caption when another grounded
   expression is available.

The final inspect marker must hold for at least 1.5 seconds and expose review state in the evidence
trace. It does not need to create a player-facing history dashboard.

Review the full synchronized video at native scale. Passing traces alone cannot prove that a
callback reads as familiarity rather than random animation or scheduler machinery.

## Failure modes to guard against

- **Callback spam:** one strong motif fires on every relevant action. Use global and per-motif
  cooldowns plus recent-expression exclusion.
- **Fake history:** presentation or model output implies unsupported events. Every beat and phrase
  must carry authoritative evidence IDs.
- **Chatbot drift:** relationship expression becomes mostly spoken recollection. Require a coherent
  nonverbal form and keep initiated language scarce.
- **Loss of agency:** ambient behavior delays care or language. Direct player action and speech
  attention retain priority.
- **Generic progression:** every creature unlocks the same sequence. Motif strength grows from that
  creature's evidence, preferences, temperament, and routines.
- **Overfitting one scenario:** acceptance must include returns, object interaction, distress,
  missing AI, save/reload, accelerated time, and quiet observation.

## Deliberate non-goals

- A browsable memory journal or relationship history screen.
- Exact relationship meters or named bond levels.
- A model-maintained conversation transcript as creature truth.
- New memories created because generated dialogue mentioned an event.
- A bespoke animation for every memory kind in the first pass.
- Guaranteed callbacks on every return, routine window, or familiar interaction.
