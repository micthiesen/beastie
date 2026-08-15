# Beastie

Beastie is an offline, one-room creature game where a deterministic simulation owns the
truth and a tiny local language model gives the creature an unreliable voice. The game is a
Rust workspace built around a pure simulation core, a validated JSONL AI boundary, declarative
render plans, and a thin ggez shell. macOS, Windows, and Linux are first-class targets.

## Start here

```bash
cargo xtask verify
cargo xtask sim --seed 42 --days 3
cargo xtask dev --fake-ai
```

`cargo xtask verify` requires no model, display, GPU, network connection, or asset-generation
credential. The Mac-led MVP includes the playable three-day creature arc, deterministic saves and
migration, generated room art, authored sound, a warm local Qwen dialogue runtime, and optional
offline speech. See [the MVP specification](docs/mvp-spec.md), the chosen
[V1 aquarium plan](docs/v1-plan.md), and [current project state](docs/STATE.md).

## Workspace

- `crates/beastie-core`: authoritative deterministic simulation and save state
- `crates/beastie-protocol`: versioned, validated dialogue protocol
- `crates/beastie-view`: simulation state to declarative render/audio plans
- `crates/beastie-ai-worker`: fixture, warm llama.cpp, and offline TTS worker boundaries
- `crates/beastie-game`: thin cross-platform ggez executable
- `crates/xtask`: developer commands and verification entry point

Real model weights and generated outputs are not committed. Their pinned metadata belongs in
`models/manifest.toml` and `assets/manifest.toml`.

Release staging, local inference, content boundaries, and the Steam disclosure draft are documented
in [packaging](docs/packaging.md), [local mouth](docs/local-mouth.md), and
[Steam AI disclosure](docs/steam-ai-disclosure.md).
