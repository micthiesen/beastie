#!/usr/bin/env python3
"""Deterministic synthesis of Beastie's word-learned chime.

A rising three-note bell figure (major sixth to octave) with a soft chirp underneath: the sound of
the creature getting it. Pure-Python PCM, no samples, no randomness.
Usage: synth_learned.py OUTPUT.wav
"""
import math
import struct
import sys
import wave

RATE = 44_100


def bell(freq: float, start: float, length: float, gain: float, samples: list[float]) -> None:
    first = int(start * RATE)
    for index in range(int(length * RATE)):
        t = index / RATE
        envelope = (1 - math.exp(-t * 900)) * math.exp(-t * 7.5)
        tone = (
            math.sin(2 * math.pi * freq * t)
            + 0.42 * math.sin(2 * math.pi * freq * 2.76 * t) * math.exp(-t * 14)
            + 0.18 * math.sin(2 * math.pi * freq * 5.4 * t) * math.exp(-t * 24)
        )
        if first + index < len(samples):
            samples[first + index] += gain * envelope * tone


def chirp(start: float, length: float, gain: float, samples: list[float]) -> None:
    first = int(start * RATE)
    phase = 0.0
    for index in range(int(length * RATE)):
        t = index / RATE
        progress = t / length
        freq = 520 + 380 * progress
        phase += 2 * math.pi * freq / RATE
        envelope = math.sin(math.pi * progress) ** 2
        if first + index < len(samples):
            samples[first + index] += gain * envelope * (math.sin(phase) + 0.25 * math.sin(2 * phase))


def main() -> None:
    total = 0.9
    samples = [0.0] * int(total * RATE)
    chirp(0.0, 0.16, 0.22, samples)
    for start, freq in ((0.05, 1046.5), (0.14, 1318.5), (0.23, 1760.0)):
        bell(freq, start, total - start, 0.24, samples)
    peak = max(abs(value) for value in samples)
    scale = 0.38 / peak  # about -8.4 dBFS, level with the other creature cues
    with wave.open(sys.argv[1], "wb") as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(RATE)
        out.writeframes(
            b"".join(struct.pack("<h", int(max(-1, min(1, value * scale)) * 32767)) for value in samples)
        )


if __name__ == "__main__":
    main()
