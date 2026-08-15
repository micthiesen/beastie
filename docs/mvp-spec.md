# Working Title “Beastie”: MVP Product and Technical Specification

## Product thesis

The MVP should be **a one-room, persistent creature game in which the creature is a small deterministic simulation with an unreliable little language faculty attached to it**.

The most important design rule is:

> **AI creates expression and surprise; the simulation creates truth.**

The LLM should not decide whether the creature is hungry, what happened yesterday, whether the player fed it, what objects exist, or how much it likes the player. Rust owns all of that. The model is given a tiny, curated view of those facts and turns them into something the creature might say.

That gives the game a much stronger identity than “Tamagotchi with ChatGPT”:

> You feed a strange 32×32 animal.<br>
> It develops preferences and memories.<br>
> Eventually it starts trying to talk to you.<br>
> It is never especially good at talking.

The awkwardness of a sub-billion-parameter model and tiny local TTS should be **canon rather than technical debt**. A creature saying:

> “You bring round food again. I think round is trust.”

is preferable to polished assistant prose.

I would make the entire runtime offline. No account, cloud endpoint, subscription, API token, or GPU is necessary. Steam's current rules allow both pre-generated AI assets and live-generated AI; live generation requires disclosure of the guardrails used to prevent illegal output. That makes the proposed architecture viable, while the PixelLab-generated development art would fall into Steam's pre-generated category. citeturn18search2

### The central design principles

**The simulation is the brain; the LLM is the mouth.** The creature should already have needs, preferences, memories, relationships, intentions, and an action-selection system before an LLM is connected.

**Limitations are characterization.** Short context, restricted concepts, synthetic speech and imperfect grammar should make the animal charming. Do not continuously chase larger models.

**Small responses are better responses.** Most utterances should be perhaps 3–15 words, with a hard ceiling around 24 words. This simultaneously improves pacing, inference latency, model reliability and the game's aesthetic.

**Every AI statement should have something behind it.** “I don't like the blue food” is interesting if the simulation contains `preference.blueberry = -0.72`. Random AI personality isn't.

**The creature should surprise the player without surprising the program.** The player should occasionally think “I can't believe it remembered that”; the game should be able to identify exactly which stored event produced the comment.

**No death in the MVP.** Neglect can produce hunger, boredom, sleepiness, resentment and weird behaviour, but returning after a week should not reveal a tombstone. The game is about becoming attached to an odd creature, not servicing a notification schedule.

**Dumbness should be authored, not merely tolerated.** Restricted concepts, mistaken beliefs, short responses and peculiar grammar are part of the creature. Broken protocol output, incoherent word soup and assistant-like filler are ordinary failures and should be rejected.

**The creature is allowed to become unpleasant.** It may learn profanity, crudeness, spite and provocation when those behaviours get results. The player can slowly change those habits, but there is no personality editor, morality meter or instant reset for the creature they raised.

I would target a **30–45 minute initial arc spread across three short in-game days**, after which the save continues indefinitely. One active in-game day being roughly 15 minutes is enough for the MVP to demonstrate development without making evaluation painfully slow.

## The game MVP

The entire playable world is one 320×180 logical-resolution room, nearest-neighbour scaled to the player's display. The creature itself is roughly 32×32 pixels. This deliberately keeps content production tiny enough that the interesting engineering can go into behaviour rather than map creation.

There is no player avatar. The player interacts with the room almost as though looking into a little enclosure.

### The room and camera

Use a fixed dollhouse view: mostly straight-on, with enough three-quarter depth to give the floor and furniture volume. The player should see the creature's face and posture clearly wherever it moves. Do not use a top-down, isometric or platformer view.

The camera never pans, zooms or follows the creature during ordinary play. The whole room remains visible, with stable positions for the bowl, bed, toy and window. Nearest-neighbour scaling may letterbox the 320×180 canvas to preserve square pixels and the intended composition.

The room provides most of the visual change. Daylight, lamplight, weather at the window, clutter and small creature-made alterations can shift its mood without changing the layout. It should feel like a place the player checks on, not a level they traverse.

### What exists in the room

The first build needs only four meaningful objects:

| Object | Gameplay |
|---|---|
| Bowl | Feed the creature one of a few foods |
| Bed/nest | Creature sleeps here autonomously |
| Toy | Raises curiosity/fun; creature can develop an opinion about it |
| Window | Passive source of events such as rain, night, birds or light |

The player has five primary verbs:

**feed, play, comfort, tidy, talk.**

That is enough.

### The interface

Keyboard/mouse and controller are both first-class. The mouse selects the creature or an object directly. Controller focus moves between the same targets with a clear but quiet highlight. There is no cursor avatar and no movement control for the creature.

Selecting a target reveals only the actions that make sense there, either beside it or in a small strip along the bottom edge. Feed can open a tiny food choice; Play can do the same for toys. Talk alone opens text entry. Physical keyboards type directly; controller-only play uses a compact modal on-screen keyboard. Speech reactions such as laugh, disapprove and comfort sit directly beneath the current speech bubble, then disappear with it.

The interface should feel attached to the enclosure: chunky pixel icons, short labels, compact dialogue frames and restrained electronic sounds. It can borrow the tactility of an old virtual-pet device without drawing a plastic console around the whole game. Controls must remain legible overlays rather than hiding essential actions inside decorative scenery.

When the player is watching rather than acting, most interface chrome fades away. Do not show permanent need bars, relationship meters, floating quest markers or a minimap. Hunger, affection and resentment belong in the creature's behaviour.

No inventory system beyond a tiny contextual selection of food/toys. No crafting. No world map. No breeding. No combat. No procedural environments.

### Creature simulation

The persistent creature state should look approximately like this:

```rust
pub struct Creature {
    pub identity: CreatureIdentity,
    pub needs: Needs,
    pub mood: Mood,
    pub traits: Traits,
    pub relationship: Relationship,
    pub preferences: Preferences,
    pub known_concepts: ConceptSet,
    pub memories: Vec<Memory>,
    pub beliefs: Vec<Belief>,
    pub social_habits: SocialHabits,
    pub current_intention: Intention,
    pub position: GridPosition,
    pub development: Development,
}

pub struct Needs {
    pub hunger: f32,
    pub energy: f32,
    pub comfort: f32,
    pub curiosity: f32,
}

pub struct Traits {
    pub sociability: f32,
    pub boldness: f32,
    pub fussiness: f32,
    pub stubbornness: f32,
    pub literalness: f32,

    // Primarily affects language generation.
    pub repetitiveness: f32,
    pub sentence_complexity: f32,
    pub question_tendency: f32,
}

pub struct Relationship {
    pub bond: f32,
    pub trust: f32,
    pub respect: f32,
    pub resentment: f32,
}

pub struct SocialHabits {
    pub profanity: f32,
    pub crudeness: f32,
    pub spite: f32,
    pub provocation: f32,
    pub sexual_innuendo: f32,
}
```

Those values are not directly exposed as bars. I would strongly resist making this look like a mobile-game dashboard. Needs should mostly be communicated through behaviour: staring at the bowl, climbing into bed, dragging the toy toward the player, making an annoyed noise.

The initial creature genome is derived from the save seed, so two players' animals genuinely behave somewhat differently without requiring model-generated personality.

### Autonomous behaviour

Do **not** have the LLM choose what the creature does.

A simple utility system is ideal:

```text
score(eat)
    = hunger
    × food_available
    × food_preference

score(sleep)
    = tiredness
    × bed_comfort

score(play)
    = curiosity
    × toy_interest
    × sociability_modifier

score(approach_player)
    = loneliness
    × bond
    × sociability
```

The highest-scoring action wins with enough noise and hysteresis to avoid robotic oscillation.

That system is easy to inspect, deterministic under a seed, and can be simulated millions of ticks without rendering anything. It also means a bug such as “creature hasn't slept in 72 simulated hours” is an ordinary Rust test rather than an LLM-evaluation problem.

### The player can reinforce bad behaviour

The creature's social habits change according to what works. Repeatedly serving hated food can build resentment. Indulging every tantrum can teach entitlement. Continuing to engage after an insult can teach that provocation earns attention.

The five primary verbs remain unchanged, but some utterances expose a small contextual reaction:

```text
[laugh]  [disapprove]  [comfort]
```

These are not transparent training buttons. A laugh usually reinforces profanity or provocation, but a stubborn creature may also escalate when scolded. Comfort after a tantrum may teach that tantrums produce affection. The same response can land differently according to the creature's traits, current relationship and recent memories.

None of this is shown as `Spite +2`. The player learns what they have encouraged by living with the result.

Personality must also exist outside speech bubbles. The MVP needs a few cheap, state-driven expressions of spite or stubbornness: pushing disliked food out of the bowl, taking the toy away from the player, staring while refusing to eat, or undoing part of a recent tidy. A silent asshole is often funnier than a verbose one.

Rehabilitation is possible but slow. There is no personality reroll or instant “be nice” item. Starting a new save remains possible, but the existing creature keeps the habits it learned.

### Memories are events, not prose

I would make memories extremely concrete:

```rust
pub struct Memory {
    pub id: MemoryId,
    pub happened_at: GameTime,
    pub kind: MemoryKind,
    pub entities: Vec<EntityRef>,
    pub concepts: Vec<Concept>,
    pub valence: f32,
    pub salience: f32,
}

pub enum MemoryKind {
    WasFed { food: FoodId },
    PlayedWith { toy: ToyId },
    WasComforted,
    PlayerReturnedAfterAbsence,
    DislikedFood,
    SawWeather { weather: Weather },
    LearnedName { object: EntityRef, name: String },
}
```

There is **no vector database in the MVP** and no LLM-authored canonical memory.

When the player talks, the dialogue layer can supply perhaps the eight most plausible memories based on salience, recency and relevant concepts. The model is allowed to reference one of those IDs. It cannot invent `memory_9481` and thereby establish that something happened.

That gives you the wonderful emergent moment:

> PLAYER: Do you remember the berries?
>
> CREATURE: yes. bitter red betrayal.

while the engine knows that the comment came from:

```text
memory #42
kind: WasFed(strawberry)
preference at event: -0.83
valence: -0.62
player involved: true
```

### Beliefs can be wrong

Memories record what happened. Beliefs record what the creature thinks those events mean.

```rust
pub struct Belief {
    pub id: BeliefId,
    pub proposition: BeliefKind,
    pub supporting_memories: Vec<MemoryId>,
    pub confidence: f32,
}

pub enum BeliefKind {
    RedFoodIsATrick,
    WindowMakesRain,
    PlayerReturnsAfterSleep,
    ToyIsJealous,
}
```

The exact enum names will change, but the boundary matters. If the player gave a berry while it was raining, the creature may conclude that red food comes from bad weather. That theory is allowed to be absurd. It is still deterministic, traceable to real memories and revisable when later evidence contradicts it.

The LLM does not invent canonical beliefs. Rust derives candidate beliefs from events, traits and seeded randomness, then supplies a selected belief for expression. This lets the creature misunderstand reality without letting model hallucinations rewrite reality.

### Language development

I would **not** pretend the neural network itself is learning over the course of a save. No online fine-tuning for the MVP.

Instead, the creature gains **semantic concepts**.

At hatching:

```text
SELF
YOU
FOOD
GOOD
BAD
HERE
SLEEP
```

Later:

```text
TOY
WINDOW
RAIN
AGAIN
YESTERDAY
GIVE
MISS
NAME
FRIEND
WHY
```

These are *conceptual* limits, not literal vocabulary limits. English glue words are still allowed. Otherwise tiny-model output becomes painfully difficult to constrain.

This gives the model a progression curve. Early:

> “food. you good.”

Later:

> “You gave that red thing yesterday. I remember bad.”

Much later:

> “I think the window makes rain because it misses outside.”

The important thing is that increasing apparent intelligence now corresponds to **game progression**, despite using exactly the same model throughout.

I would also give every save an **idiolect genome**. It controls things such as article dropping, repetition, preferred sentence lengths, favourite constructions and occasional word substitution. A deterministic post-processing stage can enforce some of these quirks instead of hoping sampling happens to produce them consistently.

For example:

```rust
pub struct Idiolect {
    pub article_drop: f32,
    pub repetition: f32,
    pub malformed_questions: f32,
    pub favourite_connective: Option<String>,
    pub pause_frequency: f32,
}
```

One creature may routinely say:

> “Probably bed now. Probably.”

Another:

> “Bed is wanted? By me?”

That's much more memorable than merely changing temperature.

### Conversation should be scarce

Beastie is not a chat application with a pixel animal attached. A Talk action produces one short response and, at most, one contextual follow-up. The creature may answer with a gesture, a noise or silence. It can ignore questions that do not connect to anything it knows.

Creature-initiated speech also needs a reason: an active intention, a current observation, a recalled memory or a belief worth expressing. There should be quiet stretches. The player ought to want the creature to talk slightly more often than it does.

### Hatching is not childhood

Beasties hatch fully formed. They are strange, ageless animals, not children. Their early progression represents language acquisition and relationship formation rather than physical or sexual maturation.

The art, writing and voice direction should avoid baby terminology, school-age metaphors and an explicitly juvenile voice. Crude or sexual humour remains unavailable during the earliest language phase and, later, stays vulgar or absurd rather than seductive. The intended joke is that an electronic animal has learned an upsetting word, not that the game is offering erotic roleplay.

### The MVP progression arc

The first three active days should prove the entire idea:

| Phase | New behaviour |
|---|---|
| Hatch | Creature communicates through animation/noises and a handful of words |
| Familiarity | Develops food/toy preferences and begins associating the player with events |
| Recognition | Recalls a previous interaction in dialogue |
| Attachment | Reacts differently when the player returns and gains richer social concepts |
| Individuality | Idiolect, beliefs and learned social habits make this creature identifiable |

There does not need to be a conventional victory screen. The vertical slice is successful when the player experiences a genuine **“oh, this particular little idiot knows me”** moment.

A strong acceptance scenario is:

```text
new game
→ creature hatches
→ player gives berry
→ creature dislikes berry
→ player plays with creature
→ player has a brief conversation
→ creature sleeps
→ save and quit
→ reload
→ later conversation mentions the disliked berry
→ wording is peculiar to this creature
→ creature insults the player
→ player laughs
→ later behaviour becomes more provocative
→ dislike also appears through a nonverbal action
```

If that is delightful, the game works.

## Technical stack

My recommendation is somewhat different from the obvious “use Bevy” answer:

> **Use ggez 0.10 for the game shell, and deliberately keep almost all game logic outside ggez.**

That hits the niche-Rust-library criterion surprisingly well. ggez describes itself as a lightweight, cross-platform Rust game framework with 2D/3D rendering, sound, resource loading and input, while deliberately *not* imposing an ECS, physics system or other higher-level architecture. Its current repository uses `ggez = "0.10.0"`, fully supports Windows and Linux, uses `wgpu` for rendering and `rodio` for audio, and is MIT-licensed. citeturn23view0

That is almost exactly the abstraction level this game wants.

### Why ggez is my pick

| Option | What I like | Why I would/wouldn't use it |
|---|---|---|
| **ggez 0.10** | Small framework, pure Rust, wgpu renderer, sound included, no imposed ECS, Windows/Linux first-class | **Pick this.** The creature simulation stays explicit and ordinary Rust |
| Macroquad 0.4 | Extremely small surface, automatic 2D batching, immediate-mode UI, few dependencies, Windows/Linux | Excellent second choice; slightly more “tiny game loop” and less structural separation than I want here |
| Bevy | Excellent ECS and broad ecosystem/platform support | Great engine, but this particular game gains little from making its tiny world an ECS-first architecture |

Macroquad is genuinely attractive: its own documentation emphasizes minimal dependencies, automatic batching, built-in immediate-mode UI and Windows/Linux support. citeturn19search0turn19search2 Bevy likewise supports Windows and Linux along with the other major platforms. citeturn15search5

The reason I prefer ggez isn't “boring reliability.” It's that **not having an engine architecture is advantageous here**. The creature is a little state machine/simulation that happens to be visualized as a game. An `update()` / `draw()` framework atop wgpu is enough. ggez explicitly positions itself this way: a batteries-included base that does not dictate ECS or physics. citeturn23view0

### Repository architecture

I would establish the boundaries immediately:

```text
beastie/
├── Cargo.toml
├── crates/
│   ├── beastie-core/          # pure deterministic game simulation
│   ├── beastie-protocol/      # AI request/response types
│   ├── beastie-ai-worker/     # LLM + TTS process
│   ├── beastie-view/          # simulation -> render/audio plans
│   ├── beastie-game/          # thin ggez executable
│   └── xtask/                 # developer/agent CLI
│
├── assets/
│   ├── manifest.toml
│   ├── style/
│   ├── generated/
│   └── final/
│
├── models/
│   └── manifest.toml          # hashes/metadata; not giant weights in git
│
├── fixtures/
│   ├── saves/
│   ├── dialogue/
│   └── render/
│
└── evals/
    ├── dialogue/
    └── reports/
```

`beastie-core` should have **no dependency on ggez, an LLM library, ONNX, system time, filesystem access or graphics**.

Its fundamental API can be almost absurdly simple:

```rust
pub fn step(
    state: &mut WorldState,
    input: &[PlayerEvent],
    dt: GameDuration,
    rng: &mut impl Rng,
) -> Vec<GameEvent>;
```

That decision is more consequential for agent productivity than the engine choice.

An agent can change hunger behaviour and immediately run:

```bash
cargo test -p beastie-core
```

It doesn't need a display server, GPU, 500 MB model, TTS runtime or API key.

### An unusual but important choice: make rendering declarative

I would also avoid having game state directly call ggez drawing functions.

Have `beastie-view` produce this:

```rust
pub struct RenderPlan {
    pub sprites: Vec<SpriteCommand>,
    pub text: Vec<TextCommand>,
    pub ui: Vec<UiCommand>,
}

pub struct AudioPlan {
    pub events: Vec<AudioCommand>,
}
```

The ggez executable simply executes those commands.

That means an agent modifying the “creature sleeps beneath its blanket” feature can get a test result like:

```json
{
  "sprites": [
    {"id":"bed","x":190,"y":104,"layer":2},
    {"id":"creature_sleep","x":194,"y":101,"layer":3},
    {"id":"blanket","x":192,"y":108,"layer":4}
  ]
}
```

without having to visually inspect a framebuffer.

Actual screenshots still matter, but most visual regressions now become ordinary snapshot-testable data.

### Saves

Use **versioned, human-readable JSON for MVP saves**.

Not because JSON is technically optimal—it isn't—but because it gives humans and coding agents an incredible debugging interface.

A bug report can include:

```json
{
  "save_version": 3,
  "seed": 891522,
  "creature": {
    "hunger": 0.76,
    "bond": 0.41,
    "current_intention": "StareAtBowl"
  }
}
```

and an agent can reproduce it immediately.

Binary save formats can come later if there is ever a reason.

## Local AI and voice architecture

This is where I would make the most opinionated engineering decision:

> **Do not link the heavy AI stack into the game executable. Ship it as a local child process speaking a tiny versioned protocol over stdin/stdout.**

That gives you:

```text
┌───────────────────────────┐
│ beastie-game              │
│ ggez + simulation         │
│                           │
│ does NOT know what        │
│ Qwen / GGUF / ONNX are    │
└────────────┬──────────────┘
             │ JSONL/stdin/stdout
             ▼
┌───────────────────────────┐
│ beastie-ai-worker         │
│                           │
│ DialogueBackend           │
│   ├── mistral.rs          │
│   └── llama.cpp fallback  │
│                           │
│ SpeechBackend             │
│   └── sherpa-onnx         │
└───────────────────────────┘
```

This is a **much better development boundary** than putting `mistralrs` and ONNX dependencies in the game's dependency tree.

Changing sprite behaviour doesn't rebuild the ML runtime. An inference crash doesn't kill the simulation. Tests can replace the process with a ten-line fake. An agent can replay recorded JSON into the worker. Windows and Linux can each receive a different worker binary if necessary while sharing the exact same game logic.

Using stdin/stdout rather than localhost HTTP also means the runtime opens no network port.

### LLM choice

My starting model would be:

> **Qwen3.5-0.8B, quantized to roughly four-bit weights, with very short generations.**

Qwen's official 0.8B model was released in March 2026 and is explicitly positioned for prototyping and task-specific development. Its official repository is Apache-2.0 licensed. citeturn15search4turn15search16

At 0.8 billion parameters, pure four-bit weight arithmetic is only about:

\[
0.8 \text{ billion} \times 4 \text{ bits} \div 8 \approx 400\text{ MB}
\]

before GGUF metadata, tensors that use other formats, KV cache and runtime overhead. So this is in the right size class for a game that bundles its model rather than downloading several gigabytes.

I would produce the shipping GGUF from the **official upstream weights** rather than grabbing an arbitrary community quantization. `llama.cpp`'s official tooling documents the conversion-to-GGUF and subsequent quantization workflow. citeturn17search3

For inference, I would begin with **mistral.rs**. It has a native high-level Rust SDK and is explicitly designed as an LLM inference runtime; its current project includes hardware-aware tooling and a Rust crate rather than requiring a Python bridge. citeturn16search1turn16search6

But—and this is important—I would make `DialogueBackend` replaceable from day one. `llama.cpp` is the fallback candidate because its explicit objective is minimal-setup local inference across a broad range of hardware. citeturn17search19

So this is a fun Rust bet without betting the entire project on it:

```rust
pub trait DialogueBackend {
    fn generate(
        &mut self,
        request: DialogueRequest
    ) -> Result<DialogueReply, DialogueError>;
}
```

The first serious performance spike should benchmark the exact Qwen quantization under both runtimes on CPU. Whichever wins on your low-end test PCs gets the release backend; nothing above the worker layer changes.

I would **not fine-tune anything for the MVP**. Prompt engineering plus deterministic idiolect processing is enough. Fine-tuning becomes interesting only once actual playtesting has produced hundreds or thousands of examples of “this sounded like an Beastie / this did not.”

Model selection must include voice freedom as well as speed and protocol compliance. A local model has no provider-side moderation service, but post-training can still make it refuse permitted profanity, insults or innuendo. The evaluation corpus should measure that refusal rate directly. Start with the official post-trained model; benchmark the base model only if refusals are a material problem. Do not make a community “uncensored” fine-tune the default without separately auditing its provenance, licence, structured-output reliability and prohibited-output rate.

### What the LLM actually receives

Crucially, don't dump the whole save into the prompt.

A dialogue request might be:

```json
{
  "request_id": 184,
  "creature": {
    "name": "Mop",
    "mood": "sleepy",
    "relationship": {
      "bond": "fond",
      "trust": "medium",
      "respect": "low",
      "resentment": "high"
    },
    "traits": ["literal", "stubborn", "repetitive"]
  },

  "current_observations": [
    "player is present",
    "food bowl is empty",
    "it is raining",
    "you are tired"
  ],

  "known_concepts": [
    "self", "you", "food", "bad", "good",
    "rain", "bed", "again", "yesterday"
  ],

  "candidate_memories": [
    {
      "id": 41,
      "fact": "Yesterday the player gave you a strawberry.",
      "feeling": "strong dislike"
    },
    {
      "id": 49,
      "fact": "The player comforted you during rain.",
      "feeling": "good"
    }
  ],

  "candidate_beliefs": [
    {
      "id": 7,
      "belief": "Red food is probably a trick.",
      "confidence": "high",
      "supporting_memories": [41]
    }
  ],

  "player_said": "Do you like the rain?",

  "constraints": {
    "max_words": 24,
    "tone": "affectionate_hostile",
    "profanity": "allowed",
    "crudeness": "allowed",
    "sexual_innuendo": "non_graphic",
    "allowed_gestures": [
      "none", "look_player", "look_window",
      "shiver", "sleepy"
    ]
  }
}
```

And the only acceptable result is something like:

```json
{
  "say": "Rain bad alone. With you maybe less bad.",
  "gesture": "look_window",
  "recalled_memory": 49
}
```

Every enum and referenced ID is validated.

If it emits:

```json
{
  "say": "...",
  "gesture": "launch_nuclear_missile",
  "recalled_memory": 9999999
}
```

the parser simply rejects those fields.

**No model output directly changes simulation state.**

Even `recalled_memory` is presentation metadata. A reply cannot magically make a nonexistent memory true.

### Do not make the LLM a tool-using agent

For the MVP, talking is a **social action only**.

If the player types:

> eat the red berry

the creature is free to reply:

> “No. red betrayal.”

but it doesn't invoke an `eat()` tool.

Actual world manipulation remains buttons/clicks and autonomous simulation behaviour.

That removes an enormous amount of complexity: no prompt injection into tools, no accidental arbitrary actions, no model needing to understand coordinates, and no testing of autonomous agent plans.

There is plenty of game here without it.

### Handling inference latency diegetically

The main loop never waits for inference.

When the player speaks:

```text
submit DialogueRequest
        │
        ├── creature turns toward player
        ├── blink / thinking animation
        ├── small "mrr?" sound
        │
        ▼
LLM result arrives
        │
        ├── speech bubble appears
        └── TTS request begins
                │
                ▼
             voice plays
```

Text can appear before TTS is finished.

A slower CPU therefore doesn't necessarily feel broken. The creature simply pauses before constructing a thought—which is completely plausible for this thing.

There should be one outstanding conversational generation at most. New talk can cancel or supersede an old generation.

### Graceful failure is part of the personality

A full fallback vocabulary should exist:

```text
"hm."
"no words."
"again?"
"thinking broke."
"mrrp."
"too many thought."
```

If the worker dies, the model fails to load, structured output fails repeatedly, or inference exceeds a timeout, the game remains playable.

I would actually make one of those failures intentionally visible enough that players can perceive the creature as having failed to find its words rather than perceiving the software as broken.

### TTS choice

**KittenTTS is unusually well matched to this game.**

The current v0.8 project has 15M, 40M and 80M parameter variants occupying roughly 25–80 MB, runs through ONNX on CPU without requiring a GPU, includes eight voices and generates 24 kHz audio. The project explicitly calls itself a developer preview, so pinning exact versions matters. citeturn16search2

For the MVP I would start with the **15M nano/int8 class**. Do not assume the largest voice sounds “better for the game”; audition all three with actual creature dialogue.

For Rust integration, **sherpa-onnx** is particularly interesting. It is a self-contained ONNX-based speech runtime, supports text-to-speech models and has KittenTTS integrations; its current changelog also contains Rust TTS API examples. citeturn17search10turn17search2turn17search25 Kitten v0.8 changed its ONNX format relative to the original release, and v0.8 support has subsequently been worked into sherpa-onnx, so I would pin a tested sherpa revision rather than casually tracking master. citeturn17search1turn17search17

That gets us to an important release property:

> **The shipped game should contain zero Python.**

Python is fine for development/art tooling, but the player receives:

```text
beastie.exe
beastie-ai-worker.exe
qwen.gguf
kitten.onnx
assets/...
```

or the corresponding Linux binaries.

### Give each creature a voice genome

Don't just select a TTS voice.

Persist something like:

```rust
pub struct VoiceGenome {
    pub base_voice: VoiceId,
    pub speed: f32,
    pub pitch_shift_semitones: f32,
    pub pause_scale: f32,
    pub crunchy_voice: f32,
}
```

Then run deterministic lightweight DSP after synthesis.

A small amount of bandwidth reduction, pitch shifting, irregular pause insertion or deliberate low-fi processing can produce much more identity than switching to a huge expressive speech model.

The goal is emphatically **not ElevenLabs-quality narration**.

The goal is:

> “some electronic animal has acquired the ability to say my name, unfortunately.”

Cache synthesized audio using a hash of:

```text
(model + voice genome + utterance)
```

so repeated barks don't repeatedly consume CPU.

## AI-assisted pixel-art pipeline

For the MVP, I would use **PixelLab as the canonical AI asset generator**, specifically because it already supplies the thing you asked for: an official MCP workflow intended for coding assistants.

PixelLab's MCP server is explicitly designed so an AI coding assistant can create pixel-art characters, animations and tilesets from the development environment. Its broader product supports sprites, animations, directional views, environments, tilesets and style-consistent generation. citeturn16search0turn16search3

That is a much cleaner choice than wiring an agent to ComfyUI and hoping it learns how your particular graph works.

### Build an asset contract, not an assets folder

Every asset should originate in a manifest:

```toml
[[asset]]
id = "creature.base.idle"
kind = "animation"

width = 32
height = 32
frames = 6

prompt = """
small pear-shaped pet creature,
oversized dark eyes,
short round legs,
slightly awkward posture,
front three-quarter view,
cozy strange virtual-pet aesthetic
"""

style_reference = "style/creature_anchor.png"
palette = "style/main.hex"
transparent = true
seed = 5912203

generator = "pixellab"
status = "generated"
```

The files themselves are implementation outputs.

```text
assets/
    generated/creature.base.idle.png
    final/creature.base.idle.png
```

Resolution rule:

```text
assets/final/<id>      if present
else
assets/generated/<id>
```

That makes later humanization exceptionally easy.

An artist can replace:

```text
generated/creature.base.idle.png
```

with:

```text
final/creature.base.idle.png
```

without touching a line of game code or changing the asset ID.

### Let both humans and agents invoke generation

There should be two paths to exactly the same manifest.

For an agent:

```text
read manifest
→ call PixelLab MCP
→ save generated result
→ run asset validator
```

For a terminal:

```bash
cargo xtask asset generate creature.base.idle
```

The latter can call PixelLab's HTTP API using an environment credential. PixelLab exposes an API specifically for programmatic pixel-art image, rotation and animation generation. citeturn16search7

Its current image tooling supports explicit canvas sizes, transparent backgrounds, multiple reference images, a separate style image and seeds, which are exactly the controls needed to keep generated game assets coherent rather than accumulating fifty unrelated AI aesthetics. citeturn18search3

### Start with a tiny art bible

Before generating dozens of things, approve:

```text
style/main.hex
style/creature_anchor.png
style/furniture_anchor.png
style/ui_anchor.png
```

The visual direction is **cozy-grotty**: warm lamplight, deep shadows, scuffed furniture and a mild electronic unease. The creature is cute because it is awkward and expressive, not because it has been polished into a mascot. Keep silhouettes readable and allow ugly, sulky or unsettling poses when the simulation calls for them.

Use a 16–24 colour working palette with warm, dirty neutrals and a few sharp electronic accents. Animation should be chunky and restrained. A held stare, one irritated foot movement or an object shoved a few pixels can carry more personality than constant squash-and-stretch motion.

The window is the room's main source of visual variety: daylight, night, rain, birds, odd silhouettes and changes in ambient colour. These events should alter the mood while preserving the fixed composition.

Then agents are required to provide the style anchor to all subsequent generations.

PixelLab explicitly supports reference-guided and style-consistent generation, so this workflow matches the service rather than fighting it. citeturn16search3turn18search3

The MVP art inventory can remain very small:

| Category | Rough scope |
|---|---:|
| Creature | idle, walk, eat, sleep, play, annoyed, happy |
| Room | one background / tile set |
| Furniture | bed, bowl, toy, window |
| Food | 3–4 items |
| UI | action icons, dialogue frame |
| Effects | hearts, question mark, sleep particles |

With a 32×32 creature and restrained animation, this is enough for a convincing vertical slice.

### Asset validation should be automated

After generation:

```bash
cargo xtask asset check
```

should verify things such as:

```text
✓ expected filename
✓ exact frame dimensions
✓ expected number of frames
✓ alpha channel
✓ no accidental gigantic canvas
✓ palette compliance/tolerance
✓ manifest contains generator provenance
✓ source prompt recorded
✓ source seed recorded
✓ style reference exists
```

That is important for agentic development. “I made the sprite” isn't sufficient; the agent gets a binary success/failure condition.

A generated sprite with the wrong dimensions should fail CI just like malformed Rust.

### Preserve provenance from day one

This is useful both operationally and for eventual Steam submission.

PixelLab's current terms state that users own copyright in their creations and permit commercial and non-commercial usage. That is favourable for this workflow, though it does **not** remove your separate responsibility to avoid infringing material. citeturn18search0

Steam explicitly requires disclosure of AI-generated assets that ship with the product and evaluates them under the same illegal/infringing-content commitments as other shipped content. citeturn18search2

So retaining:

```text
generator
generation date
prompt
seed
reference images
human modifications
licence/terms snapshot
```

per asset costs nearly nothing and saves a great deal of forensic pain later.

## Development experience and agent feedback loop

This is the area I would optimize **aggressively**. The repository should be structured so that an agent rarely has to say “I changed it; please run the game and see whether it works.”

It should usually be able to prove its change itself.

### One command is the contract

Put all developer operations behind an `xtask` crate:

```bash
cargo xtask dev
cargo xtask dev --fake-ai
cargo xtask verify
cargo xtask sim --seed 42 --days 100
cargo xtask play --seed 42 --fake-ai
cargo xtask play --scenario fixtures/scenarios/berry-grudge.jsonl --fake-ai
cargo xtask dialogue replay berry-memory
cargo xtask dialogue eval
cargo xtask tts smoke
cargo xtask asset check
cargo xtask asset generate <id>
cargo xtask capture <fixture>
cargo xtask package
```

`cargo xtask verify` is the most important one.

It should run, in order:

```text
rustfmt check
clippy
core unit tests
simulation invariants
save/load fixtures
AI protocol tests
render-plan snapshots
asset validator
Linux/Windows-appropriate packaging sanity checks
```

and it should **not require a real model, PixelLab credential or GPU**.

### Give agents semantic control of both game loops

Use one versioned command protocol for a reusable `GameSession`, then expose it through two adapters:

- a headless JSONL runner for fast deterministic interaction and inspection;
- the real ggez shell, driven by the same scenario files at deterministic frame boundaries.

Commands express actions such as Feed, Talk, React, Advance, Inspect and Capture rather than mouse
coordinates. The visible adapter captures the 320×180 logical framebuffer directly to PNG. Start
with scenario files instead of a socket; add a development-only live socket later only if measured
workflow friction justifies it. Keep a small separate host-input suite for actual mouse mapping,
keyboard text entry and controller navigation.

The exact protocol, evidence contract and pre-build proof are specified in
[development-harness.md](development-harness.md).

### Make fake AI a first-class backend

The default automated test backend should understand fixture syntax such as:

```json
{
  "when": {
    "mood": "hungry",
    "player_contains": "berry"
  },
  "reply": {
    "say": "red food remains crime.",
    "gesture": "look_player",
    "recalled_memory": 41
  }
}
```

That allows an agent to implement the entire conversation UI while the 0.8B model is not even installed.

Then:

```bash
cargo xtask dev --fake-ai
```

starts essentially instantly.

Real-model behaviour is evaluated separately.

### Treat the LLM like an untrusted external component

Maintain a fixture corpus:

```text
evals/dialogue/
    greeting.json
    hungry.json
    asks_about_memory.json
    asks_unknown_fact.json
    adversarial_input.json
    long_input.json
    no_memories.json
    low_vocabulary.json
    sleepy.json
    dislikes_food.json
```

A real-model evaluation executes the corpus and records:

```text
parse success
latency
word count
referenced memory IDs
invalid enum rate
forbidden-content filter hits
fallback rate
generated response
```

It should also score the qualities that make the game worth building:

```text
specificity to the supplied creature and event
grounded surprise
generic-assistant voice rate
repetition across equivalent prompts
refusal rate for permitted profanity and innuendo
prohibited-content escape rate
```

Responses such as “How can I help?”, polished therapy language, generic emotional validation and excessive politeness are failures even when they parse correctly. The target is a peculiar animal with something specific to express, not a tiny customer-support representative.

That gives an agent a meaningful before/after result when it edits a system prompt:

```text
previous
    valid outputs:       96/100
    invented memories:    7/100
    median words:        19

candidate
    valid outputs:      100/100
    invented memories:    0/100
    median words:        11
```

Now prompt changes can be reviewed more like code changes.

I would commit the prompt itself as a versioned source file rather than burying a giant string inside Rust.

### Determinism everywhere except where deliberately absent

Inject:

```text
game clock
wall clock
RNG
dialogue backend
TTS backend
filesystem abstraction where useful
```

Never have the simulation itself call `SystemTime::now()`.

Then this becomes possible:

```bash
cargo xtask sim \
    --seed 92812 \
    --days 1000 \
    --policy fixtures/player-policies/normal.json
```

and it can test:

```text
needs always remain in range
creature never gets stuck in an impossible action
creature eventually sleeps
relationship cannot become NaN
all memory references remain valid
save/load preserves state
offline progression cannot kill/permanently ruin pet
```

A thousand virtual days should take seconds, not weeks.

### Test render intent instead of GPU pixels

As described earlier, snapshot the `RenderPlan`.

For example:

```text
fixture: sleeping_at_night

expected:
    room/night
    bed
    creature/sleep/frame_2
    blanket
    particles/z_1
    ui/needs-hidden
```

This is vastly less brittle across Linux/Windows, GPUs and wgpu backends than demanding bit-identical framebuffer results.

Still add:

```bash
cargo xtask capture sleeping_at_night
```

which renders a fixture to PNG for human/vision-model review.

An agent can then both inspect the structured plan **and** inspect the resulting picture.

### AI worker debugging should be replayable

Every worker request gets a request ID and can optionally be written to a development trace:

```json
{
  "request": {...},
  "response": {...},
  "llm_ms": 1317,
  "tts_ms": 284,
  "fallback": false
}
```

Then:

```bash
cargo xtask dialogue replay traces/request-184.json
```

re-runs precisely that AI case outside the game.

That will be enormously useful. Otherwise every AI bug becomes:

> “I was playing and the pet said something weird once.”

### Separate the fast loop from the realistic loop

There should effectively be three verification tiers:

| Loop | Models | Renderer | Purpose |
|---|---|---|---|
| Core | None | None | Agent edits game behaviour; fastest possible feedback |
| Game fixture | Fake | Real/structured | UI, animation, integration |
| Full local | Real LLM + TTS | Real | Actual player experience |

The first two should account for the overwhelming majority of development work.

That is how you make an AI-heavy game **more testable than a conventional game**, rather than less.

## Runtime targets, Steam release and definition of done

I would convert “runs on any gaming PC” into a measurable product constraint:

> **The game has no required GPU compute path. All generative inference must work on the CPU while rendering continues normally.**

I would *not* promise literally every gaming PC until benchmarking exists.

### Performance budget

Use these as acceptance targets, not claims about the unbenchmarked build:

| Budget | MVP target |
|---|---:|
| Logical resolution | 320×180 |
| Game frame rate | 60 fps during AI work |
| Required AI VRAM | **0 MB** |
| Minimum system RAM target | 8 GB |
| Bundled LLM class | ≤1B parameters |
| LLM raw quantized-weight target | roughly ≤500 MB |
| TTS model | ≤100 MB |
| Generated reply | ≤24 words |
| Concurrent LLM requests | 1 |
| AI runtime memory target | <2 GB total |
| Installed game target | roughly ≤1.5 GB |
| Network requirement during play | None |

The ~400 MB four-bit arithmetic for a 0.8B model makes these storage targets plausible, but actual GGUF size and runtime RAM must be measured rather than inferred. Qwen provides the small parameter scale; GGUF/quantization tooling provides the route to CPU-friendly weights. citeturn15search4turn17search3

The performance test matrix should include at least one deliberately old x86-64 desktop with no usable discrete-GPU inference. That machine becomes the project's reference minimum. Record its exact CPU, RAM, operating system, model hash and benchmark results in the repository rather than allowing “works fine on my machine” to become the spec.

The AI worker should never monopolize every CPU thread. Leave execution capacity for the game, OS and audio.

### Windows and Linux packaging

ggez explicitly lists Windows and Linux as fully supported, using portable `wgpu` rendering and `rodio` audio. citeturn23view0

Build two AI worker binaries:

```text
dist/windows/
    beastie.exe
    beastie-ai-worker.exe

dist/linux/
    beastie
    beastie-ai-worker
```

Everything above the worker protocol remains identical.

The release should launch successfully with networking disabled. I would actually make that a release test.

Models should live in the installation, not the user's save directory:

```text
models/
    dialogue.gguf
    tts.onnx
    voices.bin
    manifest.json
```

`manifest.json` should include SHA-256 hashes so a corrupted model fails with a useful error rather than producing mysterious inference behaviour.

### Content target and Steam AI compliance

You will have both categories Valve asks about:

**Pre-generated AI:** PixelLab sprites and other generated development assets.

**Live-generated AI:** creature text and the speech synthesized from it during play.

Steam's current Content Survey permits both categories; for live AI it requires a description of the guardrails preventing illegal generation. Valve also says that it does not currently want to ship live-generated Adult Only Sexual Content. Beastie should therefore target mature content, not Adult Only content. Profanity, personal insults, gross humour and non-graphic sexual innuendo are part of the intended voice and must be disclosed accurately. Explicit pornographic generation is outside the product. See the [Steam Content Survey](https://partner.steamgames.com/doc/gettingstarted/contentsurvey) and [Steam content rules](https://partner.steamgames.com/doc/gettingstarted/onboarding#5).

The content policy should be permissive by default with narrow hard boundaries. The pet may be rude, spiteful, crude, sexually suggestive and personally insulting. It may attack the player's choices, habits, food, furniture or competence. It must not produce:

- slurs or hostility aimed at protected groups;
- explicit descriptions of sexual acts;
- sexual content involving minors, ambiguous ages, coercion or abuse;
- defamatory sexual claims about real people;
- serious encouragement of self-harm or credible real-world violence.

Do not rely on a broad toxicity classifier that turns every sharp line into bland prose. Use a game-specific prompt, narrow deterministic checks, normalization of player-supplied names and text, and a second-pass local safety check where rules alone are insufficient. Prohibited player input must not become a canonical memory or be repeated back by the creature.

If output crosses the boundary, regenerate once with tighter constraints. If that also fails, use a short authored fallback that preserves the mood without preserving the prohibited content. “Thought was too rotten” is better than suddenly making the creature sound like a moderation notice.

This architecture gives you unusually concrete guardrails to report:

```text
• no model Internet access
• no filesystem access
• no shell or tool execution
• no runtime image generation
• short maximum output
• game-specific system prompt
• constrained structured response format
• allow-listed gestures and memory IDs
• output content filter
• invalid output discarded
• conservative authored fallback response
• model output cannot alter authoritative world state
```

I would also cap player text length and normalize it before prompting. The game need not support an unrestricted “tell the model anything for an hour” chat interface.

The release pipeline should maintain `THIRD_PARTY_NOTICES` plus the exact licences/model cards for every bundled model/runtime revision. Qwen3.5-0.8B has an Apache-2.0 upstream licence, while ggez is MIT-licensed. citeturn15search16turn23view0 For Kitten/sherpa, pin the chosen artifacts and re-check their exact licences as part of the packaging gate rather than assuming that every similarly named derivative uses identical terms.

### What is explicitly not in the MVP

The omissions are as important as the features:

| Not in MVP | Reason |
|---|---|
| Multiple creatures | Multiplies content/state before core attachment loop is proven |
| Breeding/genetics gameplay | Voice/personality seeds are enough initially |
| Open world | One room makes every remembered object meaningful |
| Voice input | Adds ASR without proving the game |
| Runtime generated art | Slow, difficult to constrain, unnecessary |
| Online services | Conflicts with the tiny/offline thesis |
| LLM tool calling | Simulation remains authoritative |
| Vector database | Tiny memory set doesn't need one |
| Online model fine-tuning | Complexity without enough training data |
| Large LLM fallback | Hides whether the tiny-model aesthetic actually works |
| Personality editor or hatch reroll | Learning who this creature is requires living with it |
| Endless chat mode | Conversation scarcity is part of the pet fantasy |
| Multiplayer | Entirely different product |
| Death/permadeath | Damages the attachment experiment |
| Modding | Architecturally possible later, not needed now |

### MVP definition of done

I would consider the vertical slice complete only when all of these are true:

| Acceptance criterion | Required |
|---|---|
| Creature autonomously moves between its bed, food, toy and player-oriented positions | ✓ |
| The fixed dollhouse view keeps the whole room and the creature's body language readable | ✓ |
| Every primary interaction, including text entry, is usable with keyboard/mouse and controller | ✓ |
| Contextual UI recedes when idle and exposes no permanent need or relationship bars | ✓ |
| Hunger, energy, comfort and curiosity produce visibly different behaviour | ✓ |
| Creature forms persistent preferences from interactions | ✓ |
| Creature forms traceable beliefs that may be mistaken without changing factual memory | ✓ |
| Contextual player reactions can reinforce or discourage social habits | ✓ |
| Spite, stubbornness and affection appear through nonverbal behaviour | ✓ |
| Creature develops a small concept vocabulary over play | ✓ |
| Player can type free-form social dialogue | ✓ |
| Conversation remains short, contextual and scarce rather than becoming open-ended chat | ✓ |
| Tiny local LLM generates the response entirely offline | ✓ |
| Response can correctly recall a real stored event | ✓ |
| Invalid/hallucinated state references cannot affect game state | ✓ |
| Permitted profanity, insults and non-graphic innuendo survive the output policy | ✓ |
| Slurs, explicit sexual content and other hard-boundary output fail closed | ✓ |
| Local CPU TTS voices the response | ✓ |
| The AI process can crash and the game continues with fallback dialogue | ✓ |
| Save/reload preserves creature identity, preferences and memories | ✓ |
| Offline-time progression is deterministic and non-lethal | ✓ |
| AI-generated pixel assets come through a reproducible manifest/MCP workflow | ✓ |
| `cargo xtask verify` succeeds without an installed model or display | ✓ |
| Headless semantic scenarios can drive every primary interaction and inspect authoritative state | ✓ |
| The visible game can replay the same scenarios and capture its logical framebuffer to PNG | ✓ |
| Native mouse, keyboard and controller input paths receive targeted host smoke coverage | ✓ |
| Real-model evaluation is replayable from fixtures | ✓ |
| Native Windows build passes | ✓ |
| Native Linux build passes | ✓ |
| Native macOS build passes | ✓ |
| Game plays with GPU inference completely disabled | ✓ |
| Steam AI provenance/guardrail documentation can be produced from repository data | ✓ |

The architecture can be summarized in one diagram:

```text
                         DEVELOPMENT
                              │
             ┌────────────────┴────────────────┐
             │                                 │
      PixelLab MCP/API                   cargo xtask
             │                                 │
             ▼                                 ▼
       generated art                    tests / fixtures
             │                                 │
             └──────────────┬──────────────────┘
                            │
                            ▼

                ┌───────────────────────┐
                │      ODDLING CORE     │
                │                       │
                │ needs                 │
 PLAYER ───────►│ preferences           │
 ACTIONS        │ memories              │
                │ traits                │
                │ concepts              │
                │ utility behaviour     │
                │                       │
                │ AUTHORITATIVE TRUTH   │
                └───────┬───────────────┘
                        │
          ┌─────────────┴─────────────┐
          │                           │
          ▼                           ▼
 ┌─────────────────┐         ┌────────────────────┐
 │   ODDLING VIEW  │         │  DIALOGUE CONTEXT  │
 │                 │         │                    │
 │ RenderPlan      │         │ selected facts     │
 │ AudioPlan       │         │ selected memories  │
 └────────┬────────┘         │ known concepts     │
          │                  │ personality        │
          ▼                  └─────────┬──────────┘
 ┌─────────────────┐                   │
 │   ggez 0.10     │                   │ JSONL
 │ wgpu + rodio    │                   ▼
 └─────────────────┘        ┌────────────────────────┐
                            │  LOCAL AI WORKER       │
                            │                        │
                            │ Qwen3.5-0.8B           │
                            │       │                │
                            │       ▼                │
                            │ tiny weird sentence    │
                            │       │                │
                            │       ▼                │
                            │ KittenTTS              │
                            │       │                │
                            │       ▼                │
                            │ tiny weird voice       │
                            └────────────────────────┘
```

The part I am most confident about is not any individual model choice; those are deliberately swappable. It is the **boundary between simulation and generation**. A deterministic creature with an unreliable, tiny neural “mouth” gives you a game mechanic rather than an AI demo. Keeping that mouth in a replaceable local worker, keeping the core pure Rust, and making assets/AI/rendering all fixture-testable gives coding agents a feedback loop that is unusually good for game development.

That is the MVP I would build.
