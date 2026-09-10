# Beastie

Beastie is an offline aquarium creature game. A deterministic Rust simulation owns the truth,
while a tiny local language model gives the creature an unreliable voice. The creature swims,
notices food and the player, develops habits and grudges, and expresses authoritative state through
movement, face, sound, and scarce dialogue.

Beastie is the game's name. Mop is the default creature name; renaming the creature does not
change the game title.

The game uses a pure simulation core, a validated JSONL AI boundary, declarative 3D scene
plans, and a Bevy shell rendering a fixed-camera voxel aquarium. macOS, Windows,
and Linux are first-class release targets.

## Start here

On Ubuntu, install the native build libraries first. The CI and release-build workflows use the
same [Bevy Linux prerequisites](https://github.com/bevyengine/bevy/blob/v0.19.1/docs/linux_dependencies.md):

```bash
sudo apt-get install g++ pkg-config libx11-dev libasound2-dev libudev-dev libxkbcommon-x11-0 libwayland-dev libxkbcommon-dev
```

```bash
cargo xtask verify
cargo xtask play --scenario fixtures/scenarios/aquarium-v1.jsonl --fake-ai
cargo xtask dev --fake-ai \
  --script fixtures/scenarios/aquarium-v1-visible.jsonl \
  --capture-dir target/captures/aquarium-v1
```

`cargo xtask verify` needs no model, display, GPU, audio device, network connection, or asset
generation credential. The visible command uses fixture AI and exercises the real game shell.
`cargo xtask dev` defaults to the optimized `dev-perf` profile, retaining debug assertions and
overflow checks. Use `--profile dev` for unoptimized debugging. Renderer measurements, visual
comparisons and ordinary-GPU fallback behavior are documented in the
[renderer efficiency review](docs/renderer-efficiency.md).

V1 includes continuous deterministic aquarium movement, physical food, expressive animation and
gaze, a persistent compose interface, controller-equivalent semantic navigation, versioned save
migration and recovery, local transcript export, solid voxel aquarium geometry, authored sound, a warm
local Qwen dialogue runtime, and optional offline eSpeak NG speech. The original
[MVP specification](docs/mvp-spec.md) remains as historical design context. See the
[V1 plan](docs/v1-plan.md), [architecture](docs/architecture.md), and
[current project state](docs/STATE.md) for the implemented reality and remaining acceptance work.

## Workspace

- `crates/beastie-core`: authoritative deterministic simulation, aquarium objects, and save state
- `crates/beastie-session`: shared semantic command and observation boundary
- `crates/beastie-protocol`: versioned, validated dialogue and speech metadata
- `crates/beastie-view`: display-free render, UI, expression, and audio plans
- `crates/beastie-ai-worker`: fixture and warm llama.cpp worker boundaries
- `crates/beastie-game`: cross-platform Bevy window, input, audio, saves, settings, and transcripts
- `crates/xtask`: verification, simulation, evaluation, capture, asset, and release tooling

Real model weights and generated test outputs are not committed. Pinned model metadata lives in
`models/manifest.toml`; sound and font provenance lives in `assets/manifest.toml`.

Release staging, local inference, content boundaries, and Steam disclosure are documented in
[packaging](docs/packaging.md), [local mouth](docs/local-mouth.md), and
[Steam AI disclosure](docs/steam-ai-disclosure.md).
