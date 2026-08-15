# Steam store assets

`source/key-art.png` is the approved 1672×941 OpenAI-generated, no-text key art. Do not replace
it, alter its dimensions, or add gameplay claims to it without updating `provenance.json` and the
reviewed source SHA-256.

Build every required store image offline and deterministically:

```sh
cargo xtask store-assets build
cargo xtask store-assets check
```

The builder uses a high-quality cover crop, restrained dark/contrast grade, and a code-native
chunky 5×5 `BEASTIE` glyph treatment. Capsules carry title text only; `library-hero.png` is art-only.
`library-logo.png` is the transparent logo upload. `source/app-icon.jpg` is the approved,
lossy canonical derivative of the key art used for the 184×184 app-icon upload; the builder copies
it byte-for-byte so the JPG remains portable and deterministic with the workspace's PNG-only
`image` feature set.

The four gameplay screenshots are deliberately not here. Capture them from the native shipped
build at a minimum of 1920×1080, 16:9: aquarium idle, food drop, rejection/expression, and
reunion. Never substitute key art or fabricated imagery for those screenshots.
