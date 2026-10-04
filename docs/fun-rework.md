# Fun rework

Status: proposed on 2026-10-04. This is the normative contract for the next major pass. It
supersedes lower-level rules in earlier feel contracts where they conflict (listed below).

## Why this pass exists

Beastie has had more than twenty feel passes, and the creature looks and moves well. The game is
still not fun to play. The owner's playtest verdict, in their words:

1. The creature doesn't respond to anything you do, and when it does, the response is very late.
2. Chat produces generic lines, and there is no understandable way to teach the creature. This
   phase is boring and gives no reason to keep playing.
3. It is unclear what the creature is doing or wants. It mostly floats.
4. It is not engaging, especially at the start.
5. Some interactions take two clicks for no reason.
6. Above all, it is not fun.

Earlier passes missed this for a structural reason: their evidence was scripted runs judged
through filmstrips and traces, and several reviews state that full-speed subjective review was
not performed. Scripts sent cursor positions rarely, skipped discovery, and never asked "would I
keep playing?" This pass is judged by play, latency measurements and a fun scorecard, not only by
causal correctness.

## Diagnosis of the current code

These causes were verified at `02ad95a`. Line numbers will drift; the mechanisms are the point.

### Responsiveness

- **Moving the mouse cancels play and comfort.** `app.rs` sends `SessionCommand::Cursor` whenever
  the pointer's world position changes. `should_interrupt_for_player_event` in
  `beastie-core/src/simulation.rs` treats `PlayerEvent::Cursor` as interrupting, so moving the
  mouse after choosing Play or Comfort aborts the approach. This alone explains much of "it doesn't
  respond" and why players click again.
- **The simulation runs at 1 Hz.** `SIMULATION_TICK_MS = 1_000`, and phases advance in whole
  ticks. Feeding runs Notice → Brake → Gaze → Turn → Approach → Inspect → Act → Recover at about
  1 s per phase, so the creature spends about 4 s before it starts swimming. Click-to-eating is
  roughly 10–18 s.
- **Accepted play has no acknowledgement cue.** The first sign is a gaze change and a small text
  label in the bottom bar.
- **Talk has a 30 s cooldown** (`TALK_COOLDOWN_MS`), and during it talk is silently `TalkIgnored`.
- An earlier contract (`creature-presence-and-care.md`) says "Do not shorten valid travel merely to
  make every command resolve quickly." This pass revises that rule (see Product rules).

### Teaching and conversation

- **Learning runs on a timer.** `update_development` adds `Again` at day ≥ 2 and `Yesterday` at
  day ≥ 3 (a day is 15 minutes). Nothing the player says teaches anything. `Toy`, `Trust`, `Give`,
  `Friend` and `Why` exist as concepts but nothing ever adds them. `NameRecognized` is never set.
- **The creature can't hear most of what is said.** `language.rs` is a fixed keyword table that
  only counts words whose concept is already known. Everything else increments `unknown_words` and
  is dropped. `player_said` is overwritten with the understood fragment before the model sees it.
- **Replies are designed to be generic.** Hatch-stage replies are capped at 3 words, the fallback
  directive is "greeting" or "don't know", and sampling is temperature 0 / top_k 1. Unpackaged dev
  runs (and `--fake-ai` without `BEASTIE_AI_WORKER`) get no worker at all, so every reply is an
  authored fallback line such as "hm. words stuck."
- `ConceptLearned` and `LanguageAdvanced` events exist, but nothing in the view reacts to them.
  The player never sees learning happen.

### Intent

- Most idle time is a 16–32 s settle window (`IDLE_BOUT_MIN_MS`/`MAX_MS`) of random ±40 drift.
  That is the "it just floats" reading.
- Intent is shown only as a 65×15 text label ("hovering", "swimming over") in the bottom bar.
  There are no pre-commitment tells, wants, or target indicators.

### Clicks

- Clicking the creature or a toy opens a context menu; the action is a second click. Feeding takes
  three clicks: Feed → food → drop point.
- While a menu is open, world clicks do nothing and there is no click-outside dismissal, so the
  player closes the menu and clicks again.

### Opening

- A fixed seed and an immediate creature with no event. No first-meeting moment, no early hook,
  no reason to act. The first unprompted speech arrives when hunger crosses 0.82, about 23 minutes
  in. The message field has default focus, which invites typing before the player has any idea
  what Mop understands.

## Outcome

Within ten minutes of a fresh save, a new player should have:

- seen Mop react the instant they did anything, every time;
- understood what Mop is doing and what it wants without reading a label;
- taught Mop at least one word through play and heard it used back;
- been surprised by Mop at least once in a way that felt specific to this creature;
- wanted to keep playing to teach the next thing.

The target reaction stays the philosophy's: "of course *my* Beastie would do that," and
"this particular little idiot knows me."

## Product rules

These hold for the whole pass. Rules from earlier feel contracts that conflict are superseded.

- **Acknowledge instantly, resolve quickly.** Every player input produces a visible creature
  reaction (head turn, ear or fin flick, eye change, body twitch, or sound) within 100 ms. Physical
  payoff for a nearby target lands within about 1.5 s; travel across the tank within about 3 s.
  Creature agency is expressed through *how* it responds (eager, reluctant, refusing, distracted),
  never through dead air. Refusal is a visible act, not an absence.
- **Pointer motion is attention, not a command.** Hover can draw gaze and curiosity. It never
  cancels an accepted action.
- **Core verbs are one gesture.** Pet: click or stroke Mop. Play: click a toy to toss or nudge it.
  Feed: drag food from the feeder into the water (or a single click that drops near Mop). Talk:
  type or hold to speak, always available. Menus are only for rare verbs (rename, inspect). An open
  menu never swallows a world click; clicking elsewhere both dismisses it and performs the click.
- **Intent is visible before action.** Mop shows what it wants and what it is about to do through
  gaze, orientation, anticipation poses and wordless want-bubbles (pictograms of food, a toy, the
  player, sleep). An observer should be able to say what Mop is doing and why from the picture
  alone. Aimless drift is replaced by short purposeful activities with clear beginnings and ends.
- **Teaching is the core loop.** The player teaches Mop words by using them while something
  relevant is shared: holding up or tossing the ball while saying "ball", praising after an action,
  naming Mop and calling it. Repetition in context builds an association in simulation state;
  enough evidence makes Mop learn it. The player may coin their own words ("zorp" for the ball),
  and Mop learns those just as well. Learned words change what Mop understands, what it can be
  asked for, and how it talks.
- **Learning is legible.** Hearing an unknown word in a clear context makes Mop visibly curious
  (head tilt, an attempted echo like "baw?"). Learning a word is a celebrated moment with a distinct
  animation and sound. Mop's known words are visible somewhere diegetic and lightweight (a small
  shell of collected words, a page Mop keeps), never a stat bar or percentage.
- **Being heard is never scarce; speaking is.** Mop always reacts nonverbally when addressed
  (unless asleep, and even then it can stir). Verbal replies stay short and occasional. Replace the
  silent 30 s talk cooldown with visible attention or fatigue behavior.
- **Words are grounded.** Every line Mop says refers to something real: the current activity, the
  last thing the player did, a learned word, a memory, a want. Generic greetings and "don't know"
  filler are failures. The no-model path (authored grammar over known words and current context)
  must be good on its own, because it is what plays when inference is absent.
- **Wants create goals without quests.** Mop's desires (to play with a specific toy, to be fed a
  favorite, to learn the name of the thing it keeps staring at) are the player's goals. No checklist,
  quest log or meter.
- **The opening is an event.** A fresh save begins with a short, authored first meeting in which
  Mop notices the player, reacts to the first touch, and invites the first lesson. It should teach
  the loop by playing it, without a tutorial modal. Vary the seed per new save.
- **Short, medium and long loops all pay off.** Seconds: touch, toss, feed feel good every time.
  Minutes: a word is taught, a preference is discovered, a want is satisfied. Sessions and days:
  vocabulary grows, Mop uses your words in new combinations, habits and the relationship become
  specific, and returning is greeted.

### Philosophy check

These rules fit [the philosophy](game-design-philosophy.md): learning stays explicit simulation
state backed by evidence; the model only phrases expression; the game plays fully without
inference; personality is embodied. Two points need explicit amendment if implemented as written,
and the pass should make those edits in the philosophy rather than let code redefine it:

- "Conversation stays scarce" is retained for Mop's speech but clarified so the player's speech is
  always perceived and visibly received.
- "No bars and meters" is retained; a diegetic collection of learned words is a record of history,
  not a meter. State this distinction in the philosophy.

## Authority to rework

Rewrites of core parts are in scope when they produce the better game. Specifically allowed:

- Raise the simulation tick rate or move to variable-step/event-driven stepping, keeping
  determinism under an injected clock and seed.
- Replace the action phase pipeline, idle scheduler and behavior selection with a design that makes
  intent readable and responses fast (for example, a utility system with explicit wants and
  telegraphed commitments).
- Replace `language.rs` keyword interpretation and `update_development` time gates with an
  evidence-based word-learning model (word ↔ concept/object/action associations with context,
  repetition and reinforcement), including player-coined labels.
- Rebuild the interaction model (direct manipulation, drag, toss) and the bottom rail.
- Rework the AI prompt, context selection, reply limits, sampling and the authored no-model
  grammar. Use `$hillclimb` for this layer (see Evaluation).
- Break save compatibility. The game is unreleased; bump the save version, and either migrate or
  start fresh, recording the decision.

Keep the renderer, art direction, creature identity, offline/local-only runtime, simulation
authority over model output, graceful degradation and cross-platform headless gate. This is not a
renderer pass. Do not build multiple creatures, breeding, economy, crafting or storefront work.

## Evaluation

Correctness tests are necessary and not sufficient. The pass is judged by four kinds of evidence.

### 1. Latency budgets, measured

Add instrumentation (trace events with presentation timestamps) and automated checks for:

| Measure | Budget |
|---|---|
| Input to first visible creature reaction | ≤ 100 ms |
| Nearby physical payoff (pet, toss within reach, food near Mop) | ≤ 1.5 s |
| Cross-tank payoff | ≤ 3 s |
| Accepted action cancelled by pointer motion | never |
| Clicks needed for pet / play / feed | 1 / 1 / 1 gesture |
| Dead world clicks while a menu is open | none |

Include a feel scenario that moves the pointer continuously, as a human does.

### 2. Readability, judged blind

Sample frames across ten minutes of quiet life and play. A reviewer subagent that sees only the
frames (no labels, traces or bottom-rail text) names what Mop is doing and what it wants. Target:
correct for at least 9 of 10 samples.

### 3. Dialogue quality, hillclimbed

Use `$hillclimb` to build an eval over fixed simulation situations (fresh hatch, mid-lesson,
just-learned word, refused food, returning player, coined word, unknown word). Grade each line for
groundedness in the provided state, use of learned words, creature voice and absence of generic
filler. Hillclimb the prompt, context selection, sampling and the authored no-model grammar
separately: the no-model path must also pass. Run the real local model where available; record the
model, settings and scores in `evals/`.

### 4. The fun scorecard, from real play

Play the native debug build with live input, through the normal pointer, not only scripted
semantic commands. For each run of the first ten minutes, record:

- time to first creature reaction after first input;
- time to first word learned and first time Mop uses it;
- number of distinct moments where Mop did something specific and surprising;
- any moment of confusion, dead input, double click, waiting, or "what is it doing?";
- whether there was a clear next thing to want to do at every point.

Targets for a fresh save: first reaction under 1 s, first word learned within 3 minutes, Mop uses
it within 5 minutes, at least three surprising moments, zero dead inputs or double clicks, and no
stretch over 20 s without either something to watch or something to do. Record runs in a
`docs/feel-review-fun-*.md` review with timestamps and evidence paths.

## Implementation checkpoints

Checkpoints, not optional scope. Order may change if evidence suggests it.

1. **Responsiveness floor.** Fix the pointer-cancel bug; add the instant-acknowledgement layer;
   raise tick rate or decouple reaction from ticks; shorten phase preambles; one-gesture verbs;
   click-through menu dismissal; latency instrumentation and tests.
2. **Readable intent.** Wants, telegraphs and want-bubbles; replace long settle drift with short
   purposeful activities; blind readability check.
3. **Teaching loop.** Evidence-based word learning with context, coined words, curiosity and
   echo on unknown words, learned-word celebration, diegetic word collection, requests Mop can
   understand and choose to honor.
4. **Grounded voice.** Rebuilt context and prompt, authored no-model grammar, hillclimbed dialogue
   eval for both paths.
5. **Opening and loops.** First meeting, seed variety, early wants that lead into the first
   lesson, session return greeting, medium- and long-term development that uses learned words.
6. **Fun iteration.** Repeated live play against the scorecard, fixing whatever is least fun each
   round, until two consecutive full ten-minute runs meet every target and reveal nothing material
   left to fix.

## Definition of done

- Every rule above holds in the native game, with the latency table, blind readability check,
  hillclimb dialogue results and fun scorecard recorded.
- Philosophy amendments are made explicitly where the rules required them.
- Superseded rules in earlier feel contracts are marked as superseded with a link here.
- Headless regression tests cover learning, latency-critical paths, pointer non-cancellation and
  save versioning. `cargo xtask verify` passes; `cargo xtask dev --fake-ai` was exercised natively.
- A short owner playtest brief exists: what to try in the first ten minutes and what should happen.
  The owner's verdict after playing is the final judge; their feedback reopens the pass.
- This document records the final design rather than the proposal, and `docs/STATE.md` describes
  the result honestly.

## Implementation record

Living notes. Decisions here supersede the proposal text above where they differ.

### Responsiveness (done, first pass)

- Simulation ticks at 100 ms. Velocities are fixed-point units per second; `tick_step` converts
  them per tick without rounding drift. Swimming accelerates (16,000 u/s²) to 3,400 u/s with an
  eased arrival that snaps onto targets.
- Pointer motion never interrupts anything and never starts travel. It only draws an idle
  creature's gaze when near, and a resentful creature may flee.
- Food phases total about half a second before the swim; feeding pays off ~1.5 s after a click.
- Accepted toy play perks up at once (`PositiveNotice`). A direct offer is a play *session*: the
  ball is chased for three more rounds, the bell struck twice more, the sock towed for 2.6 s.
  History and reward are recorded on the first contact only.
- Pet = click Mop. Play = click a toy (in the tank or rail). Feed = click a food on the rail; it
  drops just above Mop's head. Clicking empty water taps the glass: an idle Mop looks, a curious
  one swims over, with a ripple and a bubble sound. World targets stay live under shallow menus,
  and empty water dismisses them. The name in the rail opens Inspect and Rename.
- A reply never takes keyboard focus away from the message field (it used to move focus to a
  reaction chip, so typing after Mop spoke silently did nothing).

### Teaching (done, first pass)

- `lexicon.rs`: words gather evidence from salient meanings (`teaching.rs` focus marks from
  events plus live state). A word needs two hearings, at least 4 evidence, and a 1.5x lead.
  Mutual exclusivity discounts meanings that already have a word. Player-coined words work like
  any other. Language stage follows vocabulary size (3 words: Words, 8: Phrases).
- Every utterance is heard immediately. Unknown words get a curious head tilt, a "?" and a soft
  echo attempt ("ball" -> "baw?"). Learning plays a sparkle burst and a dedicated chime
  (`creature/learned`, `tools/audio/synth_learned.py`). Known words are requests: toys, food,
  come, play, sleep, praise and scolding. Disliked food and stubbornness produce visible refusals.
- A sleeping creature does not hear words.

### Voice (done, first pass)

- Requests carry `vocabulary` and a `speech_intent`; the creature can only say learned words,
  creature sounds and stage glue. `compose_line` is the complete no-model voice. The worker asks a
  local model for the line only and holds it to the same words; see
  `evals/dialogue/speech/log.md` (31/32 train, 15/15 validation on Qwen3.5 0.8B).
- The "Local AI unavailable" banner no longer appears for the designed no-model voice.
- Speech is a compact bubble at Mop's head. Laugh/Disapprove/Comfort chips are gone; praise and
  scolding are taught as words.

### Intent and opening (done, first pass)

- `wants.rs`: hunger, a favorite toy, company, sleep, or wanting the name of the thing it is
  playing with. Shown as a thought bubble with a voxel pictogram (with "?" for a name). Mop asks
  out loud for wants every 20 s when it has the word, 45 s otherwise, never while busy.
- Idle bouts are 3-8 s; private activities are shorter.
- New saves get a wall-clock seed, a ranked favorite/fine/disliked toy and food, and a first
  meeting: Mop waits at the cave mouth and comes to the glass after 2.5 s or the first input.
- State-driven coaching hints guide the first lessons and vanish once done.

### Open work

- Remove the legacy intent-free dialogue path (relationship-lane prompts, `authored_fallback_phrase`,
  legacy corpus cases) now that every request carries a speech intent.
- Native feel capture with live pointer input, blind readability check, and the ten-minute fun
  scorecard runs.
