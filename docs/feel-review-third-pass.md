# Third feel review: relationship breadth

Date: 2026-08-17

This pass reviews the first simulation-owned relationship-expression implementation described in
[relationship-expression-design.md](relationship-expression-design.md). The complete native 60 fps
bundle is retained locally at
`target/feel/relationship-breadth-20260817-native/relationship-breadth`. The review combined the
real-time video, marker and overview filmstrips, input/event/state traces, reference audio mix, and
three independent visual, causal, and audio readings.

The run used a normal debug game binary. No release or final package was built.

A follow-up capture with corrected markers did not acquire a macOS game window after an unrelated
authorization dialog took focus. It wrote zero frames, was stopped, and is not used as evidence.
The checked-in scenario corrections are therefore acceptance setup for the next valid native run,
not claimed proof from this pass.

## Main finding

### Relationship callbacks are grounded in history but not always in the present moment

- Evidence: after berry consumption at `01:51.233`, the next relationship beat is
  `shared_toy(ball)` at `01:52.233`. After mushroom rejection at `02:14.300`, the next beat is
  `trusted_food(berry)` at `02:15.300`, triggered by completion of the mushroom action.
- Observation: both selected motifs have valid historical evidence, but neither is relevant to the
  object or action the player just saw. The second sequence is especially confusing: the creature
  rejects a mushroom, retains its spit/frown presentation, then begins an invisible callback about
  trusting a berry.
- Intended reading: the creature recognizes the specific familiar food or toy currently present.
- Actual reading: an unrelated memory appears to change its mind or redirect its attention for no
  visible reason.
- Cause: object motifs accept the broad `ActionCompleted` trigger, whose low generic relevance can
  still beat every other eligible candidate. The selected beat then applies its own destination and
  intention, even when the triggering object is different.
- Priority: **high**. This breaks the perceived causality that makes remembered behavior believable.

The fix should be structural rather than a score adjustment:

1. Object-bound motifs should require an exact semantic match. Trusted food and food grudges react
   to that food; shared-toy callbacks react to that toy. A generic completed action should not
   select a different object motif.
2. Ambient recollection should be a separate, deliberately scheduled trigger. It may choose among
   old memories, but it should not masquerade as feedback for the player's current action.
3. A relationship beat should retain the triggering subject through selection, pose, presentation,
   audio, and trace so each layer can be checked against the same fact.

Acceptance: in an exact replay, berry input can select only berry-linked food history, mushroom
input can select only mushroom-linked food history, and ball input can select only ball-linked toy
history. Unrelated memories may surface only during a separately traced ambient beat.

## Supporting findings

### Mature motifs share too much presentation vocabulary

- Evidence: all eight relationship starts in the reviewed run use `creature/curious`, including
  shared toy, food grudge, trusted food, and player return. Trusted food and familiar place map to
  the generic visual `Notice` cue through every expression strength. The grudge gains a spit cue
  only during its mature act phase, which this run never selected.
- Observation: movement targets differ in the trace, but at native scale most callbacks begin with
  the same eyes, punctuation, and sound. Trusted food is particularly weak because “curious” reads
  as novelty, not established confidence.
- Priority: **medium-high**, after causal selection is corrected.
- Proposed tuning: reuse existing authored vocabulary first. Trusted food should have a warm pause,
  food-directed gaze, and positive anticipation sound; a grudge should brake and recognize before
  the normal rejection; familiar cave behavior should orient to the cave and resolve with the
  existing settle sound. Do not stack full-level rejection sounds.
- Acceptance: with trace labels hidden, a reviewer can distinguish trusted food, a food grudge, and
  a familiar place from body, gaze, movement, timing, and sound in three randomized clips.

### A new relationship beat does not preempt contradictory residue

- Evidence: `food/spit-reject` begins at `02:14.300`. The misdirected trusted-food beat starts at
  `02:15.300`, but the visible spit cue remains until `02:16.283`; its generic notice cue does not
  take over until `02:16.300`.
- Observation: for the first second of “trust,” the creature still looks like it is rejecting food.
- Priority: **medium**. Correct selection will remove the worst instance, but presentation priority
  still needs an explicit rule.
- Proposed change: a newly accepted high-level beat should clear or replace incompatible terminal
  reaction residue. Preserve compatible overlap only when it has an authored meaning.
- Acceptance: no positive relationship beat begins under spit, suspicion, or refusal residue.

### Familiar-place behavior was not reviewable in this run

- Evidence: no `familiar_place` beat occurs. The final state has no routines and only one genuine
  cave arrival. The old `familiar-place` markers actually surrounded a player-return beat and plain
  idle movement.
- Disposition: **evidence gap, not a claimed runtime defect**. The scenario markers were renamed to
  state what they really capture, and direct food-trigger checkpoints were added for future runs.
  A future targeted fixture must establish two genuine same-slot visits across active days before
  judging familiar-place presentation.
- Acceptance for the fixture: the marker is valid only when the synchronized event contains
  `relationship_beat_started` with `motif.kind = familiar_place` and the final state contains the
  matching routine and visit evidence.

## What already feels strong

- Direct food, toy, and return actions remain crisp and readable at native 2x scale. The authored
  gaze, wide-eye attention, frown, refusal, approach, and recovery poses carry the basic action
  grammar without text.
- Relationship beats are safely interruptible. Direct player input preempts them without being
  lost, and the active beat clears rather than leaving the creature stuck.
- Player-return history genuinely matures from notice to anticipation as evidence accumulates.
- The bounded expression ledger remains coherent after four active days, with no permanent active
  callback or repetition loop.
- Audio timing, ambience spacing, and duck attack/release remain synchronized and restrained. The
  reference mix proves cue identity and timing, not host-speaker timbre.

## Recommended next pass

Rework relationship trigger relevance around exact semantic subjects, then tune the three breadth
motifs using existing body and audio vocabulary. Keep this as one coherent pass because selection,
pose, cue priority, and sound all need to agree on the same triggering object. Add a deterministic
familiar-place setup before claiming that motif is visually proven.

The corrected `relationship-breadth` feel suite is the acceptance loop. Run it through the normal
debug build, inspect the three direct-trigger filmstrips with labels hidden, cross-check their
events and state, and finish with `cargo xtask verify`. A release build is not part of this loop.
