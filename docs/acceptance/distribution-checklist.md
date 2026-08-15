# Beastie V1 distribution acceptance

Release target date: **2026-09-30**
Owner: **release operator**
Status: **not release-ready until every required native row has evidence**

## Required before publishing

- [ ] `cargo xtask verify` passes on the release commit.
- [ ] Every platform package passes `cargo xtask package --check` with `network = false`.
- [ ] Package manifests, installer hashes, and source notices are retained as CI artifacts.
- [ ] The exact Qwen model, llama.cpp runtime closure, and eSpeak source archive match pinned hashes.
- [ ] Save directory is writable and remains outside the install directory after first launch.

## Native matrix

| Target | Artifact | Install/extract | Launch smoke | Save path | Cleanup | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| macOS arm64 | ad hoc signed DMG or notarized DMG | [ ] | [ ] | [ ] | [ ] | link/hash |
| Windows x64 | Inno Setup installer | [ ] | [ ] | [ ] | [ ] | link/hash |
| Linux x64 | AppImage or tar equivalent | [ ] | [ ] | [ ] | [ ] | link/hash |

For each native run, disable networking at the OS/firewall or runner boundary, start from a clean
user profile, verify the offline model/worker/TTS path, send one message, save, quit, and confirm no
`beastie-ai-worker`, `llama-server`, or `beastie-tts` descendants remain. A CI definition is not a
substitute for this evidence.

## Store assets and compliance

- [ ] `cargo xtask store-assets build` then `cargo xtask store-assets check` passes from the
  approved OpenAI-generated `steam/assets/source/key-art.png` source. Generated capsule art has
  the exact current Steam dimensions, source SHA-256 provenance, and only the code-native
  `BEASTIE` title treatment. `library-hero.png` remains art-only.
- [ ] Four native gameplay screenshots: aquarium idle, food drop, rejection/expression, and
  reunion. Each is captured from the shipped build at a minimum of 1920×1080, 16:9. Do not use
  store key art or fabricated gameplay images as screenshots.
- [ ] Trailer (30 to 60 seconds) shows feeding, intentional swimming, expression, and local-only
  dialogue/speech disclosure. No secrets, real account data, or debug overlays.
- [ ] Steam Input layout is `steam/input/beastie-gamepad.json`; action names match the shipped build.
- [ ] Achievement IDs are copied from the deterministic manifest without renaming.
- [ ] MIT, Apache-2.0, GPLv3 eSpeak license/source, model card, and third-party notices ship together.
- [ ] Apple signing/notarization, Windows Authenticode, and Steam credentials were supplied only in
  the private release environment and are recorded in the operator log, never in git.

## Evidence template

Commit: `________________`  Date: `________________`  Operator: `________________`

Artifact SHA-256s: `______________________________________________________________`

Observed save paths: `___________________________________________________________`

Observed child-process cleanup: `________________________________________________`

Exceptions and follow-up owner: `_______________________________________________`
