# State

Last updated: **2026-10-04**.

## Now

- **The fun rework is implemented** ([contract](fun-rework.md), [review and
  evidence](feel-review-fun-20261004.md), [owner playtest brief](playtest-brief-fun.md)). Mop
  reacts in the same frame to every input; pet, play and feed are one click each and pay off in
  under about 1.7 s; pointer motion never cancels anything; empty water taps the glass.
- Teaching is the core loop: words learn from what the player points at and what Mop is doing
  (lexicon with evidence, mutual exclusivity, coined words, curious echoes, a learned-word
  celebration). Learned words become requests Mop may honor or refuse, and Mop speaks only in the
  player's words plus creature sounds, through a deterministic composer or the local model held to
  the same vocabulary (speech hillclimb 31/32 train, 15/15 validation).
- Intent is readable: wants as pictogram thought bubbles, rings on the thing Mop is heading for,
  crossed-out refusals, play sparkles, a bubble chase instead of drifting. A shy first meeting,
  ranked per-creature preferences and state-driven coaching hints open each new save.
- The final consecutive live-input ten-minute playthroughs (composer voice and local model) learn
  the first word in about 17 s, pay off every input, and show no errors, banners or 20-second
  lulls. Every click in the tank gets a creature reaction. A fresh model reviewer read Mop's
  activity and want correctly in 9 of 10 random play frames in the final round. A human
  playtest has not happened yet.
- One dialogue path remains (learned-word speech). The session undoes any command that would
  produce invalid state; a randomized test drives 48 creatures through mixed play.
- The game remains offline and deterministic; the simulation owns facts and the model only phrases
  them. The renderer and art direction are unchanged from the [UI redesign](ui-redesign-20261001.md)
  and [renderer energy](renderer-energy.md) work.

## Next

The owner plays ten minutes using the [playtest brief](playtest-brief-fun.md); their verdict
decides whether the fun rework reopens. Without new feedback, the strongest next candidates are
richer word grammar as vocabulary grows (two-word requests such as "ball come"), and verifying
microphone input with the learned-word loop.

## Candidates Not Chosen

- **Cross-device renderer measurement:** deferred behind the fun rework. Reproduction commands
  and raw evidence remain in [the evidence guide](performance/20260909-energy/README.md).
- **Spatial lighting reuse:** workgroup sharing, coarse producer/resolve passes and multiple pixels
  per invocation regressed against paired controls.
- **Single-map soft shadows:** faster than its control but visibly weakened contact and broad shadows.
- **Generic traversal or index reordering:** previous grids/compressed trees and vertex-cache ordering
  showed no material repeatable benefit.
- **Lower visible cadence or primary resolution:** current presentation quality is retained;
  hidden-window pacing provides savings without reducing visible animation.

## Learned Recently

- Live-input playtest scripts and the opt-in `BEASTIE_EVENT_LOG` trace: [tools/playtest](../tools/playtest/README.md).
- Learned-word speech eval and hillclimb log: [evals/dialogue/speech](../evals/dialogue/speech/README.md).
- UI targets, reconciled mockup details and native examples: [target index](style-reference/ui-20261001/README.md).
- Complete findings, accepted fixes, repeated native comparisons and honest limits: [UI/feel review](feel-review-ui-20261001.md).
- Shared text metrics, modal/input ownership and physical presentation boundaries: [architecture](architecture.md).
- Toy-specific cue ownership and quiet cave behavior: [audio direction](audio-direction.md).
- Bounded capture deadlines, binary provenance and supplemental fixtures: [feel review loop](feel-review-loop.md).
- Product authority and retained visual identity: [game-design philosophy](game-design-philosophy.md),
  [art bible](art-bible.md), [renderer energy](renderer-energy.md).
