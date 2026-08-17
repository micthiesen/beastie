# Fourth feel review: interaction continuity

Date: 2026-08-17

This pass reviewed the whole lived game loop after the relationship-causality rework. It used the
normal debug game at 640x360 and 60 fps. No release or final package was built.

The following fresh bundles completed validation and are accepted evidence:

- `target/feel/fourth-pass-first-five-minutes/first-five-minutes`, 5:00.383;
- `target/feel/fourth-pass-quiet-observation/quiet-observation`, 3:00;
- `target/feel/fourth-pass-interaction-chain/interaction-chain`, 1:00;
- `target/feel/fourth-pass-relationship-over-time/relationship-over-time`, 1:43.400;
- `target/feel/fourth-pass-relationship-over-time-no-ai/relationship-over-time-no-ai`, 0:53;
- `target/feel/fourth-pass-trusted-berry/trusted-berry`, 0:16;
- `target/feel/fourth-pass-mushroom-grudge/mushroom-grudge`, 0:16.

Each accepted bundle contains continuous video, dense and overview filmstrips, synchronized
input/event/state/audio traces, a reference mix, waveform, hashes, and review scaffolding. Three
independent visual, behavior, and audio reviews formed observations from the footage before using
the traces to explain them.

The findings converge on one problem: a visible beat can lose continuity with the creature or
action that owns it. The simulation, presentation, dialogue, and art are individually expressive,
but a few handoff seams let one action acquire another action's payoff, let speech outlive its
visible turn, or let an emotional reaction replace the creature itself.

## Main findings

### Positive reactions change Mop's apparent species

- Sessions: `quiet-observation`, `interaction-chain`, `relationship-over-time`, and
  `relationship-over-time-no-ai`.
- Evidence: `interaction-chain` `00:20.600` to `00:21.100`,
  `filmstrips/03-play-start-001.png`; `quiet-observation` `00:09.033` to `00:10.033`,
  `00:41.050` to `00:42.050`, `01:21.066` to `01:22.066`, `01:49.083` to `01:50.083`, and
  `02:34.116` to `02:35.116`; `relationship-over-time` `00:14.033` to `00:16.066` and
  `00:48.133` to `00:50.150`; `relationship-over-time-no-ai` delight at `00:00.016` to
  `00:01.016` and `00:08.016` to `00:09.016`, comfort at `00:12.033` to `00:14.033` and
  `00:22.033` to `00:24.033`, and overlapping delight/comfort at `00:32.083` to `00:34.100`,
  including `filmstrips/00-nonverbal-start-001.png` and
  `01-early-return-without-voice-001.png`.
- Observation: delight, affection, and comfort replace the gold fish with a brown, round,
  curled-tail creature whose silhouette reads as an otter or monkey. The replacement lasts long
  enough to read clearly, then snaps back to the fish.
- Intended reading: this particular fish is delighted, affectionate, or comforted.
- Actual reading: the protagonist changes species at the attachment payoff.
- Cause: delight and affection both map to `creature-v1/reaction/affection-south`, while comfort
  maps to `creature-v1/reaction/comfort-south`. Both authored clips were promoted despite
  substantial identity drift. Their provenance claims exact identity, but the current asset gate
  checks format, palette, alpha, and dimensions rather than character continuity.
- Severity: **high**. It breaks the particular-creature illusion in the moments meant to build it.
- Acceptance: every transition frame preserves the canonical fish's palette, fins, tail, body
  mass, scale, and visual centroid while delight, affection, and comfort remain distinct and
  readable. The same result holds in the AI-off relationship replay.

### Rejected toy play turns into authoritative delight on arrival

- Session: `interaction-chain`.
- Evidence: `00:30.433` emits `toy_rejected(bell)` and `take_toy_away(bell)` while the intention is
  `refuse_and_stare`. At `00:31.233`, with that intention still active, the trace emits
  `toy_played(bell)`, clears the destination, plays `object/toy-impact`, and changes the visual cue
  from suspicion to delight through `00:32.233`. See `filmstrips/05-disliked-toy-001.png` and
  `filmstrips/06-repeat-disliked-toy-001.png`.
- Observation: Mop refuses the bell, swims to it while staring, then appears to enjoy it less than
  a second later. The repeat rejection therefore creates its own contradictory positive outcome.
- Intended reading: Mop knows this toy, dislikes it, and sustains the refusal long enough to make
  that preference legible.
- Actual reading: the refusal is canonically reversed by merely reaching the rejected toy.
- Cause: `play_with_toy` assigns every offered toy as the generic aquarium destination before it
  knows the outcome. `record_genuine_arrival` treats every toy destination as an idle play visit,
  emitting `ToyPlayed` and recording favorite/routine evidence without knowing why the creature
  traveled there.
- Severity: **high**. This is not presentation ambiguity. The false arrival becomes an
  authoritative `ToyPlayed`, updates arrival/favorite/routine evidence, and manufactures impact
  audio and delight while refusal is still active. Accepted direct play currently records its
  relationship and `PlayedWith` mutations eagerly, before contact; moving those mutations to the
  truthful payoff is a deliberate redesign rather than an observed rejection-side mutation.
- Acceptance: a rejected toy cannot emit play, impact, delight, positive history, favorite-place
  evidence, routine evidence, or relationship reward. Repeating the rejected offer may renew the
  refusal, but it cannot turn proximity into acceptance.

### Superseded dialogue speaks after its visible turn is gone

- Session: `relationship-over-time`.
- Evidence: a deliberate interruption burst applies play at `01:08.200`, comfort at `01:08.216`,
  talk at `01:08.233`, and play again at `01:08.250`. The final play clears the pending caption and
  owns the body. Nevertheless `speech-002.wav` begins at `01:08.333` and runs for 2.592 seconds.
  From `01:08.333` through about `01:11.150`, state has `speaking = true` and
  `speech_present = false` while the creature is playing.
- Observation: a disembodied voice speaks over the newer play response with no matching caption,
  mouth, or visible conversational owner.
- Intended reading: the last direct action cleanly supersedes the short-lived talk, or an
  intentionally concurrent reply remains visibly owned for its full audible duration.
- Actual reading: TTS survives after the response it belongs to has been dismissed.
- Cause: direct commands call `clear_speech`, which clears view and active playback but does not
  invalidate a queued TTS request. `poll_tts` discards the completion request ID and plays any
  returned WAV. Ordinary dialogue also lacks the pending-context invalidation used by relationship
  dialogue.
- Severity: **high**. Speech without its creature-facing presentation reads as a software race.
- Acceptance: superseding a turn invalidates its pending dialogue, caption, TTS request, mouth
  animation, and transcript presentation as one operation. No canceled WAV starts, and no frame
  reports audible speech without a valid current speech owner.

### Hearing and answering repeat the same chirp into TTS

- Session: `interaction-chain`.
- Evidence: speech perception at `00:21.633` plays `creature/curious` for 0.580 seconds. Accepted
  talk at `00:22.366` plays the same cue again 153 ms after the first ends. TTS starts at
  `00:22.433`, only 67 ms later, and overlaps the second chirp for roughly half a second.
- Observation: hearing, understanding, and answering sound like a duplicated or stuttering cue
  followed immediately by synthetic voice.
- Intended reading: Mop notices speech, then begins a distinct response.
- Actual reading: the same acknowledgement fires twice and crowds the response onset.
- Cause: both `SpeechPerceived` and `TalkAccepted` map to `creature/curious`; the event projector
  has no turn-level rule that separates perceptual receipt from response voice.
- Severity: **medium**. The input is legible, but the beat is busier and more mechanical than it
  needs to be.
- Acceptance: spoken input retains one immediate perceptual receipt and does not replay an
  identical full-level creature cue into TTS. Typed input keeps prompt visual acknowledgement.

## What already feels strong

- Base swim, turn, hover, notice, food rejection, and toy-refusal art keep one coherent fish
  silhouette across the complete five-minute and three-minute recordings.
- Trusted berry begins with exact-food warmth at `00:00.000`, consumes at `00:10.016`, and completes
  at `00:11.016`. Mushroom grudge begins with exact-food resentment at `00:00.000`, rejects at
  `00:09.016`, and completes at `00:10.016`. Their semantic subjects, outcomes, visual recipes,
  and owned audio remain exact and distinct.
- Player-return expression matures from notice to anticipation across days, stays bound to the
  player, and remains mechanically complete without AI or speech.
- Direct food acknowledgement, physical approach, consumption, first toy refusal, ordinary play,
  comfort, ambience spacing, ducking, fallback status, and speech-card placement remain readable.
- The compact deck, settings layout, speech panel, aquarium composition, and ambient particle work
  do not justify another global UI or scene rewrite from this evidence.

## Dismissed observations and calibration gaps

- The quiet run repeats five preferred-sock bouts in three minutes. An independent review found
  that rhythm predictable, but the second feel pass already reviewed and deliberately accepted the
  same-seed cave, plant, and toy pattern after fixing false arrivals. There is no new regression
  evidence, so this pass does not reopen it.
- Icon-only first-use discoverability still requires a genuinely new player. Semantic replay can
  show layout and state changes, but cannot prove what an unbriefed person notices.
- The reference mixes prove cue identity, overlap, timing, and ducking. They do not prove host
  speaker timbre or output-device quality.

## Evidence gaps

Three fresh `bad-conditions` launches, including the exact copied-binary fallback, stalled before
frame zero. `familiar-cave` and `familiar-plant` later did the same. Each invalid MP4 contains only
its 262-byte header, all JSONL traces are empty, and no manifest exists. These directories were
excluded from feel judgment. Other experiences launched successfully before and after the stalls,
so the failure is not evidence that the game loop itself froze.

The runner correctly fails closed, but it waits the entire authored duration plus 60 seconds before
detecting a zero-frame startup. The next implementation pass must add a short first-frame watchdog,
retain failed-attempt diagnostics, and retry only that startup class in fresh processes. A valid
`bad-conditions` capture remains required before the pass can claim broad degraded-path acceptance.
Prior valid familiar-place evidence remains authoritative; this round adds no contrary product
finding for those motifs.

## Recommended next pass

Implement [interaction-continuity-rework.md](interaction-continuity-rework.md) as one cohesive
pass. Give travel an explicit reason, make toy acceptance resolve at real contact, bind dialogue,
caption, TTS, and mouth animation to one cancelable turn, replace the two identity-breaking
reaction clips, and make zero-frame capture failure fast and diagnosable.

The acceptance loop uses the normal debug build. It replays the exact rejected-bell and rapid
talk/interruption sequences, the quiet and relationship sessions that exercise positive reactions,
the trusted/grudge breadth fixtures, and a newly valid degraded-conditions run. Release or final
builds are not part of this loop.

## Resolution

The cohesive implementation pass closed all four findings and the capture-infrastructure gap.
Final same-seed debug evidence is retained under:

- `target/feel/interaction-continuity-final-interaction-v2/interaction-chain`;
- `target/feel/interaction-continuity-final-dialogue-races-v4/dialogue-races`;
- `target/feel/interaction-continuity-final-relationship-v2/relationship-over-time`;
- `target/feel/interaction-continuity-final-breadth-v2`;
- `target/feel/interaction-continuity-final-quiet-v2/quiet-observation`;
- `target/feel/interaction-continuity-final-bad-v2/bad-conditions`;
- `target/feel/interaction-continuity-final-no-ai-v2/relationship-over-time-no-ai`;
- `target/feel/reaction-identity-contact-sheet.png`.

Rejected bell offers now receive exact nonzero interaction IDs and remain refusal-only through the
old false-arrival window. The final trace contains no bell `ToyPlayed`, impact, delight, positive
visit, or relationship payoff. Accepted player play defers social history and relationship reward
until exact physical contact; autonomous play uses the same owned contact lifecycle without
crediting the player.

Dialogue, caption, TTS, mouth, status, and speech playback now share a composite turn owner. In the
rapid relationship interruption at `00:60.233` to `00:60.250`, the independently numbered TTS request is
enqueued and canceled in the same frame and never starts. Final interaction and relationship
traces contain zero subtitle-on frames with speech but no caption, and zero speaking frames without
a caption owner. Spoken input retains the perceptual curious cue at `00:21.633`; `TalkAccepted` no
longer repeats it into TTS. The separate delayed fixture cancels before reply acceptance, and the
extended interaction chain stops active owned playback at `01:01.333`. A subtitles-off turn remains
audible and internally owned without manufacturing a hidden caption.

Delight, affection, and comfort now use three distinct clips curated from approved gold-fish
runtime frames. The exact 2x contact sheet and final native filmstrips preserve body mass, fins,
tail, palette, and anchor through every transition. The AI-off identity proof remains valid in
`target/feel/interaction-continuity-final-no-ai-v2/relationship-over-time-no-ai`.

The runner now uses a flushed first-frame heartbeat, concurrent early-exit classification, bounded
zero-frame retry, retained per-attempt diagnostics, and recursive process-tree cleanup. A valid
degraded-condition bundle exists at `target/feel/interaction-continuity-final-bad-v2/bad-conditions`;
the final dialogue-race run followed a fully diagnosed three-attempt invalid root, bad conditions
and AI-off recovered on attempt 2, and the focused familiar-ball run recovered on attempt 3. Failed attempts
remain clearly invalid and only the successful attempt is promoted and hashed.

`cargo xtask verify` passes. Three independent final review lenses found no remaining material
interaction-continuity regression. The previously accepted quiet rhythm, direct food response,
relationship causality, layout, ambience, and graceful AI-off behavior remain closed strengths.
