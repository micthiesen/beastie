# STT evaluation corpus

`cargo xtask stt eval` validates every WAV and scores checked-in protocol replies without a model,
audio device, display, network connection, or platform-specific speech service. The same corpus can
be sent through one persistent local `beastie-stt` process by supplying its Moonshine runtime paths.

The clips are synthetic development fixtures. Speech was generated with eSpeak NG 1.52.0 using
`en-us+f3` and `en-gb+m3`, converted by ffmpeg to mono PCM16 WAV at 16 kHz, and then frozen by byte
count and SHA-256. Reproduction uses the GPL-3.0-or-later eSpeak NG command-line program as an
external development tool; neither eSpeak NG nor its voice data is bundled with these fixtures.
The silence and pink-noise controls were generated directly by ffmpeg. The noisy speech fixture is
a deterministic synthetic mix. These files are useful for protocol, vocabulary, no-speech,
normalization, and regression checks. They are not evidence of accuracy on human voices, accents,
rooms, microphones, disabilities, or background speech.

Real reports are written beneath ignored `evals/reports/`. A shipping selection still requires the
real-human acceptance matrix in `docs/stt-runtime.md`.
