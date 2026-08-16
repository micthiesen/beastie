# Steam AI and content disclosure

This is the repository-backed draft for Beastie's Steam Content Survey. Recheck Valve's current
survey wording immediately before submission, then adapt these facts without broadening the claims.

## Pre-generated AI content

Beastie includes pre-generated pixel art made with OpenAI image generation and PixelLab. OpenAI
image generation produced the aquarium concept reference. PixelLab produced the aquarium
background, cave, plants, toys, and creature base and animation frames used by the game. Earlier
development assets remain recorded but are not necessarily shipped or used at runtime.

`assets/manifest.toml` records each asset's provider, generation identifier, prompt summary,
dimensions, palette reference, review status, and runtime status. Generated assets are reviewed
before selection. The asset checks enforce hard alpha, palette, dimensions, density, and animation
contracts. Code-native pixel geometry keeps essential faces, gaze, mood, effects, food, and
interaction cues legible when an optional overlay asset is absent.

## Live-generated AI content

During offline play, a bundled Qwen3.5 0.8B model generates short creature dialogue from bounded
authoritative game facts. A separately bundled eSpeak NG executable may synthesize that validated
text into speech. Speech synthesis does not establish game facts. Neither path contacts an online
service. The model has no filesystem access, shell access, tools, account, API token, or general
network access. Its loopback server requires a per-process authentication token and is supervised
by the game worker.

Model output is expression, not game truth. It cannot mutate creature state, create memories,
change preferences, unlock concepts, or award progress. Those outcomes come only from the
deterministic Rust simulation.

Optional spoken player input is transcribed locally by the bundled Parakeet TDT 0.6B V3 INT8
model inside Beastie's contained STT worker. Audio, recognition, dialogue, and speech synthesis do not contact a
cloud service. Raw microphone audio is written only as a bounded temporary WAV owned by the STT
worker and is deleted after recognition, cancellation, timeout, and failure. Raw audio is never
saved as game history. Recognized text follows the same bounded input rules as typed text and is
not persisted by default. Transcript retention is opt-in and transcript export is an explicit,
local player action.

## Intended mature content

The creature may learn profanity, crude jokes, non-graphic sexual innuendo, personal insults,
spite, and provocation. It may criticize the player's choices, habits, aquarium decor, food, or
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
creature to repeat. Dialogue requests contain only bounded recent-turn context, allow-listed
concepts, candidate memories and beliefs selected by the simulation, personality values, mood,
the authoritative current action and emotion, and explicit content-lane permissions.

The worker requires one versioned JSON reply with a short nonempty `say` field, an allow-listed
gesture, and only memory or belief identifiers offered in that request. It rejects malformed,
oversized, ungrounded, or prohibited output. The worker retries once with tighter instructions,
then returns a short authored in-character fallback. Invalid output never reaches memory or save
state. Dialogue and TTS failures leave the deterministic game playable.

Transcript retention is off by default and can be enabled or disabled in settings. Any transcript
export is an explicit player action and remains local to the player's computer.

The checked-in dialogue corpus includes permitted-sharpness cases and prohibited-content cases.
`cargo xtask verify` runs the fixture-backed protocol and safety gate without a model, display,
audio device, network connection, or generation credential. Real-model evaluation uses
`cargo xtask dialogue eval` and records the candidate, hash, invocation, latency, grounding,
fallback, permitted-content, and prohibited-content results.

## Release evidence

- `models/manifest.toml` pins the model source, immutable upstream and quantization revisions,
  byte count, SHA-256, license snapshot SHA-256, and model-card snapshot SHA-256.
- `THIRD_PARTY_NOTICES` records the shipped model and runtime licenses.
- `cargo xtask package` requires the exact dialogue model, model license, model card, llama.cpp
  runtime and license, eSpeak NG runtime and data, GPLv3 license and corresponding source archive,
  plus the STT worker, CC-BY-4.0 license/model card, and exact five-file Parakeet STT model.
- `package-manifest.json` records the byte count and SHA-256 of every packaged file and declares
  network access disabled. `cargo xtask package --check` rejects missing, stale, extra, tampered,
  wrong-platform, oversized, or development-only contents.

Before submission, recheck Valve's then-current disclosure wording and review the final packaged
asset set, store description, mature-content answers, screenshots, capsule art, selected model, and
content boundary. This document records implementation facts; it is not evidence that the Steam
survey or release review has been completed.
