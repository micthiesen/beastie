# Architecture

The architecture turns the MVP's central rule into dependency boundaries: AI creates expression,
while the simulation creates truth.

```text
player input --> beastie-core --> game events --> beastie-view --> beastie-game
                    |
                    +--> curated facts --> beastie-protocol --> beastie-ai-worker
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

Platform-specific packaging may vary, but the core, protocol, save format, assets, and worker
contract remain identical across macOS, Windows, and Linux.
