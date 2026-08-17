# Relationship causality and presentation rework

Status: proposed next implementation

Date: 2026-08-17

## Outcome

Relationship expression should answer the thing happening now before it recalls anything else.
Berry history may change a berry interaction, mushroom history may change a mushroom interaction,
and a learned cave routine may change a cave visit. An unrelated memory must never appear to be
feedback for the player's current action.

This rework splits relationship expression into two paths:

1. **Action-bound expression** modifies an existing interaction with the exact same semantic
   subject. Trusted food and food grudges belong here.
2. **Standalone expression** remains a sparse, interruptible behavior selected at a quiet or
   explicitly relevant moment. Returns, comfort seeking, shared rituals, and familiar places belong
   here.

The simulation owns both paths. View, audio, and dialogue project the selected expression; they do
not infer familiarity independently.

This supersedes the broad `ActionCompleted` selection described in
[relationship-expression-design.md](relationship-expression-design.md). It responds to the causal
break found in [feel-review-third-pass.md](feel-review-third-pass.md).

## Why scoring changes are not enough

The current director can select any object motif eligible for `ActionCompleted`. Generic relevance
is low, but a mature unrelated motif can still win. This produced two native failures:

- eating berry selected a shared-ball memory;
- rejecting mushroom selected trust in berry.

Increasing relevance weights would make those examples less common without making them impossible.
The trigger also arrives after the food outcome, too late to express recognition or anticipation.
The correct fix is to make mismatched candidates unrepresentable and bind familiarity to the action
while it is still unfolding.

## Product rules

- Present context is a hard eligibility boundary, not a scoring preference.
- Historical evidence may alter expression, never the authoritative accept/reject/play/care
  outcome.
- Direct interaction feedback remains immediate. Relationship expression enriches it without
  delaying it or launching a competing seven-second sequence.
- Unrelated recollection is allowed only through a distinct ambient trigger during independent
  creature time.
- Positive and negative cues must not occupy the same semantic moment unless deliberately authored
  as ambivalence.
- Every expression remains coherent with AI, TTS, microphone, or audio output unavailable.
- No relationship meters, history feed, milestone popup, or model-authored facts are introduced.

## Architecture

```text
player interaction + exact subject
             |
             v
authoritative action and outcome ----------------------+
             |                                         |
             v                                         v
matching relationship motif                    ordinary interaction
             |                                         |
             v                                         |
action-bound expression context                         |
             +------------------+----------------------+
                                v
                 phases, gaze, movement, cue, audio

quiet/relevant trigger -> standalone relationship beat -> embodied callback
```

The two paths share motif derivation, evidence, maturity, cooldowns, the expression ledger, and
dialogue projection. They differ only in scheduling and ownership.

## Typed trigger contract

Remove broad object selection from `ActionCompleted`. Exact subjects should be present in the type
that reaches selection.

One suitable shape is:

```rust
enum RelationshipTrigger {
    FoodPresented { food: FoodId },
    FoodResolved { food: FoodId, outcome: FoodOutcome },
    ToyEngaged { toy: ToyId },
    ComfortCompleted,
    PlayerReturn,
    RoutineWindow { hour_start: u8, destination: SemanticDestination },
    PlaceArrived { destination: SemanticDestination },
    RelevantUtterance { subject: Option<RelationshipSubject> },
    QuietMoment,
}

enum RelationshipSubject {
    Food(FoodId),
    Toy(ToyId),
    Place(SemanticDestination),
    Player,
}

enum FoodOutcome {
    Consumed,
    Rejected,
}
```

Exact variant names may follow existing conventions. The invariant matters:

| Trigger | Eligible motifs |
|---|---|
| berry presented/resolved | `TrustedFood(Berry)`, `FoodGrudge(Berry)` only |
| mushroom presented/resolved | `TrustedFood(Mushroom)`, `FoodGrudge(Mushroom)` only |
| ball engaged | `SharedToy(Ball)` only |
| cave routine/arrival | `FamiliarPlace(Cave)` only |
| comfort completed or matching need | `ComfortRitual` only |
| player return | `PlayerReturns`, plus a deliberately authored return-linked ritual |
| relevant utterance with subject | motifs matching that subject only |
| quiet moment | any grounded standalone motif that passes cooldown and priority rules |

Selection should first filter by this table, then score the remaining candidates. Strength,
recency, temperament, relationship, and time since expression still choose among valid candidates;
they never make an invalid subject eligible.

`QuietMoment` is the only subject-free route for an unrelated recollection. It should be evaluated
only while the creature is idle, no direct action or speech is pending, needs are not urgent, and
the global callback budget permits it. Its trace and event name must make clear that it is ambient,
not an action receipt.

## Action-bound expression

### State

Add an optional relationship context to the authoritative action timeline rather than starting a
competing active relationship beat:

```rust
struct ActionRelationshipContext {
    motif: RelationshipMotifKey,
    expression_kind: RelationshipExpressionKind,
    evidence: Vec<RelationshipEvidence>,
    subject: RelationshipSubject,
}
```

The food action must retain `FoodId` through recovery even after a consumed object is removed from
the aquarium. The resolved `FoodOutcome` should also remain available until recovery completes.
This can be stored directly on the action timeline or in an equally bounded typed action result.

At food presentation:

1. Create the normal authoritative food action.
2. Derive only motifs matching that `FoodId`.
3. If one is eligible, attach its relationship context to the action.
4. Record it in the bounded expression ledger at the point it first becomes visible.
5. Let the normal action phases advance and resolve acceptance or rejection from preference.

The relationship context may change phase duration, gaze, steering, body cue, and sound. It cannot
change preference, hunger, memories, beliefs, relationship values, or the final outcome.

### Trusted food recipe

- **Notice:** pause and gaze at the exact food. Use restrained positive attention, not a heart.
- **Anticipate:** turn toward the food or feeding place sooner than an unfamiliar food. A familiar
  expression may shorten generic inspection, but must not teleport or skip readable movement.
- **Act:** retain the normal eating outcome and `food/eat` cue.
- **Recover:** settle contentedly, then resume independent life.
- **Audio:** reuse one soft positive cue such as `creature/mrr` at recognition, then the ordinary eat
  sound at consumption. Do not stack affection at full level.

The initial pass should reuse notice, positive gaze, movement, and settle vocabulary. Commission a
new food-anticipation effect only if native hidden-label clips remain ambiguous.

### Food grudge recipe

- **Notice:** brake and lock gaze on the exact disliked food before ordinary inspection.
- **Anticipate:** use suspicion or hesitation, not the terminal spit effect.
- **Act:** retain the authoritative rejection and its existing push-away/spit body and sound.
- **Recover:** clear the rejection residue before another positive or ambient expression may begin.
- **Audio:** one restrained recognition/annoyance cue before the outcome, then one semantic
  `food/spit-reject` cue at rejection. Never layer two full-level rejection sounds.

Maturity should make recognition earlier and the pre-rejection behavior more characteristic. It
must not manufacture a rejection when current authoritative preference would accept the food.

### Shared toy and comfort

Direct toy and comfort actions should follow the same exact-subject rule, even if they keep their
current implementation initially:

- ball input can bind only `SharedToy(Ball)`;
- comfort input can bind only `ComfortRitual`;
- immediate toy impact and comfort feedback remain the primary action receipt;
- a mature ritual may alter anticipation or recovery but must not delay the direct payoff.

## Standalone expression

Keep the existing interruptible `RelationshipBeat` for behavior that is not already owned by a
direct action:

- player-return notice, anticipation, or welcome;
- comfort seeking from a matching need state;
- a quiet shared-toy invitation;
- familiar-place recognition and routine behavior;
- explicitly ambient recollection during `QuietMoment`.

Standalone beats retain notice, anticipate, act, and recover phases. Direct player input, speech
attention, sleep, and urgent needs still interrupt them immediately.

An active standalone beat must carry its subject where one exists. Validation should reject or
cancel a beat whose motif, evidence, trigger subject, or target no longer agrees with canonical
state.

## Familiar-place expression

Familiar place remains standalone because the place visit is itself independent creature behavior.
Its presentation should rely on spatial behavior more than punctuation:

- **Notice:** orient toward the learned destination or landmark.
- **Anticipate:** approach using normal authored swimming.
- **Act:** settle, hover, or inspect at the destination for a bounded interval.
- **Recover:** leave naturally when another motive wins.
- **Audio:** remain quiet during travel. Use `environment/cave-settle` only when actually settling
  at the cave; other destinations use their semantically appropriate existing ambience or silence.

The routine must yield to needs and player actions. Missing the routine window is not failure and
must not create a notification or deferred obligation.

## Presentation ownership and cue priority

View and audio should consume one semantic owner for each moment:

1. direct authoritative outcome, such as eat, reject, comfort, or toy impact;
2. the current action-bound relationship phase;
3. an active standalone relationship beat;
4. ordinary mood and ambience.

When ownership changes, incompatible residue from the previous owner clears immediately. A trusted
food context cannot begin under spit or suspicion. A grudge cannot inherit hearts or delight. A
direct action interrupting a standalone beat replaces its visual cue immediately and cancels or
attenuates the superseded relationship one-shot.

Do not solve priority with longer cue durations. Each cue should carry a semantic owner or token so
the runtime can replace only incompatible presentation while preserving unrelated ambience.

### Initial vocabulary map

| Motif | Recognition | Act/outcome | Recovery audio |
|---|---|---|---|
| trusted food | positive notice, food gaze, soft `mrr` | ordinary eat/crumbs | quiet settle |
| food grudge | suspicion, fixed food gaze | push-away/spit | silence |
| shared toy | toy gaze, notice | delight plus actual toy impact | quiet settle |
| player return | player gaze, notice | approach or affection for welcome only | silence |
| familiar cave | cave orientation | settle/inspect | cave settle |
| comfort ritual | player gaze, approach | comfort/affection | soft `mrr` if no stronger cue played |

This table is a starting contract, not proof of quality. Native hidden-label review decides whether
existing vocabulary is sufficient.

## Dialogue

Dialogue continues to receive a motif only after simulation selection. Add the semantic subject and
selection mode so phrasing cannot blur ambient recall with a current interaction:

```text
motif: trusted_food(berry)
subject: food(berry)
mode: action_bound
evidence: [memory 5, memory 8]
```

For `action_bound`, generated wording may refer only to the exact subject. For `standalone`, it may
refer only to the selected motif and evidence. Dialogue remains optional and must not delay the
embodied action.

## Save migration

Relationship triggers are persisted inside active beats, so removing broad `ActionCompleted`
requires an explicit save migration.

- Bump the save version.
- Preserve memories, beliefs, preferences, routines, relationship values, and the recent expression
  ledger.
- Cancel an in-flight legacy beat whose trigger cannot be represented exactly. Active beats are
  transient expression, not historical truth.
- Default new action relationship fields to `None` for older saves.
- Validate that a restored action-bound context matches its action subject and derivable motif.
- Exact save/reload replay must remain deterministic for new-version saves.

## Deterministic evidence setup

Targeted feel review should not depend on autonomous randomness happening to create the required
history during one recording.

Add validated current-version starting saves for expression review:

- trusted berry with two cross-day accepted-feed memories;
- mushroom grudge with preference, belief, and cross-day rejection evidence;
- familiar cave with two genuine same-slot visits and the matching routine.

These fixtures are review setup, not proof that history formation works. Keep separate headless tests
that produce each state from real simulation events and prove the derived motif matches the fixture
invariants. The feel runner may accept an `initial_save` per experience, but the shipping game must
not expose fixture injection.

Each scenario marker is valid only when its synchronized event and state contain the named motif,
subject, expression mode, and evidence. The runner should fail the experience if a declared
required motif never occurs; a marker name alone is not evidence.

## Implementation slices

### 1. Exact causality

- Replace broad action completion with exact typed subjects and outcomes.
- Bind matching food motifs to the food action timeline.
- Preserve direct outcomes, interruption, cooldowns, ledger bounds, and deterministic replay.
- Add negative tests proving every cross-subject pairing is impossible.

### 2. Presentation arbitration

- Give visual and audio cues semantic ownership and replacement rules.
- Implement trusted-food and grudge phase recipes using existing assets.
- Cancel or attenuate relationship audio when direct input supersedes it.

### 3. Familiar-place proof

- Add validated starting-save support to the feel runner.
- Record a real familiar-place beat at cave, plant, and one toy destination.
- Tune spatial orientation, settle timing, and destination-specific audio.

### 4. Native adjudication

- Run the corrected `relationship-breadth` suite with the normal debug build.
- Review full-speed video first, then hidden-label randomized motif clips.
- Cross-check only after making an unaided reading.
- Run `cargo xtask verify`; do not add a release build to the iteration loop.

Estimated effort is 8 to 12 focused hours if existing body vocabulary reads clearly, or 12 to 18
hours if one new food-anticipation effect is required.

## Acceptance

### Causality

- Berry presentation can select only berry motifs; mushroom only mushroom; ball only ball.
- No object-bound motif is selected through a subject-free completed-action trigger.
- Unrelated recall occurs only as a traced quiet-moment standalone beat.
- Food familiarity becomes visible before its authoritative consumption or rejection.

### Feel

- With labels hidden, trusted food, food grudge, and familiar place are distinguishable in three
  randomized native clips from body, gaze, movement, timing, cue, and sound.
- Trusted food never begins beneath spit, suspicion, or refusal residue.
- A mature grudge reads as recognition before it reads as ordinary rejection.
- Familiar-place recognition anchors visibly to the learned destination without UI explanation.

### Safety and continuity

- Direct player input interrupts standalone callbacks without loss or delayed payoff.
- Bound relationship context never changes authoritative food preference or action outcome.
- New saves replay exactly across save/load; legacy saves retain history and safely drop only an
  unrepresentable transient beat.
- AI-off and audio-off runs preserve every mechanically meaningful callback.
- Full headless verification remains display-, model-, GPU-, audio-, network-, and credential-free.

## Non-goals

- A new universal relationship level or progression track.
- Bespoke art for every motif before existing vocabulary is tested.
- Guaranteed callbacks on every familiar interaction or routine window.
- Ambient memories disguised as responses to player actions.
- A slower release/final build in the normal feel iteration loop.
