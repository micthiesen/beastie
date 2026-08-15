# Architecture

The architecture turns the MVP's central rule into dependency boundaries: AI creates expression,
while the simulation creates truth.

```text
player input --> beastie-core --> game events --> beastie-view --> beastie-game
                    |
                    +--> curated facts --> beastie-protocol --> beastie-ai-worker
```

Development control enters through a shared session boundary rather than bypassing the game rules:

```text
JSONL scenario --> headless adapter --+
                                    +--> GameSession --> beastie-core / worker / view
JSONL scenario --> visible adapter --+                         |
                                                              +--> trace / PNG capture
real devices -------------------------------> beastie-game input mapping
```

`beastie-core` is deterministic and sans-I/O. It accepts elapsed game time, player events, and an
injected random source. It owns needs, traits, preferences, memories, intentions, development,
and save data. It cannot import the renderer or AI worker.

`beastie-protocol` is the narrow trust boundary. Requests contain curated observations, concepts,
and candidate memories. Replies are bounded text plus allow-listed presentation metadata. Reply
memory references are validated against the request and never mutate simulation state.

`beastie-view` converts state into serializable render and audio plans. The ggez shell executes
those plans and owns platform input/window/audio integration. This keeps most visual behavior
testable without a GPU or display.

The AI worker is a child process using versioned JSONL over standard input/output. Heavy model and
TTS runtimes stay out of the game executable. A fixture backend remains permanent for development,
tests, and graceful fallback.

`GameSession` is the intended reusable orchestration layer above the pure core and below ggez. Grow
the headless and visible adapters incrementally so they accept the same versioned semantic commands
and produce the same observations. See [development-harness.md](development-harness.md). Native
window capture and pointer control are already proven for cheap host checks; scenario files and
direct logical-framebuffer capture should arrive with the gameplay slices that need them rather
than as a large prerequisite.

The current headless adapter is `cargo xtask play --fake-ai`. It reads bounded JSONL commands from
standard input or a scenario file and emits one structured observation or error per line. The
checked-in berry-grudge scenario is replayed by the display-free verification gate. The visible
adapter will consume this same `beastie-session` boundary as the room shell gains real interactions.

Platform-specific packaging may vary, but the core, protocol, save format, assets, and worker
contract remain identical across macOS, Windows, and Linux.
