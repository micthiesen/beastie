# Spoken-input foundation

This is the technical seam beneath the implemented product direction in
[v2-plan.md](v2-plan.md). The recognizer, activation, and player-facing states now built on this
seam are documented in [stt-runtime.md](stt-runtime.md).

## Lifecycle

Spoken input crosses the session boundary as four versioned commands:

```text
speech_started
speech_candidate { text, confidence }
speech_ended
speech_failed { failure }
```

`speech_started` reaches the deterministic simulation before any words do. The creature emits a
typed `SpeechPerceived` result (`ignored`, `glanced`, or `attended`) from its current activity,
energy, curiosity, personality, bond, and resentment. Hearing may redirect gaze, but it does not
cancel an action, replace an intention, or move the creature. Glancing or attending projects to the
authored notice reaction and curious sound; ignoring adds no fake acknowledgement.

Recognition confidence is an integer from 0 through 1000. The usable threshold is 650. Candidate
text is transient session state. A usable final candidate enters the exact same
`apply_talk` path as typed text. An uncertain candidate, missing candidate, or technical failure
does not create dialogue, language exposure, memory, or belief. Technical failures remain typed and
separate from `TalkIgnored`, which represents creature behavior.

The lifecycle is order checked. A candidate, end, or failure before start is rejected without
mutation, as is a second start while listening. Saves never contain a raw candidate or an in-flight
recording. Loading or resuming begins idle.

## Worker boundary

Dialogue and TTS now share `beastie-game`'s `JsonlWorkerSession`. It owns contained child-process
lifetime, bounded one-line replies, cancellation polling, unsolicited-output detection, and clean
tree termination. Dialogue and TTS retain their own protocol validation, correlation, fallback,
and recovery rules.

The implemented STT path keeps microphone capture and recognition details outside `beastie-core`,
translates backend confidence into `AcousticConfidence`, and emits only the typed lifecycle above.
Audio bytes and raw partial transcripts never enter saves or authoritative memory. The game owns
capture and resampling; `beastie-stt` validates a private content-addressed WAV and runs Parakeet
inside its persistent contained process. The Moonshine fallback supervises a native sidecar.

## Deterministic evidence

`fixtures/scenarios/spoken-input-foundation.jsonl` covers:

- word-independent attention at speech start;
- a usable candidate entering normal dialogue;
- an uncertain candidate producing no dialogue;
- recognizer unavailability remaining infrastructure failure.

The fixture runs without a microphone, model, audio device, display, or network:

```bash
cargo xtask play --fake-ai \
  --scenario fixtures/scenarios/spoken-input-foundation.jsonl
```

`cargo xtask verify` replays the same fixture and checks its lifecycle counts and single dialogue
request. Core, protocol, and session tests additionally cover deterministic attention, occupied
behavior, text equivalence, malformed ordering, save/replay, and the absence of raw speech text in
durable state.

## Deliberately deferred

- broader speaker and room-condition quality evaluation beyond the deterministic corpus;
- open-microphone, wake-word, or partial-result activation beyond bounded push-to-talk;
- learned pronunciation and per-player vocabulary adaptation;
- prosody, laughter, and other nonverbal acoustic cues;
- deeper habituation, salience, and interruption beyond current attend, glance/defer, ignore, and
  visible refusal.

Future additions must extend this boundary rather than bypassing it.
