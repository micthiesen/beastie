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
credential. The interactive game shell is intentionally minimal while the deterministic MVP
foundation is built. See [the MVP specification](docs/mvp-spec.md) and
[current project state](docs/STATE.md).

## Workspace

- `crates/beastie-core`: authoritative deterministic simulation and save state
- `crates/beastie-protocol`: versioned, validated dialogue protocol
- `crates/beastie-view`: simulation state to declarative render/audio plans
- `crates/beastie-ai-worker`: local JSONL worker process, currently fixture-backed
- `crates/beastie-game`: thin cross-platform ggez executable
- `crates/xtask`: developer commands and verification entry point

Real model weights and generated outputs are not committed. Their pinned metadata belongs in
`models/manifest.toml` and `assets/manifest.toml`.
