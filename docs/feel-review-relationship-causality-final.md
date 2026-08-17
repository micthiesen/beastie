# Relationship causality implementation review

Date: 2026-08-17

This review closes the exact-subject relationship rework in
[relationship-causality-rework.md](relationship-causality-rework.md). The final candidate was
reviewed through the normal debug game at 640x360 and 60 fps. No release or final package was
built.

Two complete post-gameplay-change evidence sets were accepted:

- `target/feel/relationship-causality-clean1/`, one validated five-experience suite;
- `target/feel/relationship-clean-a-{trusted,grudge,cave2,plant,ball}/`, five independently
  validated native launches used to isolate macOS AppKit lifecycles.

Each bundle contains the full video, dense marker filmstrip, synchronized input/event/state/audio
traces, reference mix, waveform, hashes, and generated review scaffold. Earlier partial or timed-out
directories are invalid and were not used.

## Final reading

### Trusted berry

- Evidence: recognition begins at `00:00.000`; consumption and direct eat audio occur at
  `00:10.016`; the action-bound context completes at `00:11.016`.
- Reading: Mop pauses with a warm side-facing expression and exact food gaze, then returns to the
  ordinary turn, swim, inspect, eat, and content recovery grammar. The soft `creature/mrr` is
  restrained and does not stack with the full eat cue.
- Result: the history is visible before the outcome without changing or delaying the outcome.

### Mushroom grudge

- Evidence: resentful recognition begins at `00:00.000`; authoritative rejection and the single
  `food/spit-reject` cue occur at `00:09.016`; the bound context completes at `00:10.016`.
- Reading: the first second is unmistakably wary rather than curious. Mop then approaches through
  the ordinary physical action before the direct rejection takes ownership. The pre-cue is
  cancelled, no second full-level rejection sound stacks, and no relationship cue remains after
  recovery.
- Result: this reads as remembering the exact disliked food before rejecting it, not as an
  unrelated memory or a manufactured outcome.

### Familiar cave, plant, and ball

- Evidence: each beat starts at `00:00.983` and reaches Act at `00:04.000`. At Act, the synchronized
  state proves exact gaze, hover steering, matching `last_arrived_destination`, and an unexpired
  settlement interval. Cave alone plays `environment/cave-settle`; plant remains quiet; ball uses
  ordinary authoritative toy impacts when physical contact occurs.
- Reading: the callbacks are spatial and quiet. Cave and plant resolve beside their distinct
  landmarks; ball familiarity naturally becomes play. None needs explanatory UI text or generic
  creature speech.
- Result: familiar-place expression is grounded in a current arrival, not stale visit history.

## Broad regression review

- Direct food and toy receipts remain immediate and higher priority than relationship expression.
- UI and underwater ambience survive creature-channel cancellation.
- Trusted and grudge contexts retain semantic subject and outcome after the food object disappears.
- Standalone callbacks clear on completion and remain interruptible by direct input, speech
  attention, sleep, and urgent needs.
- Dialogue now receives the same exact motif, subject, mode, action identity, and evidence as the
  active embodied context. AI-off play remains mechanically complete.
- Save version 5 preserves history and bounded ledgers, defaults new action fields, and clears only
  unrepresentable legacy transient beats.

No remaining material issue was found in the reviewed relationship breadth. Ambient particle phase
can differ between native launches because presentation time is real, but authoritative movement,
relationship timing, outcomes, and owned audio traces remain deterministic.

## Evidence-runner limitation

Repeated ggez/AppKit window lifecycles can nondeterministically stall in Metal drawable acquisition
on macOS. The runner now fails closed on update errors, early video, or a scenario-length timeout,
and `--experience <id>` supports isolated validated launches. A stalled or partial directory is not
evidence. This affects capture orchestration only; valid recordings and headless behavior are not
affected.

## Acceptance result

The exact-subject causality, action-bound and standalone ownership, save migration, dialogue
projection, visual/audio recipes, fixture grounding, and native acceptance requirements in the
implementation contract are complete. The headless gate remains the final repository-wide check.
