# Offline first-run contract

Platform installers copy a verified `dist/<platform>` package as one immutable application
directory. The first launch discovers `models/manifest.toml`, the selected GGUF, `beastie-ai-worker`,
`runtime/llama-server`, and the optional eSpeak bundle relative to the game executable. It never
downloads, extracts, or modifies runtime/model files and does not require an account or token.

Only user data is created on first run: the platform-specific Beastie save/config directory and
bounded speech cache. If a bundled model, runtime, or license is missing, the game keeps its authored
fallback dialogue and silent audio behavior; it does not fetch a replacement. Installers must not
redirect those user paths into the read-only application directory.

The package manifest is checked before installer assembly. Linux and Windows extracted staging
layouts can be checked again byte-for-byte before platform signing. macOS code signing necessarily
changes Mach-O bytes after staging, so the embedded manifest remains pre-sign provenance while
`codesign --verify --deep --strict` seals the final app and the retained DMG SHA-256 identifies the
distributed image. A changed staging file, symlink, unexpected network flag, missing provenance
file, invalid bundle signature, or mismatched installer hash is release-blocking.
