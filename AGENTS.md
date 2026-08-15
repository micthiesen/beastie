# Beastie

Beastie is an offline creature game where deterministic Rust simulation creates truth and a
small local model creates expression.

> This is a living document. Update it as conventions emerge; don't ask, just update.

**Read [docs/STATE.md](docs/STATE.md) first.** It holds the fast-moving work picture and chosen
next step. Use `/next` when the next step is open and `/wrap` when ending a substantial session.

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
