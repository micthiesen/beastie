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

Runtime sprite IDs resolve through the checked asset manifest, preferring `assets/final` over
`assets/generated`. The shell decodes optional PNGs once at startup and retains geometry as a
graceful fallback. Authored one-shot WAVs use the same final-over-generated resolution; simulation
events and UI actions select sounds, while a missing output device, decode failure, or playback
failure never blocks play. See
[art-bible.md](art-bible.md) and [audio-direction.md](audio-direction.md).

The AI worker is a child process using versioned JSONL over standard input/output. Heavy model and
TTS runtimes stay out of the game executable. A fixture backend remains permanent for development,
tests, and graceful fallback.

`GameSession` is the reusable orchestration layer above the pure core and below ggez. The headless
and visible adapters accept the same versioned semantic commands. See
[development-harness.md](development-harness.md). The visible adapter executes declarative plans at
a fixed 320×180 resolution, replays deterministic scenario files, and captures the logical
framebuffer directly. Durable wall-clock saves remain in the game shell so headless simulation stays
portable and deterministic.

The headless adapter is `cargo xtask play --fake-ai`. It reads bounded JSONL commands from standard
input or a scenario file and emits one structured observation or error per line. The checked-in
berry-grudge scenario is replayed by the display-free verification gate. The visible adapter uses
the same `beastie-session` boundary; `fixtures/scenarios/room-shell.jsonl` exercises the room and
produces named PNG evidence without reading or writing the player's persistent save.

Platform-specific packaging may vary, but the core, protocol, save format, assets, and worker
contract remain identical across macOS, Windows, and Linux.
