# Fun feel review, 2026-10-04

Review of the [fun rework](fun-rework.md) on the native debug game (macOS, Metal, M5 Max). The
owner's verdict was that Beastie did not feel good to play: Mop ignored input or answered late,
chat was generic and teaching was impossible, Mop's intent was unreadable, the opening had no hook,
some actions needed two clicks, and above all it was not fun. This record covers what changed,
how it was measured, and what remains for a human to judge. The owner's own playtest
([brief](playtest-brief-fun.md)) is the final judge.

## Before and after

![Before: generic menus, no wants, "hm. rude giant."](evidence/fun-20261004/baseline-before.png)

At the start of the pass (`02ad95a`): clicking Mop opened a half-screen menu, Play opened a second
one, and feeding took three clicks. Moving the mouse cancelled an accepted play order, so players
clicked again. Feeding took 10–18 s. Typing "hi mop, this is your ball" produced "hm. rude giant."
with a "Local AI unavailable" banner; nothing the player said could be learned. Most idle time was
a 16–32 s drift labeled "hovering".

![First word: "ball!" with the next hint](evidence/fun-20261004/first-word-learned.png)
![Intent: a ring on the ball Mop is going for](evidence/fun-20261004/intent-ring.png)
![A refused spoken request in Mop's own words](evidence/fun-20261004/spoken-refusal-word.png)

## Measurements

### Latency (live OS input, scripted with `cliclick`, event trace timestamps)

| Measure | Budget | Result (final runs S, T) |
|---|---|---|
| Input to visible reaction | ≤ 100 ms | Same frame: every input type emits a presentation cue in the command that handles it (`every_player_input_is_acknowledged_in_the_same_frame`). Live: 0.06 s from a teleport click; 0.31–0.33 s with the final driver, which moves the pointer, waits 40 ms and then clicks after logging the action |
| Pet | ≤ 1.5 s | 0.31–0.36 s (including about 0.3 s of driver time) |
| Nearby toy | ≤ 1.5 s | 0.31–0.66 s |
| Hand-fed food | ≈ 1.5 s | 1.74–1.98 s (about 1.45–1.7 s of game time) |
| Cross-tank toy | ≤ 3 s | 1.58–1.95 s |
| Pointer motion cancelling an action | never | never (`pointer_motion_never_interrupts_an_accepted_toy_interaction`) |
| Clicks for pet / play / feed | 1 / 1 / 1 | 1 / 1 / 1 |
| Dead world clicks | none | none: ten clicks on the cave, plants, sand, side walls and corners each produced a tap or an offer. The cave, plants and food tap the glass there; Mop wins clicks that overlap scenery |

### Blind readability

A fresh model reviewer saw only the tank (rail, hints and labels cropped) and named Mop's
activity and want per frame; answers were scored against the rail's own labels and the traces.

| Round | Frames | Change before it | Correct |
|---|---|---|---:|
| 1 | staged | first pass | ~5/14 |
| 2 | staged | refusal bubble, play sparkles | ~6/12 |
| 3 | staged | intent rings, bigger bubbles, fixed brows, "!" glyph | 10/12 |
| 4 | staged | floor ring for foraging, brow sign fix | 11/12 |
| 5 | run C | none (first sample of real play) | 7/10 |
| 6 | run E | spoken refusals crossed out, glossy chased bubble | ~4/10 |
| 7 | run G | bubble close to Mop, no ring around it | ~5/10 |
| 8 | run K | 0.6 s snap, bubble kept clear of toys | 8/10 |
| 9 | run M | sand spray for foraging, star sparkles | ~6/10 |
| 10 | run O | Mop looks at the bubble it chases | ~7/10 |
| 11 | run Q | Mop stops short so the bubble stays at its mouth | ~8.5/10 |
| 12 | run S | a resting "z" when settling in the cave | 9/10 |

Rounds 5 to 12 use the same ten frame positions from a fresh ten-minute run, so they sample
ordinary play rather than staged moments. Scores vary with what the frames happen to catch.
Rounds 6 and 7 landed on several bubble chases, which then read as an empty ring, a toy or
nothing, and they drove most of the fixes. In round 12 the reviewer named every goal, toy, plant
and cave rest, and read the idle frames as idle. The one doubtful frame was labeled
"finishing up" and read as a pet prompt. Speech in Mop's learned words, dotted target rings,
crossed-out refusals and the cave "z" read with high confidence. The weakest remaining cue is a
lone "!", which says Mop noticed something but not what.

### Dialogue

The local model (Qwen3.5 0.8B Q4) and the composer were hillclimbed on 47 frozen speech cases
([log](../evals/dialogue/speech/log.md)): from 0/32 (every learned-word line fell back) to 31/32
train and 15/15 validation. In live runs the model answered in 40–200 ms, always in Mop's learned
words or creature sounds, with no failure banner. Lines from run D: "ball! mrp!", "mop, know mop,
mop know!", "no ding.", "yes! sock!", "pellet! mrp!".

### Ten-minute fresh-save playthroughs

Each run is the same scripted newcomer (`tools/playtest/tenmin.sh`): meet,
tap, play and name the ball, ask for it, feed and name a berry, pet and name Mop, sock, coin "ding"
for the bell, feed a mushroom, praise, scold, call, feed a pellet, and four idle stretches of
25–60 s. Real OS pointer and keyboard input; screenshots every 1.8 s; game event trace.

| Run | Voice | First word | Mop uses it | Words | Lines (distinct) | Quiet > 20 s | Errors | Notes |
|---|---|---|---|---:|---:|---|---|---|
| A | composer | 15.3 s | 15.3 s | 7 | 32 (30) | none | 0 | |
| B | local model | 15.3 s | 15.4 s | 7 | 31 (29) | none | 0 | a wedged ball dropped an offer (fixed) |
| C | composer | 15.4 s | 15.4 s | 7 | 33 (31) | none | 0 | |
| D | local model | 15.3 s | 15.4 s | 7 | 32 (28) | none | 0 | |
| E–R | both | 15.2–16.8 s | same moment | 6–7 | 31–34 | none | 0 | dead clicks on scenery and edges found and fixed; driver misses (stale pointer, bell mistaken for Mop, Mop swimming away) fixed or noted |
| **S** | composer | 16.7 s | 16.7 s | 7 | 34 (30) | none | 0 | every input paid off |
| **T** | local model | 16.6 s | 16.7 s | 7 | 31 (28) | none | 0 | every input paid off |

S and T are the final consecutive pair, run on `8eb7cb5`. Times from S onward include about 0.3 s
of driver latency per action. Every pair after the first was rerun after any behavior change, so
the final pair reflects the shipped code.

Surprising moments recurred in every run: Mop refused a request in its own words ("no ball."),
learned the player's coined "ding" for the bell, named toys unprompted while playing ("sock~
sock~", "mop... sock."), said "mop know sock!", asked for food by name when hungry, and spat out a
disliked food. A clear next thing to want was always on screen: a "?" bubble on whatever Mop played
with, a coaching hint until the first lesson and request were done, Mop's own wants afterward.

Fixes found by these runs: dead clicks on the cave, plants, sand and side walls; a bubble chase
that read as nothing; foraging and play cues mistaken for ambient bubbles; pointer-cancel
(pre-run), keyboard focus stolen after a reply,
Enter blocked while a reply was pending, duplicate suppression making repeated words show the
failure banner, the model once contradicting a complied request ("no sock!"), wedged toys making
an offer silently fail, a refused ball not learnable while Mop played with another toy, spat food
lingering, and an ownership crash when a spoken request overlapped a relationship moment. That
crash also led to the session undoing any command that would leave invalid state, so a future bug
costs one input instead of the game.

### Synchronized feel capture

`cargo xtask feel --suite first-five-minutes` was rewritten for the new loop and captured at
`target/feel/fun-first-five-2/` (commit `cb51765`, clean tree, debug build): tap, play, "ball"
learned after two lessons, the request complied, berry learned and the disliked berry spat out, Mop
named. Filmstrips showed the wedged-ball give-up later fixed.

## Not verified here

- A human playing it. Every run above used scripted input; whether it is *fun* is the owner's call.
- Full-speed human viewing of motion; reviews used stills, filmstrips and traces.
- Microphone and speech-to-text input (the speech models were absent); typed input only.
- Windows and Linux; non-M-series GPUs.
- The blind reviewer is a model with no prior knowledge of the game, not a person.
