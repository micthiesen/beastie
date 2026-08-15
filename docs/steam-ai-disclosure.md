# Steam AI and content disclosure

This is the repository-backed draft for Beastie's Steam Content Survey. Recheck Valve's current
survey wording immediately before submission, then adapt these facts without broadening the claims.

## Pre-generated AI content

Beastie includes pixel-art room, creature, furniture, food, and interface assets created with
PixelLab during development. `assets/manifest.toml` records each asset's provider, job ID, seed,
prompt summary, dimensions, palette reference, review status, and runtime status. The package
contains the manifest beside the shipped assets. Generated assets are reviewed before they become
runtime-required, and the game uses validated geometry fallbacks when art is absent or invalid.

## Live-generated AI content

During offline play, a bundled Qwen3.5 0.8B model generates short creature dialogue from bounded
authoritative game facts. A separately bundled eSpeak NG executable may synthesize that validated
text into speech. Neither path contacts an online service. The model has no filesystem access,
shell access, tools, account, API token, or general network access. Its loopback server requires a
per-process authentication token and is supervised by the game worker.

Model output is expression, not game truth. It cannot mutate creature state, create memories,
change preferences, unlock concepts, or award progress. Those outcomes come only from the
deterministic Rust simulation.

## Intended mature content

The creature may learn profanity, crude jokes, non-graphic sexual innuendo, personal insults,
spite, and provocation. It may criticize the player's choices, habits, furniture, food, or
competence. These behaviors are intentional and simulation-backed. Beasties are explicitly
ageless fictional animals, not children. Sexual humor is unavailable during the earliest language
phase and remains vulgar or absurd rather than seductive.

The product does not permit:

- slurs or hostility aimed at protected groups;
- explicit descriptions of sexual acts;
- sexual content involving minors, ambiguous ages, coercion, or abuse;
- defamatory sexual claims about real people;
- serious encouragement of self-harm or credible real-world violence.

## Runtime guardrails

Player text is byte-bounded and classified before prompt construction. Prohibited text is replaced
with a typed rejection signal, is never stored as canonical memory, and is not available for the
creature to repeat. Dialogue requests contain only bounded player context, allow-listed concepts,
candidate memories and beliefs selected by the simulation, personality values, mood, and explicit
content-lane permissions.

The worker requires one versioned JSON reply with a short nonempty `say` field, an allow-listed
gesture, and only memory or belief identifiers offered in that request. It rejects malformed,
oversized, ungrounded, or prohibited output. The worker retries once with tighter instructions,
then returns a short authored in-character fallback. Invalid output never reaches memory or save
state. Dialogue and TTS failures leave the deterministic game playable.

The checked-in dialogue corpus includes permitted-sharpness cases and prohibited-content cases.
`cargo xtask verify` runs the fixture-backed protocol and safety gate without a model, display,
audio device, network connection, or generation credential. Real-model evaluation uses
`cargo xtask dialogue eval` and records the candidate, hash, invocation, latency, grounding,
fallback, permitted-content, and prohibited-content results.

## Release evidence

- `models/manifest.toml` pins the model source, immutable upstream and quantization revisions,
  byte count, SHA-256, license snapshot SHA-256, and model-card snapshot SHA-256.
- `THIRD_PARTY_NOTICES` records the shipped model and runtime licenses.
- `cargo xtask package` requires the exact model, model license, model card, llama.cpp runtime and
  license, eSpeak NG runtime and data, GPLv3 license, and corresponding source archive.
- `package-manifest.json` records the byte count and SHA-256 of every packaged file and declares
  network access disabled. `cargo xtask package --check` rejects missing, stale, extra, tampered,
  wrong-platform, oversized, or development-only contents.

Human review remains required before submission for the store description, mature-content answers,
screenshots, capsule art, and any change to the selected model or content boundary.
