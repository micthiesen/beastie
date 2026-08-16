#!/usr/bin/env bash
set -euo pipefail

repo="$(git rev-parse --show-toplevel)"
out="$repo/fixtures/stt/audio"
scratch="$repo/target/stt-fixture-generation"

if ! command -v espeak-ng >/dev/null 2>&1; then
  echo "error: espeak-ng 1.52.0 is required" >&2
  exit 1
fi
if ! espeak-ng --version | head -1 | grep -q '1\.52\.0'; then
  echo "error: fixture hashes require espeak-ng 1.52.0" >&2
  exit 1
fi
if ! command -v ffmpeg >/dev/null 2>&1; then
  echo "error: ffmpeg is required" >&2
  exit 1
fi

mkdir -p "$out" "$scratch"

render() {
  id="$1"
  voice="$2"
  text="$3"
  raw="$scratch/$id-raw.wav"
  espeak-ng -v "$voice" -s 155 -p 47 -w "$raw" "$text"
  ffmpeg -hide_banner -loglevel error -y -i "$raw" \
    -ar 16000 -ac 1 -c:a pcm_s16le "$out/$id.wav"
}

render espeak-us-muck-berry en-us+f3 "Muck, do you remember the berry?"
render espeak-gb-why-there en-gb+m3 "Why did you put that there?"
render espeak-us-mushroom en-us+f3 "Do you want this mushroom?"
render espeak-gb-profanity en-gb+m3 "You're a weird little bastard."
render espeak-us-bell-sock en-us+f3 "Do you like the bell or the sock?"
render espeak-gb-asshole en-gb+m3 "You're being an asshole again."
render espeak-us-repeated-name en-us+f3 "Muck? Muck, are you listening?"
render espeak-gb-disfluency en-gb+m3 "Um, Muck, I, uh, cleaned your aquarium."
render espeak-us-noisy-aquarium en-us+f3 "I cleaned your aquarium yesterday."

mv "$out/espeak-us-noisy-aquarium.wav" "$scratch/espeak-us-noisy-aquarium-clean.wav"
ffmpeg -hide_banner -loglevel error -y \
  -i "$scratch/espeak-us-noisy-aquarium-clean.wav" \
  -i "$repo/assets/generated/audio/environment/underwater-loop.wav" \
  -filter_complex '[1:a]volume=0.12[bed];[0:a][bed]amix=inputs=2:duration=first:dropout_transition=0' \
  -ar 16000 -ac 1 -c:a pcm_s16le "$out/espeak-us-noisy-aquarium.wav"

rm -f "$scratch"/*-raw.wav

echo "Regenerated nine frozen STT speech fixtures in $out"
echo "Run cargo xtask stt eval to validate their recorded sizes and hashes."
