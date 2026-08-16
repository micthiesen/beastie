# Moonshine Tiny Streaming English model

Beastie uses the quantized Tiny Streaming English architecture 2 model published for Moonshine
Voice 0.1.2. The exact seven required component names, byte counts, and SHA-256 hashes are pinned in
`models/manifest.toml`. The package validator rejects missing, extra, symlinked, truncated, or
modified components.

- Source: `https://download.moonshine.ai/model/tiny-streaming-en/quantized_26_07_30`
- Runtime revision: `07648e45e0b1daf1923ce325cd61d624a407e615`
- Model bytes: 51,441,771
- Language: English
- License: MIT

This model performs local speech recognition. It may misrecognize names, accents, profanity,
speech differences, competing voices, or noisy rooms. Beastie treats its output as uncertain
perception, never authoritative state. Text input remains fully available.

Current evaluation evidence and the selected Parakeet default are documented in
`docs/stt-runtime.md`.
