# Second feel review

Date: 2026-08-16

This pass reviews the game after the first baseline cleanup in
[feel-review-baseline.md](feel-review-baseline.md). Same-seed before and after evidence is retained
under `target/feel/second-pass-before-*` and `target/feel/second-pass-after-*`. These ignored bundles
are local review evidence; the durable findings and decisions live here.

## Applied findings

### Captions ended before the voice and mouth

- Before evidence: every reviewed experience containing dialogue. The first-use line lost its
  caption for 40 speaking frames. The interaction line lost it for about 0.9 seconds, both fallback
  lines for 0.55 to 0.9 seconds, and all four relationship lines for 0.67 to 1.77 seconds.
- Observation: the view expired speech against coarse simulation time before the game shell could
  extend it from the active audio player. The body continued talking after its words vanished.
- Change: active speech now retains its caption through playback and a bounded 500 ms release.
  Clearing speech also restores compose focus when a transient reaction control owned focus.
- Acceptance: each same-seed after trace must contain zero frames where speech is active without a
  caption and zero frames where an absent reaction surface retains reaction focus.

### Autonomous play had no arrival or play beat

- Before evidence: `quiet-observation`, especially the five sock trips around 00:01 to 00:13,
  00:30 to 00:50, 01:10 to 01:27, 01:40 to 01:54, and 02:25 to 02:40.
- Observation: `Intention::Play` occupied 4,682 of 10,814 frames, but every frame had no action and
  no arrival event. The creature swam to the sock, paused, and reversed, so independent play read as
  a repeated commute.
- Change: a genuine autonomous toy arrival emits the same authoritative `ToyPlayed` outcome as
  accepted player play. Presentation supplies the authored delight body, toy impact, and audio duck
  without inventing simulation facts.
- Acceptance: every completed toy trip in the after run has a synchronized event, visible cue, and
  one semantic `object/toy-impact` sound.

### Autonomous routing and labels exposed the scheduler

- Before evidence: the later first-use run narrowed into a cave/plant shuttle, including seven
  consecutive alternating bouts. The repeat guard always selected the first non-matching member of
  a fixed fallback list. Quiet footage also labeled approach and play as `hovering`.
- Change: the repeat guard rotates through the available alternatives from persisted visit history
  and identity, without consuming another random draw or biasing the first fixed fallback.
  The summary now distinguishes `swimming over`, `swimming to a toy`, and `playing`.
- Acceptance: same-seed replay remains exact, avoids immediate destination repetition without a
  fixed cave/plant fallback, and labels visible travel and play honestly.

### Edge destinations weakened the creature silhouette

- Before evidence: cave and toy endpoint hovers placed the body against or partly beyond the water
  boundary for roughly 55 seconds of `quiet-observation`.
- Change: ordinary body placement uses the authored side-body opaque envelope when clamping its
  presentation origin. Semantic world positions and simulation movement remain unchanged.
- Acceptance: the full authored opaque envelope remains within the water at both extreme world
  positions, and the same-seed overview keeps the face and body readable at cave and toy visits.

## After verification

- `first-five-minutes` completed 18,023 frames. Captionless speaking fell from 40 frames to zero;
  invisible reaction focus fell from 1,384 frames to zero.
- `interaction-chain` and `bad-conditions` both contain zero captionless speaking frames and zero
  invisible reaction-focus frames. Technical fallback, deferred attention, refusal, and recovery
  retain their distinct status and audio behavior.
- `quiet-observation` completed 10,814 frames. Five completed toy arrivals emit five `toy_played`
  events and five `object/toy-impact` cues with continuous authored ducking. Its overview changes
  the summary from `swimming to a toy` to `playing` at arrival and keeps the body readable at both
  endpoints.
- `relationship-over-time` completed 6,205 frames and preserved the before run's final bond
  `0.189`, trust `0.214`, seven memories, and four active days. Captionless speaking fell from 305
  frames to zero; invisible reaction focus fell from 5,124 frames to zero.
- The first relationship after attempt acquired no Metal drawable and wrote zero state frames. It
  was rejected as invalid host evidence and preserved separately with a `-zero-frame` suffix. The
  immediate fresh run completed normally.
- The full gate then exposed and drove two integration corrections: settled autonomous play remains
  interruptible by the player, and accelerated time retains at most one obsolete toy-arrival beat
  per toy. Both focused regressions and the complete gate pass. Two attempts to recapture the final
  quiet binary acquired no Metal drawable and wrote zero frames, so they were rejected rather than
  treated as evidence. The prior valid native quiet run still demonstrates the unchanged play cue,
  labels, and body placement; final routing and player priority are covered by exact replay tests.
- Core, view, and game tests passed with 50, 45, and 85 tests respectively. The normal headless
  workspace gate passed after the review changes. No release package or final bundle was built.

## Surfaced, not applied

### Accumulated relationship history is still hard to feel

The relationship trace develops materially across four active days: memories grow from three to
seven, language advances from words through phrases to settled routine, routines reach eight, and
bond and trust rise. The ordinary shell barely expresses those changes. Arrival filmstrips remain
visually similar, the final inspect marker ends before a result can be read, and the day-three and
day-four talks repeat the exact line `yes. old thing remains. remains.`

The next authored-feel pass should make selected history visible through bounded familiar routines,
remembered-object responses, or compact relationship callbacks. Dialogue context should suppress
accidental consecutive duplicates. Acceptance should require distinct same-seed arrival beats and
progressive callbacks without exposing relationship meters or turning the creature into a status
dashboard.

### Remaining judgment limits

- The reference mix proves cue identity, overlap, duration, ducking, and synchronization, but this
  runtime did not judge voice timbre or host-speaker quality.
- Real microphone latency, physical controller feel, and Windows window/input/audio behavior remain
  native-host checks. Windows validation is deliberately parked until that host is available and a
  portability checkpoint is useful.

## Clean areas

- Direct food, comfort, toy, interruption, and refusal chains remain legible and semantically
  distinct. Repeated disliked-toy input is visibly rejected rather than lost.
- Technical model, recognition, and microphone failures remain bounded, clearly technical, and
  recover to text play without freezing the creature.
- The complete eight-frame swim remains coherent. Ambient caustics, bubbles, particles, semantic
  audio mapping, duck envelopes, modal cleanup, and the absence of the old phase meter remain clean.
- The opposite-side speech panel continues to protect the speaking body; no layout rewrite was
  justified by this evidence.
