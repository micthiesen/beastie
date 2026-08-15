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

### Private life stays local

Dialogue, microphone input, memories, creature state, and generated private artifacts do not leave
the player's device for inference. There is no silent cloud fallback. Downloading an explicitly
chosen model or voice asset is compatible with this rule as long as gameplay data is not uploaded.

Runtime play requires no account, cloud endpoint, subscription, API token, or required GPU.

### Failure stays technical

A corrupt model, missing asset, unavailable voice, or crashed worker degrades gracefully. Technical
failure must not be fictionalized as creature sickness, stupidity, punishment, or personality.
Infrastructure can affect presentation quality, never canonical creature wellbeing.

### Presentation reveals state without becoming state

The creature's condition should be communicated primarily through behavior, expression, sound, and
context rather than exact bars and meters. Presentation projects authoritative meaning; it does not
invent it.

Generated dialogue may contain impossible boasts, dreams, jokes, or mistaken recollections. Text
alone cannot create items, currency, memories, relationships, or stat changes.

### The game remains focused

One highly expressive creature is more valuable than many shallow systems. New features should
strengthen creature life, care, curiosity, development, communication, play, or the sense of shared
history. Productivity features and generic assistant behavior are not default goals.

Conversation stays scarce enough to feel like one faculty of the animal rather than the entire
game. The creature should surprise the player without becoming an endless chat interface.

## Independent design axes

These concerns should remain separate even when one implementation happens to combine them:

| Axis | Owns | Must not own |
|---|---|---|
| **Creature type** | Mechanical priors, needs, affordances, temperament ranges, physical tendencies | Model vendor, prompt implementation, arbitrary scripts |
| **Expression profile** | Vocabulary, grammar, mannerisms, verbosity, emotional expression | Needs, inventory, facts, success probabilities |
| **Cognition backend** | Local inference, capabilities, resource requirements | Canonical identity or state |
| **Memory** | Events, facts, provenance, derived recollections | Raw chat history as truth |
| **Learning** | Familiarity, labels, associations, habits, evidence | Model fine-tuning as the only representation of growth |
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

The durable architecture is not a particular language, renderer, model, or speech engine. It is:

```text
particular simulated creature
        +
canonical history and learning
        +
bounded local cognition
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
