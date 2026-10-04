# Beastie game-design philosophy

## Authority and purpose

This is Beastie's canonical product philosophy. It answers **what kind of game Beastie is** and
which boundaries future work must preserve. It deliberately avoids current implementation details,
roadmaps, model selections, test commands, and platform plans.

- [STATE.md](STATE.md) records the current work and chosen next step.
- [architecture.md](architecture.md) describes the system that exists now.
- Milestone plans and specifications describe what a particular release is building.

Those documents may change frequently. When a feature or implementation choice conflicts with this
philosophy, the conflict must be resolved explicitly rather than allowing the lower-level document
or code to redefine the game by accident.

## The thesis

> **Beastie is a game about caring for a particular simulated creature whose ability to understand
> and express itself is mediated by a small, local artificial mind. The simulation is reality; AI
> gives that reality interpretation, language, personality, and surprise.**

Beastie should optimize for attachment to a particular creature, not for the objective quality of
conversation. The winning reaction is less often “that was an intelligent answer” than “of course
*my* Beastie would do that.”

The game is not “a chatbot with a pet skin” or “an assistant that lives on the desktop.” It is a
creature game that remains recognizable when conversation is removed. Care, observation, play,
development, and shared history are the center. Language is one way the creature reveals its life.

Spoken interaction should eventually be Beastie's primary freeform social interface. The player
talks aloud to a creature already living in a shared simulated space, not to a service waiting for a
request. Voice makes the creature feel present; it must not make Beastie into a voice assistant.
Text remains a complete, first-class alternative rather than a lesser mode.

The distinctive promise is not merely local inference, memory, voice, or simulation-first design.
It is their combination:

> **The limitations and characteristics of a local cognition model can become part of the
> creature's phenotype, while no model is ever authoritative about the creature's actual state.**

A small model is not necessarily a low-quality mode. It may be an intentionally strange brain. A
different model may change fluency or interpretation without replacing the creature's identity or
rewriting its life.

## What creates attachment

### Dependency and responsibility

The creature has an existence independent of the conversation interface. Its condition changes in
response to the player's care and its own activity. Neglect may produce hunger, boredom, distance,
resentment, or odd behavior, but never death or permanent ruin.

### Contingency

Reactions arise from present state, history, environment, temperament, and learned associations.
The player should be able to develop folk theories about why their particular Beastie behaves the
way it does.

### Legible imperfection

Misunderstanding, awkward language, strange associations, and limited vocabulary can make the
creature feel small and alive. Failure is useful when it reads as a creature limitation. It is
harmful when it reads as lost state, ignored input, hallucinated mechanics, or broken software.

### Development

The player should notice qualitative change over time: concepts become familiar, language shifts,
habits form, memories accumulate, and the relationship becomes specific. “We have a history” is
more important than “this model has a large context window.”

### Surprise within identity

Behavior and expression should be difficult to predict word-for-word while remaining recognizable
as this creature. Surprise should deepen identity rather than dissolve it into randomness.

## Canonical invariants

### The simulation owns reality

Needs, objects, location, relationships, preferences, progression, learned concepts, memories,
beliefs, and mechanically meaningful outcomes belong to explicit game state and rules. Generated
output cannot directly patch that state or establish that an event occurred.

AI may interpret input or propose an expression. Anything with gameplay significance must pass
through a closed, validated game command or event. A fluent claim is not a fact.

```text
simulation state
    != model context
    != generated prose
    != model hidden state
```

### The creature exists without AI

Movement, needs, animation, interaction, and basic reactions continue when cognition or speech is
absent, loading, slow, malformed, or crashed. The game acknowledges interaction immediately through
simulation and presentation rather than freezing while generation catches up.

Before adding an AI call, ask whether the feature has a coherent state transition with AI disabled.
If it does not, game truth is probably in the wrong layer.

### A Beastie is portable across brains

Identity lives in canonical state, traits, memories, learning, relationships, and stable profile
references, not in hidden model state or a provider session. Changing or removing a compatible
cognition backend cannot make a save stop being the same creature.

Model capability affects interpretation and expression. It does not grant higher stats, better
mechanical outcomes, forbidden knowledge, or a more legitimate version of the creature.

### Personality is embodied, not prompted

Personality must be visible in choices, movement, habits, preferences, attention, refusal, and
relationship behavior as well as words. It cannot exist only as a large system prompt.

The player cannot use a personality editor or instant reset to erase an inconvenient creature.
Change happens through lived interaction and is gradual enough to feel earned.

### Interaction is embodied before it is conversational

The interaction hierarchy is embodied world interaction first, voice as the primary freeform
language channel, text as an equivalent semantic path, and explicit conversational menus last.
Feeding, touching, moving objects, playing, observing, arranging the environment, and performing
routines must remain more fundamental than talking.

Conversation stays inside the continuing world. The creature remains located, occupied, and able to
move or act while the player speaks. Language may depend on shared objects and current activity:
“Why are you doing that?” only means something because both participants inhabit the same context.
Speaking must not open a mode that pauses creature life or turns the game into request-and-response
chat.

Primary does not mean mandatory. Anything meaningful that can be communicated through voice should
generally remain expressible through text, with the same interpretation and creature behavior. Core
progression must never require a microphone.

### The creature hears before it understands

Speech is an ongoing perceptual event before it becomes recognized language. The creature may
notice a voice, glance over, become attentive, or deliberately continue what it is doing before any
words are available. Hearing may capture some attention; it never automatically takes control.

Audio detection, noticing, transcription, creature-limited interpretation, willingness to engage,
and response are distinct events. Understanding does not imply cooperation, and acknowledgement
does not imply understanding. Attention and interruptibility arise from activity, needs, mood,
temperament, relationship, salience, and history rather than from an infinite conversational
interrupt channel.

Once transcribed, spoken and typed words enter the same creature-level language and behavior rules.
Input modality cannot make the creature more intelligent, obedient, talkative, or willing to
answer. The creature may respond verbally, act, acknowledge nonverbally, delay, misunderstand,
refuse, or ignore. The game should usually make receipt legible without implying that the creature
owes the player conversational compliance.

### Memory is history, not prose

Canonical memory records structured events and facts with provenance. Generated summaries,
recollections, dreams, stories, and phrasing may be selective, mistaken, or charmingly biased, but
they remain derived views. Regenerating or deleting them cannot alter what happened.

Beliefs may be wrong. Their evidence and evolution still belong to the simulation. The creature may
misunderstand reality without allowing a model hallucination to rewrite reality.

### Learning is explicit

The creature develops concepts, labels, associations, and habits through game rules backed by
evidence. A model may help interpret whether an utterance refers to a known concept; it cannot
declare that it learned something and thereby mutate learning state.

Language development should make the creature's growing understanding visible without pretending
that a foundation model is being trained inside the save.

A transcription may contain fluent language beyond the creature's current concepts. Recognition of
the player's words cannot bypass developmental limits: the creature understands only what its own
knowledge, associations, context, and development permit. Over time it may learn the player's
nicknames, recurring phrases, pronunciations, and associations so that understanding feels specific
to the relationship.

### Perception is not truth

Microphone audio and transcripts are untrusted perceptual evidence, not canonical facts. Acoustic
uncertainty, transcription error, and creature misunderstanding are separate sources of ambiguity
and may produce different behavior. The game must not silently convert uncertain speech into the
most useful command merely for interface efficiency.

Prosody, laughter, hesitation, calling, or other nonverbal audio may become soft perceptual signals.
They must never be treated as certain knowledge of the player's feelings or as authority over world
state.

### Private life stays local

Dialogue, microphone input, memories, creature state, and generated private artifacts do not leave
the player's device for inference. There is no silent cloud fallback. Downloading an explicitly
chosen model or voice asset is compatible with this rule as long as gameplay data is not uploaded.

Runtime play requires no account, cloud endpoint, subscription, API token, or required GPU.

### Failure stays technical

A corrupt model, missing asset, unavailable voice, unavailable speech recognition, microphone
failure, or crashed worker degrades gracefully. Technical failure must not be fictionalized as
creature sickness, stupidity, punishment, or personality. Infrastructure can affect presentation
quality, never canonical creature wellbeing. Creature misunderstanding can be charming; broken
software must remain distinguishable from it when the player needs practical feedback.

### Presentation reveals state without becoming state

The creature's condition should be communicated primarily through behavior, expression, sound, and
context rather than exact bars and meters. Presentation projects authoritative meaning; it does not
invent it.

A record of shared history is not a meter. Showing which words the creature has learned from the
player, or picturing the thing it wants in a thought bubble, reveals state the way the creature
itself would; percentages, need bars and scores do not.

Generated dialogue may contain impossible boasts, dreams, jokes, or mistaken recollections. Text
alone cannot create items, currency, memories, relationships, or stat changes.

### The game remains focused

One highly expressive creature is more valuable than many shallow systems. New features should
strengthen creature life, care, curiosity, development, communication, play, or the sense of shared
history. Productivity features and generic assistant behavior are not default goals.

Conversation stays scarce enough to feel like one faculty of the animal rather than the entire
game. The creature should surprise the player without becoming an endless chat interface.
Scarcity applies to the creature's own speech, never to being heard: every utterance is perceived
at once and visibly received, and the creature then speaks only when it has something of its own to
say, in the words the player has taught it.

## Independent design axes

These concerns should remain separate even when one implementation happens to combine them:

| Axis | Owns | Must not own |
|---|---|---|
| **Creature type** | Mechanical priors, needs, affordances, temperament ranges, physical tendencies | Model vendor, prompt implementation, arbitrary scripts |
| **Expression profile** | Vocabulary, grammar, mannerisms, verbosity, emotional expression | Needs, inventory, facts, success probabilities |
| **Cognition backend** | Local inference, capabilities, resource requirements | Canonical identity or state |
| **Memory** | Events, facts, provenance, derived recollections | Raw chat history as truth |
| **Learning** | Familiarity, labels, associations, habits, evidence | Model fine-tuning as the only representation of growth |
| **Language input** | Uncertain perceived words, timing, and modality-specific evidence | Personality, obedience, intelligence, or separate dialogue policy |
| **Voice** | Abstract vocal identity and presentation | Creature mechanics |
| **Generated micro-content** | Dreams, stories, songs, doodles, rituals, flavor | Implicit world-state changes |

Creature type and expression profile are distinct. The same kind of creature can be terse or
verbose; the same expression profile can survive a compatible backend swap. A model is a mechanism.
A creature has a style and a life.

## Feature placement

When deciding where a feature belongs, use these ownership questions:

| Question | Owner |
|---|---|
| Is the creature hungry, and does it like this food? | Simulation state |
| What happened before, and what does the creature believe about it? | Canonical memory and belief rules |
| Has it learned what a word or object means? | Learning state |
| What words might the player have spoken? | Uncertain local input perception |
| What did the creature understand, and will it engage? | Learning, context, attention, and behavior rules |
| Which relevant facts may cognition see right now? | Bounded deterministic context selection |
| How would it phrase its current feeling? | Expression profile and cognition |
| What emotion, motion, or sound should make that feeling legible? | Authoritative semantic state projected through presentation |
| Can this input cause a game action? | Closed command interpreted and validated by the simulation |
| Did generated content really happen? | No, unless the simulation separately records a permitted event |

Natural-language interpretation may enable an interaction that has no dedicated button. Even then,
the model translates language into a closed game command, and the simulation decides whether that
command is possible.

## Growth without punishment

The creature may become demanding, crude, spiteful, stubborn, or provocative when those behaviors
get results. The player can influence those habits, but there is no morality meter or instant
correction. Rehabilitation is possible and slow.

Consequences should create character and interaction, not coercive maintenance. Returning after an
absence should reveal a living relationship to resume, never a tombstone or irrecoverably ruined
save. Beastie invites attachment without turning attachment into an obligation schedule.

## Future extensions

Future brains, voices, creature types, memories, dreams, lifecycle systems, and mods are welcome
when they preserve the invariants above. Data-defined extensions are preferable to arbitrary code.
Any future executable extension needs explicit capabilities and must not silently gain network,
filesystem, or direct save-mutation authority.

Breeding, if adopted, should inherit creature data and tendencies rather than model weights. Dreams
and generated artifacts begin as flavor. If they ever affect mechanics, the effect must be reduced
to a closed, validated simulation event with recorded provenance.

Future spoken interaction may become low-friction, interruptible, and sensitive to bounded
nonverbal cues, but it must preserve explicit microphone control and strictly local processing. It
must not imply indefinite background capture, omniscient understanding of the player's room, a
universal wake-word command surface, or strict conversational turn-taking.

The durable architecture is not a particular language, renderer, model, or speech engine. It is:

```text
particular simulated creature
        +
canonical history and learning
        +
bounded local cognition
        +
embodied, uncertain perception
        +
replaceable expression and voice
        +
strictly local private data
        +
failure-tolerant presentation
```

## Design review test

A change belongs in Beastie when it strengthens the feeling of caring for a particular creature
without weakening simulation authority, offline play, privacy, portability across brains, or
graceful degradation.

The final question is:

> **Does this make Beastie a more convincing particular little creature, or merely a smarter
> chatbot?**

For spoken interaction, also ask whether the same words would meet essentially the same creature,
whether latency is absorbed by honest embodied behavior, and whether the feature creates presence
rather than command convenience. The detailed V2 direction and review questions live in
[v2-plan.md](v2-plan.md).
