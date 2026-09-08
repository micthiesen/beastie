# Feel review loop

## Purpose

This is Beastie's repeatable playtest loop for judging craft, clarity, responsiveness, creature
presence, and emotional readability. It treats feel as reviewable evidence rather than a collection
of unlocated opinions.

The loop complements correctness tests. A green gate can prove that an input produces the intended
state transition, but it cannot prove that the player noticed the response, understood its cause,
or enjoyed its timing. Feel review asks whether the same transition reads as the behavior of a
particular living creature inside a carefully made game.

[Game-design philosophy](game-design-philosophy.md) remains the product authority. In particular,
polish must strengthen embodied creature life rather than turn Beastie into a more efficient
command interface or chatbot.

## Start here

Run the baseline suite:

```bash
cargo xtask feel --suite baseline
```

Use `--output target/feel/<name>` when a stable before/after name matters. The output directory must
be new or empty. Review the generated reports and evidence under `target/feel/<run>/`. Create findings only when they
can cite a timestamp or a session-wide pattern. After a change, replay the same suite with the same
seed and compare the synchronized before and after evidence.

Focused suites use the same command with `first-five-minutes`, `quiet-observation`,
`interaction-chain`, `bad-conditions`, or `relationship-over-time` in place of `baseline`.
`relationship-breadth` provides the fixture-backed trusted-food, food-grudge, and familiar-place
coverage. `interaction-chain` also includes a bounded `dialogue-races` experience that delays the
fixture reply without blocking frames, proves pre-reply supersession, and exercises subtitles-off
ownership. The runner gives frame zero a separate bounded deadline, retains diagnostics for a live
zero-frame process, recursively terminates its complete worker tree, and retries that class in a
fresh process up to two times by default. `--startup-timeout-ms` and `--startup-retries` are
diagnostic overrides. Early exit, scenario errors, recorder errors, and validation errors are not
retried. Only the promoted successful attempt is validated and hashed.
The command builds the game and fake workers, verifies FFmpeg and FFprobe, launches each visible
experience, validates its video, generates filmstrips, and hashes the evidence.
Pass `--game <executable>` to record an exact copied, packaged, or release executable while still
using the normal evidence pipeline. This is also the reliable macOS fallback when Metal renders a
copied executable but stalls for the identical build under `target/debug`.

## Evidence bundle

Every run produces one self-contained directory:

```text
target/feel/<run>/
  README.md
  <experience>/
    attempts.json
    attempts/
    manifest.json
    session.mp4
    session-audio-reference.mp4
    reference-mix.wav
    audio-overview.png
    speech-*.wav
    inputs.jsonl
    events.jsonl
    state.jsonl
    audio.jsonl
    markers.jsonl
    captures/
    filmstrips/
    review.md
```

The evidence has five synchronized parts:

1. `session.mp4` records the complete native Bevy framebuffer at 60 frames per second with real frame
   timing. It preserves every transition rather than only selected checkpoints.
2. `inputs.jsonl` records timestamped semantic and native inputs, including pointer motion, clicks,
   keys, controller actions, and speech lifecycle events.
3. `events.jsonl` and `state.jsonl` align authoritative simulation events and selected presentation
   state with the recording. They explain causality without treating presentation as truth.
4. `audio.jsonl` records cue, speech, active-player, and ambience-duck timing. Speech WAVs are
   retained inside the evidence directory. `session-audio-reference.mp4` mixes those speech files
   and authored cues at their recorded times so an agent can review synchronized sound without a
   system-audio device. It is explicitly a reference mix, not a claim that host output was captured;
   `manifest.json` records that distinction.
5. `filmstrips/` contains uniformly sampled session overviews plus dense, event-aligned frame
   sequences around every interaction. `review.md` links findings to these artifacts.

The manifest records the commit and dirty state, exact game-binary and scenario hashes, platform,
build profile, seed, suite, viewport, presentation scale, enabled fake backend, audio-capture mode,
and artifact hashes. A comparison is valid only when intentional differences are explicit.

On macOS, a game created while yabai is managing the window can stall while Metal waits for its
first drawable. If sampling shows `get_current_texture` or `acquire_texture` dominating, run
`yabai --stop-service` once, restart that experience, and leave yabai stopped for the rest of the
session. A stalled zero-frame run is invalid evidence.

## Baseline suite

The baseline suite exercises seven lived experiences across six kinds of play:

1. **First five minutes:** Start with no privileged knowledge. Observe initial attention, apparent
   affordances, first interaction, text entry, settings discovery, and recovery from mistakes.
2. **Quiet observation:** Leave the creature alone for several real-time minutes. Look for a life
   with intention and variation rather than an idle loop waiting for commands.
3. **Interaction chain:** Feed, comfort, play, speak, interrupt, repeat, and offer something disliked.
   Judge acknowledgement, anticipation, payoff, and return to ongoing life.
4. **Difficult conditions:** Exercise refusal, misunderstanding, deferred attention, unavailable AI,
   unavailable speech, and other graceful degradation. Technical failure must remain distinguishable
   from creature behavior.
5. **Relationship over time:** Accelerate between important states, then record normal-speed windows
   before and after meaningful changes. Judge whether history and development become visible without
   distorting the pacing of the moments themselves.
6. **Relationship without AI:** Replay the relationship arc with generated language and TTS absent.
   Judge whether the creature remains mechanically complete, emotionally readable, and truthful
   without its optional mouth.
7. **Dialogue races:** Reproduce slow pre-reply cancellation, subtitles-off speech ownership, and
   active-playback interruption with deterministic fixture timing that leaves frame progress live.

Use semantic replay for repeatability and native pointer, keyboard, microphone, and controller
input for final host validation. Semantic replay cannot prove discoverability, hit-target comfort,
physical input feel, window behavior, or hardware-specific latency.

## How to review a run

Review at three temporal resolutions:

1. **Session rhythm:** Inspect the complete timeline for dead time, crowding, interruption rate,
   repetitive behavior, and whether the creature appears to exist between player actions.
2. **Interaction beats:** Inspect dense filmstrips and traces from shortly before input through
   acknowledgement, anticipation, outcome, and recovery. Measure where the intended reading becomes
   visible.
3. **Individual frames:** Inspect composition, hierarchy, voxel silhouette coherence, spacing, text, focus,
   hit targets, contrast, and overlap at native window scale.

Do not infer feel from a single attractive checkpoint. Do not infer broken mechanics from unclear
presentation until the authoritative trace is checked. Do not use accelerated playback to judge
animation timing.

## Review rubric

### Interaction and presentation

- **Legibility:** Can the player identify what is interactive and understand what just happened?
- **Responsiveness:** Does every action receive prompt acknowledgement even when its outcome is
  delayed, refused, or delegated to a worker?
- **Physicality:** Do gaze, motion, anticipation, impact, sound, and recovery make actions embodied?
- **Visual hierarchy:** Does the creature remain central while UI appears with only the strength and
  duration it needs?
- **Rhythm and craft:** Do pauses feel intentional, reactions have room to land, transitions remain
  coherent, and repeated actions avoid mechanical sameness?

### Creature and relationship

- **Creature continuity:** Does Beastie appear occupied and self-directed between interactions?
- **Emotional causality:** Can behavior be connected to current state, history, preference,
  temperament, attention, or activity?
- **Legible imperfection:** Does refusal or misunderstanding read as a creature limitation rather
  than ignored input or lost state?
- **Development:** Do longer sessions reveal qualitative change and shared history without relying
  on meters or exposition?
- **Attachment:** Does the session feel like time with this particular creature rather than a series
  of commands handled by an interface?

The rubric guides judgment; it is not a numeric scorecard. Automated measurements can identify
latency, repetition, clipping, discontinuity, or missing feedback, but they cannot decide whether a
moment is lovable, funny, irritating in the intended way, or emotionally true.

## Writing findings

Each finding records observation before solution:

```markdown
### Feed action reads as unregistered

- Session: interaction-chain
- Evidence: 00:18.400 to 00:19.730, filmstrips/feed-03.png
- Observation: The tray closes immediately, the berry appears 420 ms later, and Beastie does not
  orient for another 910 ms. For 1.33 seconds there is no visible causal link between the click and
  the creature.
- Intended reading: Beastie noticed the offered food and is deciding what to do.
- Actual reading: The click may not have registered.
- Likely layer: presentation timing and attention cue
- Severity: high
- Proposed change: acknowledge the drop immediately, then preserve the deliberate approach delay.
- Acceptance: On identical replay, the offer is visibly acknowledged within 150 ms and the approach
  still reads as creature-paced rather than interface-paced.
```

Prefer a small number of high-confidence findings. Separate observations that imply different
causes or require different acceptance evidence. Session-wide findings, such as repetitive idle
behavior, cite representative timestamps and state their observed frequency.

## Iteration loop

1. **Record:** Run the baseline or focused suite against a clean commit and retain the evidence
   bundle as the baseline.
2. **Triage:** Rank findings by how strongly they damage comprehension, creature presence,
   interaction payoff, or attachment. Choose one coherent problem area.
3. **Change:** Make the smallest cohesive pass that could improve the chosen reading. Preserve the
   simulation and presentation boundaries.
4. **Compare:** Replay identical seeds, inputs, timings, viewport, and backends. Generate synchronized
   before and after clips, filmstrips, traces, and timing deltas.
5. **Decide:** Keep the change only when the intended reading is clearer without weakening another
   rubric area. Run `cargo xtask verify` and perform native validation when the change affects the
   visible shell, real input, audio, or host timing.

A successful comparison closes the finding with its after timestamp and evidence path. An
inconclusive comparison remains open. Never claim improvement from changed seeds, different
simulation state, or a hand-selected after frame that omits the surrounding transition.

## Human calibration

Agent review should reduce the player's burden, not ask the player to enumerate every flaw. When
subjective direction remains uncertain, present one short synchronized A/B comparison and ask one
specific question. Record the answer as a reusable project preference only when it expresses a
durable principle rather than a one-off choice.

Human calibration is especially important for emotional authenticity, affection versus annoyance,
voice character, controller feel, and the overall aesthetic. Repeated calibration should make later
agent review more independent and more consistent with the intended taste.

## Completion criteria

A feel pass is complete when:

- the selected sessions have valid synchronized evidence bundles;
- every accepted change has same-seed before and after evidence;
- high-severity regressions found in comparison are resolved or explicitly rejected with rationale;
- the full automated gate passes; and
- affected native inputs, audio, window behavior, and real-time pacing have been exercised on the
  host where practical.

Feel evidence is review material, not a permanent binary fixture by default. Commit durable
findings, decisions, scenario definitions, small representative images, and tooling contracts.
Keep large recordings under `target/` or another ignored evidence location unless a specific clip
is important enough to justify repository storage.
