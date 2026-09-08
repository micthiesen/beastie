# Creature presence and care

Status: implementation authorized on 2026-09-08; integrated native verification in progress.

Date: 2026-09-08.

Evidence: [holistic feel review](feel-review-holistic-20260908.md).

## Outcome

Make ordinary time in the aquarium feel like caring for a particular animal. The player should
recognize opportunities to touch, feed and play before needing to decode the interface. Objects
should look as though they belong in the same physical habitat as Mop. Voice should explain its
operation, and the sound bed should leave foreground expression room without responding to its
own decorative accents.

Keep the current creature identity, restrained water palette, readable typography, compact rail,
ordinary-GPU geometry renderer and simulation-owned independent life. This is a craft and
interaction pass across the existing experience. It does not introduce another game system.

## Product rules

- The simulation owns choices, contact, refusal, outcomes and history. Presentation cannot move an
  authoritative target independently to make a screenshot more attractive.
- Care remains immediate in receipt and creature-paced in outcome. Do not shorten valid travel
  merely to make every command resolve quickly, or turn acknowledgement into promised compliance.
- Voice and text use the same creature semantics. Both remain optional, and text remains complete.
- No hidden microphone activation, automatic consent, cloud service or background listening.
- Preserve qualitative development, sparse callbacks, absence recovery and identity across brains.
- Keep the world alive while composing, viewing settings and receiving dialogue. Preserve safe
  cancellation, input editing, explicit reset confirmation and graceful silence.

## 1. Discoverable care and a quieter language surface

### Entry and repeated use

The opening screen must communicate that the creature and toys can be interacted with. Use one
short, unobtrusive contextual invitation, such as “Select Mop or a toy,” and meaningful hover/focus
descriptions such as “Mop · care and play” and “Bell · play.” Wording must describe available
interaction without implying that Mop will comply. Do not add a tutorial modal, checklist, quest,
need meter or persistent attention demand.

Give embodied care at least the same discoverability as the empty message field. Rebalance the
rail and reduce the resting emphasis of the empty compose surface; preserve immediate access,
comfortable text capacity and full-size text when focused. A settled, unfocused empty field should
not remain the strongest invitation in several minutes of quiet life. Do not hide text behind
multiple steps, steal keyboard focus, discard an unfinished message or move hit targets while the
player is using them.

Dismiss first-use guidance after a successful relevant interaction, with instructions still
available through hover/focus or the existing controls surface. If persistence is warranted,
store only a presentation preference in versioned settings, never creature learning or history.

### Shallow interaction surfaces

Care and toy choice should feel connected to the selected subject. Keep the current stationary
context placement and face clearance, and tune unnecessary empty panel space within that rule.
Normal and Large text must retain readable labels and generous hit regions. Do not trade legibility
for smaller panels, add a tooltip to every already explicit button or make panels chase the animal.

### Acceptance

- A native first-use sequence records creature/toy hover, context opening, comfort, food selection
  and drop, toy selection and return to quiet life without privileged semantic commands.
- Pointer, keyboard and controller focus expose equivalent action meaning. Physical controller
  comfort remains a separate claim unless an actual controller is available.
- Empty, focused, edited, pending and restored compose states all remain usable at Normal/Large.
- A five-minute quiet comparison shows reduced language-surface emphasis without increasing idle
  labels, notifications or creature activity simply to fill space.
- Unaided human discovery remains an explicit acceptance question; an agent that already knows the
  controls cannot certify a first-time player's discovery rate.

## 2. Voice instructions and recovery

Treat the microphone as a stateful control whose short title alone cannot explain its operation.

| State | Required visible or discoverable meaning | Input behavior |
|---|---|---|
| Disabled | Microphone off; enable in Settings, Sound & speech | No capture |
| Ready | Hold to talk | Down begins capture; release ends it |
| Listening | Listening; release to send | Receipt stays distinct from understanding |
| Recognizing | Working out what you said | Do not promise a verbal reply |
| Unavailable/error | Technical reason and a practical recovery route; text still works | No false recording state |

Provide a bounded helper or accessible state description on pointer hover and keyboard/controller
focus, including disabled controls. If disabled actions are skipped by focus navigation, expose
the same recovery information on a reachable companion affordance. Do not enable an unavailable
action just to make its explanation reachable. Preserve the existing listening/recognizing
feedback, uncertainty handling and deferred-attention semantics.

Acceptance includes actual disabled hover/click and enabled hold/release input. Pure view tests
must prove state explanations, focus reachability and Large text bounds. Real microphone capture
is required to claim device operation; fixture speech is sufficient only for the semantic states.

## 3. Belongings and shelter fit

### Toys

Replace the uniform midwater display row with a deliberately composed set of belongings. Preserve
recognizable ball, bell and sock silhouettes, but give each a clear physical reading: a buoyant
ball, a bell with a credible resting or supported form, and soft cloth with a convincing relaxed
pose. Resting objects should not all share the same height, spacing and visual support treatment.

Resolve physical placement and visual form together. Any changed meaningful location belongs in
canonical state and must agree with movement, contact, picking, saves and object identity. Small
decorative sway may remain presentation-only, but it cannot imply an unsupported collision,
attachment, carrying action or payoff. Do not add general water physics or 3D navigation to solve
an authored composition problem.

At notice, attention should locate the exact object. At approach, movement should visibly lead to
it. At contact, object motion and sound must use the existing authoritative payoff. Recovery
should return the object to a coherent resting or supported state without snapping to a newly
invented default. Ball movement, bell response, sock carrying/tug and refusal all require review.

### Shelter

Author the shelter opening and the existing rest/peek pose as one composition. During settlement,
the head and crown need clear space inside or in front of the opening, with a deliberate depth
relationship to the arch. The body must remain continuous and plausibly connected to the head.
Do not mask an intersection with darkness, hide the face, flatten the creature or disable its
existing rest behavior. Adjust local shelter geometry and presentation pose before considering
simulation changes; preserve the canonical destination and authoritative arrival requirement.

Review the whole approach, settle, idle hold and departure, including head turns and neighboring
plants. A single clean rest screenshot cannot close the finding. Keep exact presented-transform
picking, actual leaf gaps and nearest-hit ordering.

### Acceptance and persistence

Implementation decision: new aquarium defaults are Ball `(4800, 7650)`, Bell `(6650, 8700)` and
Sock `(8250, 10000)` in both object and mutable-toy state. These are constructor defaults only;
there is no save migration or schema bump. Existing saved positions and in-flight ownership are
retained. The bell carries its own cork float at every position, the ball reads as buoyant, and
the bent cloth can remain suspended at a saved midwater anchor without implying an attachment.
New cloth placement brings its lower fold close to the substrate. Shelter geometry extends
behind the interaction plane and clears the existing head/crown without changing the rest target.

- Same-seed first-five-minutes and quiet-observation comparisons show coherent resting objects
  and shelter occupation at 1280x720, including the original 03:00 shelter frame.
- An additional quiet seed and the fixture-backed familiar-cave and shared-ball experiences
  cover other approaches and relationship-driven uses of the same objects.
- Actual pointer selection succeeds on the final visible body, bell loop and scenery gaps.
- Food offered during toy approach/contact preserves interruption and the exact payoff owner.
- Save/load during carried sock and moving ball preserves motion and ownership; visual settling
  must never normalize an active canonical interaction into a resting object.
- Existing saves keep object identity, preferences, memories, learned places and interaction
  ownership. Do not silently rearrange a player's saved habitat for aesthetic uniformity.
- No save bump is needed for pure geometry/pose changes. If canonical resting positions or state
  semantics must change, document the exact versioned migration before implementing it; preserve
  meaningful history and test in-flight and settled save/load. New defaults alone do not fix old
  saves, so existing placements need a coherent supported visual treatment too. Any authored
  support must remain coherent at legacy positions without inventing a mechanical attachment.

## 4. Sound priority that follows meaning

Give playback an explicit semantic mix role. Separate ambience/decorative bubbles, interface,
physical object effects, creature vocalization and speech. Use existing typed cue identity or a
central classification rather than divergent ad hoc decisions in runtime and the review harness.

- The underwater bed stays at its ordinary level during isolated bubbles, UI selection and
  physical effects. These do not acquire the creature-vocalization duck merely by being one-shots.
- Creature vocalizations duck the bed by 4 dB with the authored 35 ms attack and 180 ms release.
- Speech takes priority with the authored 7 dB bed duck, 45 ms attack and 260 ms release. Apply the
  5 dB creature-cue attenuation by semantic role; document any deliberate physical/UI attenuation.
- Preserve movement rate limits, bubble overlap bounds, direct-outcome priority, per-owner
  cancellation and immediate care receipt. Do not add new chirps for every UI explanation.
- Returning to silence or an unavailable audio device never affects simulation or input.

Tests should cover isolated bubbles/UI/physical effects, vocalization, overlapping roles,
interrupted speech and envelope recovery. A repeated update must not extend ducking merely because
a discarded cue was queued. Review normal-speed reference sound after the fidelity work below,
and claim host sound quality only after a listening check.

Correct `docs/audio-direction.md` to describe the current Bevy shell and optional rodio output.
Remove stale ggez and decode-once claims unless implementation actually establishes the latter.

## 5. Trustworthy review evidence

The existing reference mix is a cue schedule, not playback reconstruction. Replace it with a
deterministic offline rendering of recorded playback decisions. Record enough information after
arbitration to distinguish requested, discarded, started, cancelled and completed cues; include
stable playback/owner identity, source asset hash, relevant gain/settings and envelope timing.
Keep player text and microphone recordings out of these privacy-safe traces. Retain the resolved
authored sound and TTS bytes in the bundle, or an immutable content-addressed source that the
bundle can resolve offline; a mutable path and hash alone cannot reconstruct replaced assets.

Reconstruction must honor actual accepted lifetimes, stop/cancel ownership, silence/settings,
one-shot suppression and gain/duck envelopes. Record the asset actually resolved by runtime,
rather than re-resolving a potentially changed asset later. Do not call the result captured host
audio. When output is unavailable, distinguish an intentional hypothetical renderer from an
observed playback reconstruction instead of silently inventing audible output.

Keep limiter/clipping treatment explicit. Retain pre-limiter peak measurements or reject clipping
so a safety limiter cannot conceal a bad gain decision during review. Measure reconstruction
against recorded commands with display-free fixtures that include cancellation, mute, ducking,
overlap and missing output/assets.

Review scenarios must assert the outcomes their labels claim. Extend bounded outcome checks where
needed for refusal, repeated refusal, deferred attention, supersession and contact. Preserve
existing required-motif checks. A valid-duration video is insufficient if its named causal event
never occurred. Retain a native first-use path because semantic menu opening cannot establish
discoverability or input comfort.

## 6. Deferred language must respect the current direct interaction

When an utterance deferred behind one owner becomes eligible, replacement of that owner is not
permission to interrupt a newer direct action that has not reached its own safe boundary. Preserve
the current refusal/accepted toy owner, destination, approach and payoff semantics. A refusal
must not be reduced to a lingering suspicious face because older dialogue cleared its body action.

Re-evaluate the current typed handoff before submitting deferred words. Distinguish successful
completion, interruption, owner replacement and safe boundaries rather than letting inequality of
owner IDs authorize delivery. Keep bounded expiry, avoid indefinite capture by unrelated private
life, and preserve the established liveness rule for cooldown-only deferral. New direct care may
still interrupt immediately. Do not make the model decide scheduling or add a global speech delay.

Acceptance reproduces interaction-chain at `00:27.400` and its repeated refusal. Require retained
refusal approach until its safe boundary, one rejection, no positive memory and eventual delivery
or honest expiry of the waiting utterance. Cover replacement by accepted play, refusal and food;
typed/spoken semantic parity; repeated replacement and expiry; new direct care; save/load of
authoritative in-flight ownership. The current ephemeral utterance need not be newly persisted.
Pure tests must fail against the old owner-inequality shortcut. Native review must confirm the
resulting body action, not only preservation of the suspicion cue.

## Implementation checkpoints

1. Establish outcome assertions and faithful audio reconstruction; retain the old native evidence
   as a visual/causal baseline with its documented audio limitations.
2. Correct deferred-input handoff and replay the exact refusal race with new-care interruption.
3. Restore microphone explanation and care discoverability; compare Normal/Large and real input.
4. Reauthor belongings and shelter fit; verify object contact, saves and native picking.
5. Correct semantic ducking and review the reconstructed sound with interruption and silence.
6. Run the entire baseline, targeted relationship breadth and a second quiet seed. Reassess all
   rubric areas, including emotional causality and history without language, after integration.
   Include a matching subtitles-off, no-generated-language/TTS relationship sequence for a truly
   nonverbal reading: the existing no-AI scenario still displays deterministic fallback captions.

These are checkpoints within one contract, not optional independent deliverables. Revise this
contract for material findings discovered in the existing experience during implementation.

## Definition of done

Every accepted finding in the review has a linked same-seed comparison and explicit disposition.
No change weakens creature continuity, direct input, text access, accessibility, offline operation
or truthful outcomes. Independent code and visual/causal/audio review has been adjudicated, and
at least two valid final native review passes reveal no known material improvement worth making.
Do not count unavailable human listening or full-speed perception as completed acceptance.

`cargo xtask verify` passes, and visible shell changes run through `cargo xtask dev --fake-ai` plus
the synchronized feel loop. Update this contract, the review and `docs/STATE.md` to describe the
verified result; commit and push per project policy. Do not build release binaries, installers or
model bundles, add roadmap systems, or begin distribution/marketing work.
