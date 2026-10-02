# State

Last updated: **2026-10-02**.

## Now

- The complete UI and feel redesign is implemented. Twenty-one ImageGen targets cover every
  existing page and shared interaction family; rounded teal/brass surfaces, readable Normal/Large
  typography, compact captions, Unicode editing and clear local feedback form one interface.
  See the [design contract](ui-redesign-20261001.md), [review and evidence](feel-review-ui-20261001.md)
  and [target/native capture index](style-reference/ui-20261001/README.md).
- Toy invitations now produce their actual ball, bell and sock responses exactly once at contact.
  Sock pickup/release, body turns at tank boundaries and bottom foraging retain clear, continuous
  silhouettes. Speech follows actual playback ownership; temporary affection cannot strand newer
  private journeys. Regression tests preserve saved continuation, offline return and deferred talk.
- The game remains offline and deterministic. Simulation owns facts, the model supplies expression,
  and care, quiet life and relationship behavior remain usable without inference or speech.
- Final native and headless verification is recorded in the [review](feel-review-ui-20261001.md).
  Native evidence is M2 Pro/Metal; physical microphone, gamepad and speaker audition, pinned-toolchain
  verification and other platforms remain outside this pass. Fixed-rate capture is not a host
  performance measurement.
- The existing renderer retains raster visibility, shared compute lighting, exact static caches,
  adaptive shadow maps, indexed shadow-only LOD and background geometry preparation. Earlier paired
  M5 Max measurements reduced GPU throughput time about 32%, ordinary task CPU energy about 33%
  and hidden task CPU energy about 85%. These are separate measurements, not device battery claims;
  see [renderer energy](renderer-energy.md).
- `dev-perf` keeps application assertions and overflow checks. Routine work uses the headless gate
  and native development path; release installers and model bundles remain explicit checkpoints.

## Next

Measure the renderer on Windows, Linux and an available lower-end GPU. Use the capture-free
world/UI/churn/sustained scenarios and gold-reference motion suite. Record actual viewport/backend,
selected map density, frame tails, memory and available energy accounting. Cross-device evidence
is needed before choosing further adapter-specific policies. Reproduction commands and raw
performance evidence are in [the evidence guide](performance/20260909-energy/README.md).

## Candidates Not Chosen

- **Spatial lighting reuse:** workgroup sharing, coarse producer/resolve passes and multiple pixels
  per invocation regressed against paired controls.
- **Single-map soft shadows:** faster than its control but visibly weakened contact and broad shadows.
- **Generic traversal or index reordering:** previous grids/compressed trees and vertex-cache ordering
  showed no material repeatable benefit.
- **Lower visible cadence or primary resolution:** current presentation quality is retained;
  hidden-window pacing provides savings without reducing visible animation.

## Learned Recently

- UI targets, reconciled mockup details and native examples: [target index](style-reference/ui-20261001/README.md).
- Complete findings, accepted fixes, repeated native comparisons and honest limits: [UI/feel review](feel-review-ui-20261001.md).
- Shared text metrics, modal/input ownership and physical presentation boundaries: [architecture](architecture.md).
- Toy-specific cue ownership and quiet cave behavior: [audio direction](audio-direction.md).
- Bounded capture deadlines, binary provenance and supplemental fixtures: [feel review loop](feel-review-loop.md).
- Product authority and retained visual identity: [game-design philosophy](game-design-philosophy.md),
  [art bible](art-bible.md), [renderer energy](renderer-energy.md).
