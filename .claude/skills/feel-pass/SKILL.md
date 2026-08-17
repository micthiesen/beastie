---
name: feel-pass
description: >
  Run Beastie's complete evidence-based game-feel workflow: experience the native game, adjudicate
  visual, interaction, behavioral, timing, audio, and emotional-readability findings, write a
  durable review and comprehensive implementation contract, print an approval-ready Codex project
  goal,
  then execute the approved goal through repeated implementation and native review until no known
  material improvements remain. Use for "feel review", "polish pass", "make it feel crafted",
  implementing a feel-review design doc, or /feel-pass.
---

# Feel Pass

Turn subjective game feel into a durable review, an approved implementation mandate, and repeated
evidence-backed improvement. Do not treat green tests or the first implementation as completion.

## Select the phase

- Use **Review and contract** when asked for a feel review, when no normative implementation doc
  exists, or when the user asks what should change. Stop after printing the proposed goal.
- Use **Implement and iterate** only after the user explicitly approves or sets the goal. If the
  user asks to implement without using a goal, still follow the whole implementation phase in the
  current turn.
- When the user approves the printed objective and asks to set it, create the Codex project goal
  using that exact objective before planning or implementation.
- Never create a Codex project goal merely because this skill triggered. The user must explicitly
  approve the printed objective first.

## Read product authority

Before reviewing or changing the game, read completely:

1. `docs/STATE.md`
2. `docs/game-design-philosophy.md`
3. `docs/feel-review-loop.md`
4. The focused design, review, and architecture docs linked from STATE for the selected experience

Preserve the simulation/presentation boundary, offline operation, graceful degradation, creature
agency, qualitative development, portability, and the absence of permanent ruin. Do not let a
lower-level plan silently redefine the product.

## Review and contract

### 1. Define the lived experience

Choose a bounded experience, not a code subsystem. State what the player should feel, which
interactions and time horizons matter, and what would make the experience feel unclear, mechanical,
unresponsive, repetitive, or emotionally false.

This bounds evidence collection and gives the review an entry point. It does not bound the later
contract or implementation pass, which includes every worthwhile improvement discovered elsewhere
in the existing game experience.

Prefer an existing focused feel suite. Add or correct deterministic scenarios when the required
experience is not represented. A marker name is not evidence that its named event happened.

### 2. Capture valid native evidence

Use the normal debug game and `cargo xtask feel`. Do not build a release or final package as part of
the iteration loop. Keep the same seed, scenario, viewport, backend, and timing for comparisons.

Reject zero-frame, incomplete, differently seeded, or causally mismatched runs. On macOS, follow
the drawable recovery guidance in `docs/feel-review-loop.md` and stop only the invalid game process.
Never turn an invalid capture into a finding.

Review the full video at normal speed before relying on traces. Inspect synchronized filmstrips,
inputs, events, state, audio, and the reference mix afterward to explain causality.

### 3. Adjudicate broadly

Use independent visual, causality/behavior, and audio/timing reviewers when subagents are available.
Give them the raw evidence bundle and review question, not expected findings. The main agent must
verify, deduplicate, and reject unsupported conclusions.

Review all of these, even when the initial complaint names only one:

- interface clarity, hierarchy, discoverability, focus, and input comfort;
- acknowledgement, anticipation, payoff, interruption, recovery, and repeated-use rhythm;
- creature agency, emotional causality, history, development, attachment, and quiet life;
- animation continuity, body/gaze/movement readability, composition, and visual craft;
- cue semantics, audio timing, overlap, ducking, silence, and graceful degradation.

Record strengths and explicit non-findings so later passes do not reopen clean areas without new
evidence. Classify missing coverage as an evidence gap, not a runtime defect.

### 4. Write durable documents

Write an observation-first review under `docs/` with timestamps, evidence paths, intended and actual
readings, severity, likely layer, proposed direction, and acceptance criteria.

Then write or revise a separate normative implementation contract when the changes require design
judgment or cross-layer work. Include:

- outcome and product rules;
- architectural decisions and rejected shortcuts;
- complete behavior/presentation/audio recipes;
- persistence and migration effects;
- deterministic fixtures and native evidence requirements;
- implementation slices used as checkpoints, not optional scope;
- acceptance criteria, non-goals, and the definition of done.

Expand the contract to include every material improvement found anywhere in the existing game
experience, whether or not it is directly required by the original review. Include cleanup,
significant rework, and authored assets when they would make the game more coherent or crafted. Do
not add unrelated roadmap features, but do not constrain the contract to the smallest patch when
broader feel work produces a meaningfully better game.

Update `docs/STATE.md` with a short pointer and make the implementation contract the recorded Next.
Run `cargo xtask verify`, then commit and push the completed review artifacts per repository policy.

### 5. Print the Codex project goal and stop

Generate one Codex project goal from the final implementation contract. Print the exact objective
in a fenced `text` block so the user can review or copy it. Do not store the goal in repository
files. Do not create the goal or begin implementation in the same phase.

Use this structure, replacing bracketed values with the actual document and experience:

```text
Use $feel-pass to fully implement the entire normative scope of [implementation-doc]. Treat the
document as the starting contract, not a ceiling. Make every worthwhile improvement to the
existing game experience that you identify during implementation, native play, evidence review,
code review, or testing, whether or not it is directly required by the original document or an
adjacent system. Include anything that materially improves the game's feel, coherence, craft,
clarity, responsiveness, creature presence, emotional readability, or maintainability. Revise the
implementation contract and its acceptance criteria whenever the evidence reveals a better or
more complete solution.

Do not stop after the listed slices, the first successful capture, or green automated tests.
Continue the implementation, normal debug build, synchronized native feel capture, full-speed
subjective review, trace adjudication, code review, and further-fix loop until the complete original
scope and every subsequently discovered worthwhile improvement are implemented; repeated review
finds no known material issue worth fixing in the existing game experience; no accepted change
weakens another feel area; cargo xtask verify passes; and [implementation-doc], the feel-review
record, and docs/STATE.md accurately describe the final verified game.

Exercise broad judgment and make substantial reworks or new authored assets when they produce the
better result. Do not add unrelated roadmap features. Preserve Beastie's product invariants and
offline behavior. Do not perform release or final builds, and do not enter marketing, storefront,
trailer, or distribution work unless the user separately requests it.
```

## Implement and iterate

### 1. Establish checkpoints

Read the active Codex project goal and the complete implementation contract. Inspect the current
code and evidence rather than assuming the plan's proposed mechanics still fit. Create a working
plan that covers every normative section, migration, fixture, presentation layer, and acceptance
criterion.

Treat worthwhile improvements discovered anywhere in the existing game experience during
implementation as in scope, even when they are not direct or adjacent consequences of the original
document. Add them to the contract and plan before implementing them so compaction cannot erase the
expanded mandate. Keep unrelated new roadmap features out of the pass.

### 2. Implement cohesive slices

Implement the clean architecture the evidence calls for, including significant rework when it is
better than accumulating patches. Preserve direct player feedback and authoritative simulation
outcomes. Add focused regression tests for causal and persistence rules.

After each coherent slice:

1. Run the smallest relevant tests and normal debug checks.
2. Review the diff for correctness and unintended behavior changes.
3. Update the plan and implementation contract with new findings or decisions.
4. Continue without asking the user to authorize ordinary in-scope fixes.

Ask the user only when a genuine product-taste decision cannot be resolved from philosophy, prior
calibration, or evidence. Present one short A/B and one specific question.

### 3. Repeat the feel loop

Run the same native scenario and compare the same-seed before and after evidence. Review full-speed
video before traces. Check the original findings, the entire rubric, and any adjacent experience
touched by the implementation.

If the intended reading is still weak, a regression appears, or a new material improvement becomes
obvious, record it, revise the contract, implement it, and recapture. Do not defer a discovered
improvement merely because it was absent from the original document.

Keep a change only when it improves the intended reading without weakening responsiveness,
clarity, creature continuity, emotional truth, accessibility, graceful degradation, or quiet
rhythm elsewhere.

### 4. Review and finish

Run independent code-review and feel-review lenses against the final candidate. Triage their raw
findings skeptically, fix everything that survives, and rerun affected evidence.

After the final material change, complete at least two valid native review passes, including a
fresh recapture wherever behavior or timing changed. Stop only when neither pass reveals a known
material improvement worth making.

Complete only when:

- every original and added acceptance criterion passes;
- repeated native review reveals no known material improvement worth making in the experience;
- every accepted change has valid comparison evidence and no unresolved high-severity regression;
- headless regression coverage and `cargo xtask verify` pass;
- visible shell changes were exercised through the normal debug native path where practical;
- the implementation contract records the final design rather than the initial proposal;
- `docs/STATE.md` records the achieved result and honest next step;
- finished work is committed and pushed per repository policy.

Do not run release/final builds as an extra confidence ritual. Do not suggest marketing,
storefront, trailer, or distribution work unless the user introduces it.
