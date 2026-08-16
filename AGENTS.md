# Beastie

Beastie is an offline creature game where deterministic Rust simulation creates truth and a
small local model creates expression.

> This is a living document. Update it as conventions emerge; don't ask, just update.

**Read [docs/STATE.md](docs/STATE.md) first, then read
[docs/game-design-philosophy.md](docs/game-design-philosophy.md) before designing or changing
features.** STATE holds the fast-moving work picture; the philosophy is the canonical product
authority. Use `/next` when the next step is open and `/wrap` when ending a substantial session.

Feature plans, implementation choices, and reviews must preserve the philosophy's boundaries. If a
requested change conflicts with it, surface the conflict explicitly instead of letting code or a
lower-level milestone document redefine the game by accident.

## Always commit and push

This is a personal project. Commit and push directly to `main` whenever something is finished,
bundling any other pending project changes into the same push. No PRs and no asking first.

## Product invariants

- The simulation is the brain; the model is the mouth. Model output never establishes facts or
  directly mutates authoritative state.
- The game remains playable when AI inference or TTS is absent, slow, malformed, or crashed.
- Runtime play is offline. No account, cloud endpoint, API token, or required GPU inference.
- No death or permanent ruin from neglect.
- macOS, Windows, and Linux are first-class targets. Keep core tests headless and portable.

## Gate

Always run `cargo xtask verify` after changes. It must stay independent of a display, model,
GPU, audio device, network connection, or PixelLab credential. For visible game-shell changes,
also run `cargo xtask dev --fake-ai` on the host platform.

Do not rebuild native release binaries, installers, or complete model bundles after ordinary
changes. Those slow artifacts are explicit portability checkpoints, not part of the iteration
loop. Routine development stops at the normal build, lint, and test gate above unless the user
specifically asks for a native bundle or the chosen acceptance task requires one.

For macOS visible validation, stop yabai once if it interferes with Metal drawable acquisition or
window focus, and leave it stopped for the rest of the session or until the next restart. Do not
cycle background desktop tools off and on around every smoke run. The user prefers a temporarily
disabled convenience service over repeated automation busy work.

## Rust conventions

- Stable Rust, edition 2024, pinned by `rust-toolchain.toml`.
- Strong types, explicit return types, discriminated enums, small focused modules, and no debug
  leftovers or `println!` in library/runtime code.
- `beastie-core` is pure sans-I/O logic: no ggez, model runtime, wall clock, filesystem, or OS
  dependencies. Inject time, randomness, and outside effects.
- Saves are versioned, human-readable JSON. Protocol traffic is versioned JSONL and treated as
  untrusted input.
- Rendering is declarative: `beastie-view` emits plans; `beastie-game` executes them.
- Keep model and TTS implementations behind replaceable worker-side interfaces.

## Architecture

```text
crates/
  beastie-core/       deterministic state, behavior, memories, saves
  beastie-protocol/   AI request/reply types and validation
  beastie-view/       RenderPlan and AudioPlan projection
  beastie-ai-worker/  isolated local AI/TTS process boundary
  beastie-game/       thin ggez input/update/draw shell
  xtask/              verify, sim, dev, and future asset/eval tooling
assets/               generated/final asset contract and provenance
models/               pinned model metadata and hashes, not weights
fixtures/             deterministic saves, dialogue, render cases
evals/                real-model evaluation inputs and reports
```

## Project knowledge lives here

Durable decisions, test findings, model/runtime benchmarks, prompts, provenance, and gotchas
belong in repository docs, fixtures, manifests, code comments, or skills, never private memory.
`docs/STATE.md` contains pointers and decisions, not the underlying knowledge.

When unsure about tooling or structure, inspect sibling projects under `../` or `~/Code` and
match established house style where it fits. `../stillair` is the source peer for the `/next`
and `/wrap` state-continuity system; shared tooling is maintained through `/sync`.
