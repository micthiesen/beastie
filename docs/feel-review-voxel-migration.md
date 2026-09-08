# Voxel migration review

The migration is accepted within the evidence limits recorded below. Native captures use the normal debug Bevy
game, fixture AI, the canonical eight-checkpoint scenario and the host Mac. No release artifacts
were rebuilt.

## Round 1

Evidence: `target/captures/voxel-first/`, all eight PNGs captured successfully through
`cargo xtask dev --fake-ai`. The observed window was 1920x1080 from existing window-size preferences.
These are actual Bevy GPU captures, not generated mockups. The scenario exited successfully.

### Observed visual findings

- `01-aquarium-hatch.png`, `06-expression-and-talk.png`, `07-affection.png`: a large rectangular
  shadow occupies the tank, and the oversized dark creature shadow dominates the props. The top
  tank structure casts onto its own back wall. Increase indirect illumination and prevent the
  decorative tank shell from shadowing itself; retain readable volume on the creature.
- The same frames show props apparently suspended above the narrow floor. Improve terrain depth
  and grounding without changing authoritative object positions or inventing contact.
- `06-expression-and-talk.png`: a short caption occupies a large empty panel, and fallback status
  truncates mid-message. Size speech to content and write concise complete operational status.
- `07-affection.png`: the trailing body makes a hard elbow above the head. Smooth distance-path
  sampling and recapture turns at real-time speed before accepting the fix.
- The face is readable at normal window scale. The affection still has softer eyes and a visible
  smile/head tilt. This is a still-image observation, not evidence about continuous motion.

### Independent code findings

- Readback initially paired images with mutable current state. Fixed by capture-time snapshots,
  ordered completion, separate submitted/completed frame indices and shutdown draining. Association
  and drain tests pass. Sustained native throughput remains to verify.
- Runtime errors initially returned successful exit status. Fixed by retaining failure state and
  returning a nonzero Bevy application exit result.
- Gaze carried only target type, so pupils could not track the actual cursor. Fixed with explicit
  target positions resolved from authoritative state; absent targets remain absent. View tests pass.
- Raw object coordinates and decorative time initially advanced in whole seconds. Added shared
  remainder interpolation for drawing/picking and smooth decorative time.
- Ordinary swimming incorrectly emitted attention rays through a generic effect branch. Added a
  distinct subtle wake. Carried socks now render visibly by the mouth instead of inside the head.
- Reduced shake did not suppress recoil and bell oscillation. Added flag handling and a regression.
- Text used window-wide wrapping and anchor-only occlusion. Native clip regions must follow content
  bounds and higher-layer panels. Corrected with clipped native text entities and layout regressions.
- Native logs reported Bevy slab allocator use-after-free diagnostics during dynamic mesh removal.
  Retained dynamic handles and stopped uploading empty meshes. Sustained recapture logs are clean.

## Rounds 2 through 4: composition and continuous motion

Evidence: `target/captures/voxel-second/`, `voxel-third/`, `voxel-fourth-cave/`, all eight
canonical native checkpoints completed each time. The rear wall no longer receives the enormous
creature/tank shadow, indirect lighting reveals the gold surface, cave/plant geometry reaches the
sand, and short speech uses a compact panel. Native frame inspection found no remaining material
scenery or caption-composition issue.

`target/feel/voxel-motion-first/interaction-chain/` is complete visual evidence but **invalid for
live audio timing**: its 62.63-second movie ran in about 43.58 wall seconds on the faster host
refresh rate. Added a shared 60 Hz monotonic gate for scenario progression and screenshot submission.
`target/feel/voxel-motion-paced/interaction-chain/` completed in 65.30 wall seconds including startup
and draining; a 1.585-second WAV occupies 1.55 seconds of its playback trace, instead of 2.42 seconds
in the accelerated run. The second speech owner is canceled when superseded. This validates timing
and ownership, not the subjective sound of the physical output device.

Dense frame samples around 8 seconds revealed tick-boundary head jumps. Shared, bounded continuous
head presentation now anchors the visible head, effects, carried objects and picking. Around
11 to 14 seconds, reversals folded the old distance-path samples into the head. Constrained joints
now preserve body spacing and length through turns and bounds. The next paced run exposed a separate
idle tail orbit: point-to-moving-parent guidance fed itself back into the rig. Trail tangent guidance
and retained near-stationary curvature replace that feedback. Reversal, boundary, catch-up and
600-frame stationary regressions pass. The final correction is verified by the accepted recaptures below.

## Independent code review

The general migration review covered host/input lifecycle, camera/picking, voxel surface geometry,
body animation, UI synchronization, capture ordering, assets/packaging and session/save/worker
integration. A second focused input/accessibility review found no additional source defect.
The focused capture review found an incomplete-readback error could abandon the encoder process.
`Ffmpeg` now closes input, terminates on failure and reaps on drop; a child-process regression passes.
The complete headless `cargo xtask verify` passed after these corrections.

## Actual macOS controls

Evidence: `target/feel/voxel-native-input/` (60 seconds),
`target/feel/voxel-native-controls/` (180 seconds), with privacy-safe native input traces,
GPU video, and host window screenshots under the latter's `host-screens/`.
CoreGraphics delivered actual host pointer/key events to the Bevy window; screenshot and state
changes confirmed delivery. The first mouse helper sent down/up too quickly and produced no event;
100 ms holds corrected the harness. This was not a game defect.

Confirmed food tray selection and world berry placement; compose input visibly held
“hello little swimmer”; Enter submitted it; clicking the creature opened its action menu; naming
accepted “Pebble”; clicking the bell opened its play/inspect menu; settings, large text, reduced
motion, keyboard activation, Escape, Tab, save/data and reset cancellation all operated.
These scripts use fresh non-persistent state, so native tests do not replace the player's save.

Actual large-text inspection found the final settings label borrowing the remaining panel height
and width, so “Save & data” became disproportionately large while compact values stayed too small.
This survived source review; row bounds and native recapture corrected it. The apparent
missing Close in Large mode was a hover tooltip disappearing when the pointer moved, not a missing
control. Data/reset/rename panels remained readable and behaved correctly.

## Baseline candidate: five-minute and quiet reviews

The seven-experience capture at `target/feel/voxel-final-baseline/` began after the settings and
stationary-tail corrections. The first five minutes completed in 303.04 seconds and contains
18,022 ordered 1280x720 frames at 60 fps. Its 1.5853-second WAV maps to speech from 26.200 to 27.766
seconds, with consistent caption/dialogue/TTS owners. All thirteen overview filmstrips and dense
idle/arrival/cave windows showed no prior head jump, body collapse or settled feedback orbit.
Normal settings now have consistent label/value sizes in the native frame at 64 seconds.

The 180-second quiet experience completed successfully, but dense cave-wake samples around 137.1
seconds revealed a remaining threshold bug: hover drift rose just above the velocity threshold
and restarted a broad tail turn. This is a material motion finding, not acceptance of that clip.
Add authoritative steering to the semantic creature scene and suppress path-turn guidance for
hover/settled steering even when drift crosses the numerical speed threshold. Preserve intentional
swimming. Focused regressions and fresh quiet/interaction captures must close this finding.

## Evidence limits

Visual adjudication in this tool environment uses GPU screenshots, marker filmstrips and dense
video frame samples, supplemented by real native input and synchronized traces. It does not provide
human full-speed video perception or physical listening. No full-speed viewing or audible speaker
review is claimed. No physical controller, Windows or Linux Bevy window was tested in this run;
portable headless and semantic controller tests pass. Earlier sprite-era distribution
and feel evidence is historical.

## Final control and UI correction

The complete seven-experience baseline finished successfully with no capture retry or failed
experience. Separate reviewers found no additional UI/caption/ownership finding in interaction,
dialogue race and bad-condition evidence. AI-off relationship evidence also completed.

Large settings needed one further geometric correction: value text had been allowed to extend
below its button to make it larger. Twelve-unit buttons, one-unit value insets and strict content
bounds now keep every value inside its control. Native screenshots and a bounds regression verify
this, including the final settings row.

The final native controls at `target/feel/voxel-accepted-native/` exercised Large text, all three
reduced-animation settings, food placement and typing. Screenshots under `host-screens/` show
legible controls without the former overflow. A native file comparison exposed a pre-existing
script-isolation flaw: preference toggles wrote the operator's settings even though game state was
non-persistent. Scripted sessions now suppress settings writes. Known display preferences were
restored (normal text, normal motion, window scale 3); the final native run left the saved settings
byte-for-byte unchanged before and after toggling accessibility controls. This comparison would
have failed on the old code.

## Accepted final recaptures

- `target/feel/voxel-accepted-quiet/quiet-observation/`: 180-second quiet scenario, exit 0 in 182.88
  wall seconds. All eight overview filmstrips plus dense 6 fps samples at 137–139 seconds show the
  cave-wake tail held in place, with normal travel bends elsewhere. The previous whip is absent.
- `target/feel/voxel-accepted-interaction/interaction-chain/`: 3,758 sequential frames at 60 fps,
  62.6333 seconds, 1280x720. Dense 8–14-second samples show continuous approach, local food contact,
  preserved body length through reversal and stable post-action settling. The second speech starts
  at 60.600 seconds and is stopped at the 61.333-second comfort action; caption, dialogue and mouth
  ownership clear together. Retained WAV lengths align with playback intervals.
- `target/captures/voxel-accepted-aquarium/`: all eight canonical native checkpoints completed
  through `cargo xtask dev --fake-ai`. Final speech and affection frames preserve the geometric
  face, readable text, clear tank composition and the same coherent animal.
- `target/feel/voxel-accepted-native/`: actual keyboard/pointer, Large text and reduced-animation
  controls pass; settings remain unchanged on disk. This is additional final-build native evidence.

Independent motion/face and timing/ownership reviews found no new material issue in the final
recaptures. These constitute repeated native review after the last material changes, with the
sampled-video and hardware limits above. General, input/accessibility, capture, final settings
geometry and script-isolation code reviews have no unresolved accepted finding. A proposed
one-frame input/animation ordering issue was rejected: input must resolve the preceding presented
pose before publishing the next animation frame.

`cargo xtask verify` passes 451 tests, 27/27 dialogue fixtures, 11/11 recognition fixtures and the
spoken-input replay. Asset validation rejects images and the old manifest; no ggez dependency or
legacy sprite code remains. Linux workflow YAML parses, and native dependency installation follows
Bevy's bundled Linux prerequisites. Native logs are clean apart from the observed benign winit
window-destruction warning; the debug linker emits an oversized-unwind-table warning. No release
binaries or model bundles were rebuilt.

No known material implementation or presentation finding remains in the exercised scope.
