# Baseline feel review

Date: 2026-08-16

This review is the first full use of the repeatable loop in
[feel-review-loop.md](feel-review-loop.md). The synchronized evidence lives under
`target/feel/baseline-before` and `target/feel/baseline-after`. The binary hashes in each manifest,
not the mutable working tree name, identify the executable that produced a run.

## Applied findings

### Quiet life collapsed into one endless approach

- Before evidence: `quiet-observation`, 00:00.000 to 03:00.133.
- Observation: 179 of 181 one-second samples reported `approach`; the mean position was
  `(1662, 8339)`, near the cave in the bottom-left. The overview filmstrips read as repeated travel
  to essentially the same spot rather than a creature settling, choosing, and changing its mind.
- Change: genuine arrivals now create deterministic 4 to 10 second hover bouts, avoid an immediate
  destination repeat, vary identity-shaped destinations, and record only real visits. Repeated
  cross-day visits can form a bounded routine.
- Acceptance: the same-seed after run must contain several destinations and visible settled bouts,
  without manufacturing hundreds of favorite visits.

### Relationship play was both invisible and capable of corrupting the save

- Before evidence: `relationship-over-time`, 00:00.000 to 00:48.133. The experience terminated at
  the second day with `session state became invalid: need, relationship, or habit values must be
  within 0..=1`.
- Observation: comfort did not create a memory or relationship change, accepted care barely
  distinguished a developed relationship, and an out-of-range relationship delta made a direct
  save invalid.
- Change: comfort, accepted food, and accepted play create bounded bond/trust recovery and history;
  disliked repetition raises resentment; every update clamps immediately. Comfort has a seven
  second embodied affection aftermath. Active sleep restores energy and wakes, and stale
  initiatives clear or expire.
- Acceptance: the identical relationship experience completes, remains save-valid, and its trace
  shows bounded relationship/history changes plus a return from affection to independent life.

### Half the swim cycle popped back to the static creature

- Before evidence: repeated swim passages throughout `first-five-minutes` and
  `quiet-observation`; the discontinuity is clearest in dense movement filmstrips.
- Observation: the view emitted frames 0 through 7, but the renderer loaded only 0 through 3.
  Frames 4 through 7 fell through to the unframed base asset, so every cycle visibly snapped.
- Change: the asset catalog loads all eight authored frames.
- Acceptance: the after filmstrip retains one coherent silhouette through complete swim cycles.

### Spoken and typed input could look lost or technically fictionalized

- Before evidence: `interaction-chain`, 00:21.616 to 01:00.450, and `bad-conditions`,
  00:03.016 to 00:43.416.
- Observation: “Heard you. Waiting for a good moment...” remained after submission and subsequent
  replies. Acoustic uncertainty and ignored typed input had no durable explanation. A missing local
  model displayed only creature prose, making software fallback indistinguishable from personality.
- Change: spoken statuses have explicit terminal handling and expiry; submission clears the deferred
  message; live microphone feedback outranks transcript notices; ignored text gets a short receipt;
  technical dialogue fallback is identified as technical while the authored reply remains playable.
- Acceptance: the after difficult-conditions run never claims to be waiting after submission, and
  every uncertainty or infrastructure branch shows a bounded recovery message.

### Audio outcomes diverged from their semantic events

- Before evidence: the `bad-conditions` and `interaction-chain` audio traces.
- Observation: paired refusal events stacked unrelated sounds, aquarium capacity rejection used the
  tasted-food spit, accepted toy play had no outcome cue, and all remaining nonverbal acts fell back
  to cave-settle. Detached one-shots made ambience duck for only the enqueue frame.
- Change: `beastie-view` now emits the runtime semantic audio plan. Paired events coalesce; capacity,
  food, toy, affection, speech attention, sleep, and refusal resolve to their authored cues. Active
  one-shots are retained, swim is rate-limited and quieter, TTS ducks creature cues, and ambience
  uses the authored attack/release envelopes for the full active duration.
- Acceptance: the after trace shows one cue per semantic rejection and continuous duck gain while a
  one-shot or speech player is active. The synchronized reference mix contains the expected cue.

### Presentation exposed implementation state and weak modal escape

- Before evidence: every action in `interaction-chain`, plus settings at 01:02.233 in
  `first-five-minutes`.
- Observation: a one-pixel exact phase-progress line read as debug instrumentation beside already
  expressive body animation. Pointer users had no visible close control for several modal surfaces,
  and keyboard Tab navigation was swallowed by the persistent compose field.
- Change: remove the phase line, keep phase readability in the body and behavior summary, turn the
  settings slot into an explicit `x` close control while any temporary surface is open, and allow
  Tab, arrows, Enter, Space, and Escape to navigate from Compose. Native input is now recorded in
  privacy-safe feel traces.
- Acceptance: after footage has no phase meter, every temporary mode has a non-overlapping pointer
  close target, and native keyboard/pointer replay can enter and leave a menu without Escape.

### Caption and mouth timing could disagree with audible speech

- Before evidence: dialogue beats in `first-five-minutes` and `interaction-chain`.
- Observation: the speaking body loop began as soon as caption text arrived, before asynchronous
  TTS playback. A fixed eight-second caption expiry could stop a longer spoken line.
- Change: a visible but not-yet-audible caption uses the neutral mouth. Mouth timing begins with the
  speech player, and caption expiry extends through active speech plus a short release.
- Acceptance: after state/audio traces show mouth activity only while speech is active and no
  active line is cut off by the old fixed expiry.

## After verification

- `first-five-minutes` kept the same 301 one-second samples. Settled hover rose from 3 samples to
  120, coarse occupied regions rose from 18 to 24, and the overview shows complete coherent swim
  silhouettes. The creature alternates independent travel, habitat rests, and toy interest rather
  than returning to one destination.
- `quiet-observation` kept the same 181 samples. Settled hover rose from 2 to 72 and occupied
  regions doubled from 7 to 14. Inspection of all eight overview sheets found varied height,
  facing, destination, and body state without decorative clutter or frantic oscillation.
- `relationship-over-time` now completes at 01:43.400, after 46:43 of simulated time. Final values
  are bounded at bond `0.189`, trust `0.214`, respect `0.1`, and resentment `0`; the history has
  seven memories after two feeds, two plays, three comforts, and four talks. The old build stopped
  invalid at 00:48.133.
- The foreground relationship replay retained four speech WAVs. Its 511 speaking frames match 511
  visual speaking frames, with zero non-speaking mouth frames. Ambience reached the authored speech
  duck of approximately `0.447` and recovered across 2,480 traced ducked frames.
- The difficult-conditions replay labels model fallback at 00:06, acoustic uncertainty at 00:13,
  recognition failure at 00:18, and microphone failure at 00:37. The deferred status at 00:27 is
  cleared when dialogue submits at 00:36 instead of surviving the reply.
- The interaction replay emits distinct single semantic cues for food drop/eat, toy impact,
  affection, curiosity, and annoyance. Its ambience stays ducked across 1,504 frames rather than
  only the enqueue frame.
- A real macOS native replay recorded pointer motion and button down/up while opening and closing
  Settings through `compose/close`. A separate native replay recorded three Tab presses and Enter,
  moving focus from `compose/input` to `action/comfort`. Typed input was recorded only as five
  privacy-safe `character` events.
- The first four after experiences ran from the exact copied game binary with SHA-256
  `665f122b71e08b9f31f98c627fa830814677caed83b9192f77f51cab89f836ea`. The long relationship
  experience was repeated from that same binary in a foreground terminal after a background Metal
  drawable stall. That host failure is preserved separately and is not counted as game evidence.
- After integration fixes from the full gate, `interaction-chain` was recorded again from binary
  `81a01835b0c5f9aef0f8f878fffdb7952c89ef442e2d73806728a5a6d49107aa`. The final trace ends with
  no stale status, has 94 speaking frames and zero non-speaking mouth frames, reaches the `0.447`
  speech duck, and retains the expected affection, annoyance, curiosity, food, toy, swim, and
  ambient cues.
- The final `cargo xtask dev --fake-ai` host run completed all six UI-polish captures. Settings,
  bindings, save/data, and reset confirmation remained crisp and unobscured, with the persistent
  `x` close control visible in every inspected temporary surface.

## Surfaced, not applied

- The permanent deck is visually compact and attractive at integer scale, but its icon-only first
  reading remains taste-sensitive. Hover labels and the native path make it usable; whether it needs
  a one-time onboarding hint should be calibrated with a genuinely new player rather than inferred
  from semantic replay.
- Primitive local-model lines such as “I'm. rude giant.” can read as either charming phenotype or
  broken grammar. The pass preserves them when the backend is healthy and only labels actual
  technical fallback. Voice character needs human taste calibration.
- Physical controller feel, real microphone latency, and Windows/Linux window behavior remain
  outside this macOS semantic replay. The harness records those native paths when the devices and
  hosts are available, but does not turn their absence into a passing claim.
- This agent runtime cannot ingest audio content directly. It reviewed cue identity, overlap,
  duration, ducking, waveform, and synchronization from the trace and reference mix, but did not
  claim a timbre or voice-performance judgment. The retained mix is ready for an audio-capable
  agent or short human calibration listen.

## Dismissed observations

- The speech panel itself did not need a new layout. Existing opposite-side placement consistently
  kept the speaking body readable in the reviewed footage.
- Ambient caustics, bubbles, particles, half-pixel buoyancy, and alpha-shaped hover regions remained
  coherent and restrained. The quiet-life problem was authoritative destination rhythm, not a need
  for more decorative motion.
