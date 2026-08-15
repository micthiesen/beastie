# Beastie V2 plan

V1 established the expressive aquarium, deterministic creature, persistent interaction shell, and
offline language and speech output. V2 should make talking aloud to the creature the primary
freeform social interaction without turning Beastie into a voice assistant.

This plan defines the desired experience and durable design boundaries for spoken interaction. It
does not commit to a recognition system, activation gesture, technical architecture, or low-level
implementation. [game-design-philosophy.md](game-design-philosophy.md) remains the canonical product
authority.

## Product outcome

V2 succeeds when:

> The player can talk naturally while both player and creature remain engaged with the simulated
> world. The creature may notice speech before it understands the words, then interpret and respond
> according to its own attention, knowledge, mood, personality, relationship, and current activity.

The guiding principle is:

> **Voice makes the creature feel present; it must not make Beastie into a voice assistant.**

Voice should eventually be the interaction mode the game is aesthetically designed around. It is
not mandatory. Text remains an excellent, complete alternative for accessibility, privacy, noisy
environments, speech differences, personal preference, development, and testing.

## Interaction hierarchy

1. **Embodied world interaction:** feeding, touching, playing, moving and introducing objects,
   arranging the environment, observing, and sharing routines.
2. **Voice:** the primary freeform language channel, used while creature life and world interaction
   continue.
3. **Text:** an always-available path carrying equivalent semantic meaning into the same creature
   behavior.
4. **Conversational menus:** a last resort. Social interaction should not require opening a chat
   panel or selecting dialogue commands.

The hierarchy keeps language grounded in creature life. Talking adds to care, play, observation,
and shared context; it does not replace them.

## Product invariants

- The simulation remains authoritative about the creature, world, and consequences.
- Speech recognition produces uncertain perception, never canonical truth.
- Voice and text meet the same creature-level language and behavior rules.
- Recognition cannot bypass developmental language or learned concepts.
- Speaking may attract attention but never automatically takes control of the creature.
- The creature may misunderstand, refuse, delay, answer nonverbally, or ignore the player.
- Conversation remains spatial and embodied while simulation continues.
- Microphone audio, transcripts, interpretation, and creature cognition remain local.
- Text remains a complete fallback, and core progression never requires a microphone.
- Recognition failure never blocks simulation or harms canonical creature state.

## One creature behind voice and text

After transcription, a spoken utterance enters the same semantic interaction path as the identical
typed utterance. Voice does not receive a separate personality, dialogue policy, social model, or
standard of compliance.

If the player says and types “Why did you put that there?”, the creature encounters equivalent
language events under the same context, knowledge, mood, personality, relationship, attention,
willingness, and developmental limits. The input channel must not make it more intelligent,
obedient, conversational, or likely to answer.

Differences are justified only when intrinsic to the medium: uncertain transcription, timing,
speech unfolding over time, acknowledgement during speech, interruption, and any deliberately
supported nonverbal audio.

## Hearing unfolds over time

The creature should not wait for a complete transcript before appearing to notice the player.
Speech is first an ongoing sound in its environment and only later a candidate linguistic event.

The conceptual sequence is:

1. **Audio is detected:** something resembling speech is happening.
2. **The creature may notice:** it reacts perceptually according to attention and context.
3. **A candidate transcription completes:** the system has uncertain words.
4. **The creature attempts interpretation:** its simulated knowledge bounds what those words mean.
5. **The creature decides whether to engage:** understanding never guarantees cooperation.
6. **It responds or acts:** verbally, physically, partially, later, or not at all.

Possible early reactions include glancing toward the player, turning an ear or head, pausing,
looking up from an object, blinking, making a small sound, moving closer, or visibly remaining
focused. These reactions make recognition latency feel like part of sharing space instead of a
loading pause.

The creature must not always stop when speech begins. A curious idle creature may turn immediately;
one eating may glance over and continue; a sleepy creature may barely react; an angry creature may
clearly hear and refuse; one absorbed in a toy may look up and resume; a social creature may abandon
a low-priority activity and approach.

> **Speech can capture some attention without automatically taking control of the creature.**

## Attention and autonomy

Voice must preserve the possibility of a meaningful attention model rather than creating an
infinite conversational interrupt channel. Relevant qualities include current focus,
interruptibility, social attentiveness, salience, habituation, novelty, repeated calling, competing
needs, fatigue, emotional avoidance, temperament, and relationship.

This should produce moments specific to a living creature:

- It flicks an ear while examining a new object, then finally looks over when it hears its nickname.
- It pauses at a question while angry, demonstrating that it heard, then turns away.
- It suddenly attends to a favorite food mentioned inside an otherwise uninteresting sentence.
- It approaches during the player's speech while idle, then responds after understanding arrives.

The creature does not owe the player conversational compliance. It may answer fully or tersely,
delay, acknowledge without words, return to its activity, ignore, react to one salient word, become
curious or annoyed, refuse, or respond after its current activity ends.

Even refusal should usually leave legible evidence that an interaction occurred: a turned posture,
dismissive sound, pointed resumption of an activity, or reaction to one interesting word. This avoids
making autonomy look like dropped input. Long punitive silent lockouts are undesirable; refusal
should create personality and stories rather than disable play.

## Shared space, not conversation mode

There should be no separate voice mode in which the simulation pauses and the game becomes a
dialogue interface. The creature remains physically located and occupied, current objects remain
relevant, either participant may move or act, and physical responses can carry as much meaning as
words.

Shared context should make language more powerful. “Why are you doing that?” refers to the
creature's current behavior. “Do you want this?” refers to an object the player is holding or has
selected. Voice should increase the importance of the world rather than abstracting interaction
away from it.

## Recognition is uncertain perception

A transcript is a perceptual hypothesis about what the player said, never canonical truth. The
design must keep at least two failure sources distinct:

1. The input system transcribed the player incorrectly or incompletely.
2. The creature received the words but misunderstood them through its own limited knowledge.

Either may lead to a puzzled look, a request for repetition, repeating the word it thinks it heard,
ignoring an uncertain utterance, or responding only to the understood portion. These outcomes need
not be identical because one reflects acoustic evidence and the other reflects the creature's mind.

The game must not silently repair uncertain speech into the most useful command. Some communication
friction belongs in a relationship with a creature. It stops being charming when software failure
is hidden, state is lost, or the player cannot tell how to recover.

## Language development still bounds understanding

Capable recognition must not bypass developmental language. A transcript may perfectly capture
“Could you bring the red thing from beside your bed?” while the creature knows only “bring,” “red,”
a few object labels, and no spatial concept corresponding to “beside.” Its interpretation remains
limited to those learned concepts.

The direction is:

```text
human speech
-> uncertain transcription
-> creature-limited interpretation
```

It is never human speech directly becoming perfect language understanding. Over time the creature
may learn words, pronunciations, nicknames, associations, recurring phrases, and the player's
particular ways of speaking. The desired feeling is: “It understands me this way because I raised
it.”

## Requests are social, not commands

The microphone exists so the player can talk to the creature, not to replace ordinary interface
navigation. Beastie should not become a wake-word utility for opening inventory, saving, setting
timers, controlling the operating system, or executing universal imperatives.

A request to the creature enters normal social and behavioral simulation. The creature may
understand, misunderstand, comply, refuse, forget, become distracted, negotiate, react emotionally,
or do something adjacent. It is not a natural-language command parser with an avatar.

## Latency, overlap, and interruption

Game feel cannot depend on instantaneous recognition. Spoken interaction has several temporal
layers:

- **Immediate:** the creature may notice that speech began.
- **During speech:** it may continue, attend, approach, or react nonverbally.
- **After transcription:** it attempts language interpretation.
- **After interpretation:** it decides whether and how to engage.
- **Later:** speech, action, delayed reaction, or memory may follow.

Technical delay should be absorbed into honest creature behavior whenever it remains legible. The
player should not be left watching a transcription indicator while the creature freezes.

Future design must leave room for overlapping speech and socially meaningful interruption. A
creature may stop speaking, finish anyway, become annoyed, lose its thought, or listen while
finishing an action. Partial recognition should not force it to treat every unfinished fragment as a
final utterance. Beastie should aspire toward interaction between embodied agents, not strict
request-and-response turns.

## Privacy and activation

Microphone audio, transcripts, and creature interactions remain on the player's device for
recognition and cognition. There is no silent cloud recognition fallback. Microphone permission is
explicit and understandable, and the player controls whether microphone interaction is enabled.

Spoken interaction should eventually become low-friction enough to feel natural without implying
surveillance. Background speech does not automatically become creature conversation, and the game
must not suggest that the creature understands everything in the player's real room. The creature
may judge whether speech was probably directed toward it.

No single activation experience is canonical yet. Explicit push-to-talk or another bounded mode
may remain available even if more natural modes are added. Enabling speech must not imply indefinite
background capture.

## Graceful failure and legibility

No speech detected, incomplete or uncertain transcription, unavailable recognition, missing
microphone permission, disconnected input, slow recognition, and unsupported language must never
block simulation or corrupt state. Text keeps the creature coherent and fully playable.

The creature may fail to notice, look confused, request repetition, react nonverbally, hear only
part, or remain silent. The interface may also expose a practical failure, particularly for
accessibility and debugging. Do not fictionalize every infrastructure failure as creature behavior:
charming misunderstanding and broken software must remain distinguishable.

## Text remains excellent

Anything meaningful the player communicates through voice should generally remain possible through
text. Text follows the same creature behavior and interpretation rules while avoiding only the
properties intrinsic to live audio.

Text remains essential for accessibility, noisy environments, players who decline microphone use,
privacy, speech differences, personal preference, development, automated tests, and deterministic
scripted scenarios. No core progression may be gated behind speech.

## Reserved extension points

Future local perception may softly notice laughter, tone or prosody, hesitation, a nickname,
clapping, or other simple sounds. These are bounded signals, not omniscient emotion detection. They
must not establish the player's emotional state as fact, override creature-level interpretation, or
mutate world truth.

Future work may also deepen attention, habituation, interruption, delayed responses, and learning
from the player's speech patterns. These possibilities should remain open without being claimed as
V2 scope.

## Design review questions

Any future spoken-interaction feature must answer:

1. Does this make interaction feel more like sharing space with a creature?
2. Would the creature behave essentially the same if the same words were typed?
3. What can the creature perceive before transcription completes?
4. Does speech interrupt according to attention, or summon an always-listening assistant?
5. Can the creature ignore, misunderstand, refuse, or delay?
6. Does simulated language knowledge still bound understanding?
7. Is recognition treated as uncertain perception rather than truth?
8. Does the simulation retain authority over state and consequences?
9. Does runtime interaction remain local with no silent cloud service?
10. Is text still a complete fallback?
11. What happens when recognition is slow or unavailable?
12. Does the feature deepen attachment, embodiment, personality, or individual difference?
13. Could it produce a story about this creature rather than demonstrate recognition quality?

If the primary benefit is merely that voice commands are convenient, the feature probably does not
belong in Beastie.
