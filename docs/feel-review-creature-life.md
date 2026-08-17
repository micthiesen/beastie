# Fifth feel review: creature life and expression

Date: 2026-08-17

This pass reviewed the complete normal debug baseline after the interaction-continuity rework. The
bounded lived experience was the first five minutes and the quiet time that follows: does the game
feel like time with one self-directed, particular fish, or like a sequence of polished command
responses? The review then checked interaction, degraded, relationship, and AI-off paths for the
same reading. No release or final package was built.

The fresh evidence root is `target/feel/fifth-round-baseline-before`. All seven experiences
completed validation with seed 42, a 640x360 presentation at 60 fps, the fixture dialogue backend
where enabled, and synchronized video, inputs, events, state, audio, reference mix, filmstrips, and
hash-pinned manifests:

- `first-five-minutes`, 5:00.383;
- `quiet-observation`, 3:00.233;
- `interaction-chain`, 1:02.650;
- `dialogue-races`, 0:36.333;
- `bad-conditions`, 0:43.417;
- `relationship-over-time`, 1:37.500;
- `relationship-over-time-no-ai`, 0:54.267.

Independent visual, causality/behavior, and audio/timing reviewers formed observations from the
continuous footage and reference mix before using the synchronized traces to explain them. The
previous pass's ownership fixes remain strong. The new findings converge on a different problem:
the simulation contains preferences, places, memories, initiative, and truthful action ownership,
but presentation repeatedly compresses that breadth into the same toy commute, generic heart
callback, generic timing grid, or identity-breaking talking body.

## Main findings

### Hungry speech still changes Mop into a different creature

- Sessions: `relationship-over-time` and `relationship-over-time-no-ai`.
- Evidence: `relationship-over-time` `01:24.750` to `01:31.750`, especially talk acceptance at
  `01:25.366` and speech around `01:26`; `relationship-over-time-no-ai` `00:46.000` to `00:53.000`,
  including the mature player-return act at `00:47.133` to `00:51.133`. The complete eighteen-frame
  talking contact sheet was also inspected from the checked assets.
- Observation: when hungry Mop speaks, the gold fish becomes a smaller brown creature with a curled
  tail, paw-like lower limbs, different body mass, and different facial construction. It snaps back
  to the canonical fish when the following comfort or ordinary body recipe takes ownership.
- Intended reading: the same particular fish speaks while hunger affects expression.
- Actual reading: speech or an authored return line replaces the protagonist with another species.
- Cause: `body_sprite` selects `creature-v1/talk/hungry-south` whenever dialogue is active in the
  hungry mood. Those three generated frames were promoted as identity-preserving, but visual review
  disproves that provenance claim. Other talk moods also vary anatomy and construction enough to
  require a complete identity pass rather than a hungry-only patch.
- Severity: **high**. The failure appears in both voiced and AI-off relationship play, so it breaks
  the particular-creature promise rather than only one optional worker path.
- Acceptance: all eighteen mood-and-mouth frames preserve the canonical fish's palette, fins, tail,
  body mass, scale, centroid, eye construction, and transition anchors at native 2x scale. Hungry
  speech, AI-off initiated speech, and every mood transition remain recognizably the same fish.

### Quiet life collapses into a preferred-sock demonstration loop

- Sessions: `quiet-observation` and `first-five-minutes`.
- Evidence: the five quiet bouts accept/contact the sock at `00:01.033`/`00:09.033`,
  `00:30.033`/`00:41.050`, `01:10.066`/`01:21.066`, `01:40.083`/`01:49.083`, and
  `02:25.100`/`02:34.116`. `filmstrips/overview-001.png` through `overview-008.png` show the
  session-wide recurrence. First-five contains ten toy payoffs, eight to the sock; after the last
  authored player/UI input at `01:08.266`, most visible initiative is another toy approach and play
  payoff.
- Observation: core also visits the cave and plant, but those visits read as ordinary swimming and
  hovering. The only strongly performed autonomous activity is repeated sock play, so the lived
  session becomes the same commute, right-edge pose, impact, heart, and recovery every 30 to 45
  seconds.
- Intended reading: a creature with a strong preference embedded in varied private life.
- Actual reading: a timer repeatedly selects the highest-weight showcase animation.
- Cause: idle selection can choose several destinations, but only toy arrival has a distinct
  physical payoff. Preference and short settlement windows repeatedly return to the same toy, while
  the repetition guard prevents only an immediate identical destination and does not reason about
  performed activity, recipe, subject, or recent session rhythm.
- Severity: **high** for agency, attachment, and the game's central quiet-life promise.
- Acceptance: an identical-seed three-minute quiet run contains materially varied, state-grounded
  private activities and payoff contours. A preference may recur, but it does not dominate the
  session or form a metronomic cluster. Cave, plant, bottom, water, and toy behavior are visibly
  distinct without relying on the status label.

### Autonomous toy play is not physically legible without its label and heart

- Sessions: `quiet-observation` and `first-five-minutes`.
- Evidence: quiet payoffs at `00:09.033`, `00:41.050`, `01:21.066`, `01:49.083`, and
  `02:34.116`; synchronized overview filmstrips and `object/toy-impact` audio entries.
- Observation: Mop reaches the sock, but the sock does not visibly move, compress, trail, get
  carried, or otherwise participate. Mop uses nearly the same open-mouth pose at the same edge, a
  heart appears, and the status changes to `playing`. The UI and generic affection punctuation do
  most of the semantic work.
- Intended reading: Mop physically does a favorite, object-specific thing with the sock.
- Actual reading: a generic positive state triggers beside a static prop.
- Likely layer: authoritative object/activity semantics plus presentation and authored animation.
- Severity: **medium-high**. The payoff is truthful, but it does not feel embodied.
- Acceptance: ball, bell, and sock have distinct contact and recovery reads; the exact object visibly
  participates. Any durable displacement belongs to simulation state. Presentation-only squash,
  recoil, or particles may embellish but cannot invent a canonical move.

### Shared history repeats as generic affection punctuation

- Sessions: `first-five-minutes`, `interaction-chain`, `dialogue-races`, and relationship runs.
- Evidence: first-five's shared-ball callbacks at `01:40.266` to `01:47.266` and `03:44.266` to
  `03:51.266` use the same exact memory ID, motif, subject, notice, anticipate, act, recover timing,
  curious cue, smile, heart, and reset. Similar comfort callbacks begin at interaction-chain
  `00:48.233` and dialogue-races `00:23.083` with the same curious sample and seven-second grid.
- Observation: traces identify a ball memory or comfort ritual, but the body does not orient toward
  the ball, approach the remembered place, seek the player, or perform a motif-specific act. The
  visible and audible result is interchangeable affection punctuation. The same ball callback
  repeats 124 seconds later, then immediately hands off to another sock trip.
- Intended reading: a specific memory or ritual surfaces from Mop's ongoing life.
- Actual reading: a delayed receipt confirms that a prior interaction was stored.
- Cause: grounded motif selection is exact, but most motifs share a fixed phase grid, generic body
  recipes, and a narrow cue vocabulary. The repetition ledger allows the same evidence and recipe
  to recur soon enough that the callback reveals the director.
- Severity: **medium-high** for development and attachment.
- Acceptance: motif families have semantically distinct embodied and sonic recipes, phase timing,
  environmental targets, and recovery. The same evidence cannot produce the same short callback
  twice in one five-minute normal-speed session. Sparse history expression remains interruptible
  and does not become exposition or constant activity.

### Microphone startup failure receives a false creature-hearing cue

- Session: `bad-conditions`.
- Evidence: `speech_started` at `00:36.300` immediately projects `creature/curious`;
  `speech_failed(microphone_unavailable)` follows at `00:36.316`, with the technical status
  “Microphone unavailable. Text still works.”
- Observation: no microphone capture began and no audio could have been perceived, yet Mop audibly
  acknowledges hearing the player.
- Intended reading: technical capture failure remains technical and the creature does not react to
  evidence it never received.
- Actual reading: pressing the input control is treated as creature perception before the device is
  known to exist.
- Cause: the shell submits `SpeechStarted` before `MicrophoneCapture::start`; core truthfully emits
  `SpeechPerceived`, and view truthfully turns that false premise into a curious cue.
- Severity: **medium** because it violates the explicit perception-is-not-truth and technical-failure
  boundary.
- Acceptance: microphone acquisition failure produces only technical UI feedback and no gaze,
  attention, interruption, creature cue, or canonical perception event. Successful capture retains
  immediate embodied acknowledgement.

### Deferred speech can interrupt the action it promised to wait through

- Sessions: `interaction-chain` and `bad-conditions`.
- Evidence: interaction-chain accepts ball play at `00:20.600`, glances at speech at `00:21.633`,
  reports `utterance_deferred` at `00:22.366`, then accepts talk at `00:24.166` and interrupts the
  ball before contact. In bad-conditions, the analogous deferred turn at `00:26.266` allows ball
  contact at `00:29.266` and accepts talk at `00:33.266`.
- Observation: “Heard you. Waiting for a good moment...” sometimes means waiting for the current
  payoff and sometimes means aborting it after a short timer. No embodied change-of-mind explains
  the interruption.
- Intended reading: Mop heard the player, chose to finish what it was doing, then responded at a
  legible semantic boundary.
- Actual reading: a fixed delay can override the occupied behavior it named as the reason to wait.
- Severity: **medium**. Ownership remains truthful, but willingness and agency are inconsistent.
- Acceptance: deferred language waits for an explicit safe boundary in the owned activity, expires
  honestly, or interrupts through a visible simulation-owned change of mind. It never silently
  converts “waiting” into a timer-driven abort.

## What already feels strong

- Direct food remains the strongest embodied sequence: same-frame receipt, braking, gaze, turn,
  approach, inspect, eat, cue, and recovery remain continuous and exact.
- Rejected bell offers at `00:27.400` and `00:30.433` sustain refusal and never become contact,
  impact, delight, history, favorite evidence, or relationship reward.
- Pre-reply, queued-TTS, and active-playback supersession remain clean. No canceled turn regains
  caption, voice, mouth, status, or body ownership.
- The curated delight, affection, and comfort clips remain the same gold fish, including AI-off
  relationship play. The regression is in the separate talking family.
- Technical statuses clearly distinguish AI, recognizer, and microphone failure from creature
  refusal. AI-off movement, needs, relationships, and initiative remain mechanically complete.
- Aquarium composition, mixed-case text, icon deck, speech-card placement, settings, ambience,
  ducking, and reference-mix headroom do not support a global UI, scene, or mix rewrite.

## Evidence gaps and rejected conclusions

- First-five and quiet-observation intentionally share seed 42. Their within-session repetition is
  sufficient for the finding, but cross-seed and temperament breadth must be added before choosing
  exact selection weights.
- The relationship fixtures accelerate days. They prove state-dependent selection and presentation,
  not the organic pacing of a real multi-day attachment.
- The fixture dialogue backend falls into the authored technical fallback during nominal AI-on
  first-five and relationship sessions. This is a healthy-dialogue evidence gap, not proof that the
  selected Qwen runtime normally fails. The next pass must provide a valid non-fallback fixture and
  retain separate degraded evidence.
- Semantic replay cannot prove unbriefed icon discoverability, pointer target comfort, physical
  controller feel, microphone hardware latency, or host-speaker timbre.
- No finding supports reopening direct food timing, rejection causality, global dialogue ownership,
  the aquarium composition, or the accepted technical-status language.

## Recommended next pass

Implement [creature-life-expression-rework.md](creature-life-expression-rework.md) as one cohesive
pass. Give private life typed activities and repetition-aware selection, make places and toys
physically distinct, author motif-specific history performances, replace every identity-drifting
talk frame, and move creature perception after successful microphone acquisition. Make deferred
speech wait for semantic boundaries rather than a fixed grace period.

The acceptance loop uses normal debug builds and the same seed-42 baseline, plus cross-seed quiet
and all-mood talking galleries. It must retain the current direct food, refusal, dialogue
supersession, AI-off completeness, graceful degradation, UI hierarchy, and quiet space. Release or
final builds are outside this loop.
