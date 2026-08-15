# Development harness

Codex tests ordinary interactions through one semantic boundary instead of clicking screen
coordinates. Coordinate input is reserved for pointer mapping and real-device acceptance.

## Shared session boundary

`beastie-session::GameSession` is used by the headless adapter and the visible ggez shell. It owns
world state, RNG, semantic commands, observations, dialogue-request construction, and checkpoints.
The game shell owns persistent files, backups, settings, transcripts, wall-clock time, workers,
audio, windows, and real input devices.

Current semantic commands cover:

- cursor position and physical food drops at normalized aquarium coordinates;
- feed, play, comfort, tidy, talk, reaction, inspection, and real naming targets;
- fixed ticks and accelerated minutes;
- checkpoint save/load and authoritative inspection.

Commands never expose arbitrary state mutation. Protocol lines and strings are bounded, versions
and fields are validated, and malformed commands return structured errors without crashing the
session. A 15-minute advance coalesces 900 redundant `NeedChanged` notifications into one without
changing final world state, RNG, or meaningful event order.

## Canonical V1 scenarios

Headless interaction:

```bash
cargo xtask play \
  --scenario fixtures/scenarios/aquarium-v1.jsonl \
  --fake-ai
```

The scenario inspects the initial aquarium, moves the cursor, drops a berry at a real normalized
position, advances through its full action sequence, names the creature Muck, talks, plays with the
bell, comforts it, and proves checkpoint restoration after accelerated time.

Visible interaction:

```bash
cargo xtask dev --fake-ai \
  --script fixtures/scenarios/aquarium-v1-visible.jsonl \
  --capture-dir target/captures/aquarium-v1
```

The visible twin drives the same session commands and captures the logical framebuffer at eight
named checkpoints: hatch, food dropped, notice, approach, food resolution, expression/dialogue,
affection, and restored state. Scripted runs start from seed 42 and do not read or replace the
player's persistent save.

The berry-grudge and Stage 5 scenarios remain long-form behavioral regressions. `room-shell.jsonl`
is retained only as historical room-era coverage and is not the canonical visual target.

## What automated plans prove

Display-free tests cover fixed-point world mapping, action phases, authoritative objects, integer
asset crop/scale, six full-body moods, player-facing attention, protocol-driven speech frames,
authored effects, direct-reaction cue preemption,
persistent compose behavior, hover/focus parity, food-drop mode, controller hints, settings and
bindings, save/reset controls, transcript controls, naming, and viewport calculations.

The visible runner proves real plan execution and direct 320x180 capture without OS screenshot
permissions. `cargo xtask verify` stays independent of display, model, GPU, audio device, network,
and generation credentials.

## Native host evidence

The development Mac has previously proven:

- native ggez launch, window discovery, focus, and rendered-window capture;
- real pointer selection through the food flow;
- real keyboard text entry and Talk submission;
- direct logical framebuffer PNG capture;
- packaged local dialogue and eSpeak speech without environment-variable discovery;
- child-process cleanup after package exit.

Alacritty required Screen Recording and Accessibility permissions for host automation. Starting the
game through Alacritty avoided a blank or occluded Metal window. yabai has intermittently held the
release window on a blank first frame at high CPU; stopping the service for the focused run and
restarting it afterward made the same build render and accept input.

In the shared remote session, launch visible validation as an Alacritty child. The daemon itself
could not acquire a Metal drawable, and synthetic pointer or keyboard events sent from the daemon
did not reach the focused game. The final eight-frame scripted capture and packaged smoke both
completed through Alacritty; native pointer and keyboard evidence comes from the earlier direct
Alacritty run.

No physical controller was attached. Controller callbacks, semantic focus, actions, hints,
on-screen keyboard, and rebinding are automated, but a macOS IOHID user-device probe was rejected
before report delivery. A physical controller or appropriately signed virtual device is still
required for native controller evidence.

Windows and Linux have not received native V1 install, launch, save-path, offline-runtime, or
child-cleanup acceptance. Cross-compilation and CI configuration are not substitutes for those
runs.

## Final V1 acceptance loop

Before calling V1 accepted:

1. Run `cargo xtask verify` on the final integrated commit.
2. Replay the canonical headless aquarium scenario.
3. Run and inspect all eight visible aquarium captures at native integer scale.
4. Run the real local-model and real eSpeak smokes, including fallback behavior.
5. Launch the staged macOS package, confirm rendering and child cleanup, and retain the earlier
   direct native pointer and keyboard evidence unless a fresh direct GUI session is available.

Physical-controller and native Windows/Linux evidence remain honest release prerequisites unless
the release scope is explicitly changed. Record dated evidence in the acceptance documents rather
than turning an unperformed check into a prose claim.
