# Voxel surface review

## Baseline and diagnosis

Baseline: `371d3af`; contract: [voxel-surfaces.md](voxel-surfaces.md).
`target/feel/surface-before` and `target/captures/surface-before` record the original same-seed
16-second `voxel-surfaces.jsonl` sequence: quiet, affection, ball play, bell response and recovery.

Confirmed geometry issue: floor pitches exceeded block widths (0.158/0.154 and 0.22/0.21), and
rear ridge blocks were likewise undersized. The new substrate is a single occupied lattice.
Rocks and shelves now cull interior faces and share continuous surface boundaries.

Six native studies under `surface-study-*` use identical seed, scenario and old lighting with the
new surface code. Sharp, unlit, no-shadows and clay isolate contributions; beveled and separated
compare topology treatments. Shadows-off removes most dark horizontal lines on Mop and the bell.
This supports self-shadow artifacts as a major remaining cause, not a claim of physical cracks
in the creature. Unlit surfaces are continuous. The separated treatment fragments eyes and fins
and introduces a strong grid into the cave/substrate; it is rejected for the animal and terrain.

## Reviews and iteration

The first independent code review found no actionable topology issue. It examined shared lattice
coordinates, simultaneous crease displacement, concave corners and actual authored occupancy.
Bell, fin, cave, substrate, shelf and representative ellipsoids have two incident faces per exterior
edge. Focused tests cover closure, winding, nondegeneracy, normals and coplanar color boundaries.

The integrated material/light trial is recorded under `surface-light-{finished,contact,no-shadows}`.
Contact shadows produced noisy dark fragments on Mop and thin plants, so that experiment was
removed. `surface-tuned-01` uses a bounded cascade, explicit bias, lower-resolution Gaussian-filtered
shadows, directional ambient light and broad cave colors. Review accepted softer shadows/material
separation but identified corrugated animal shading and striped substrate terraces.

`surface-rounded-01` blends volume normals into the animal without changing its geometry. The
final substrate keeps its central bed flat and concentrates low banks at the rear corners.
`surface-final-check` captures five poses at the player's native 1920x1080 setting; this is an
intentional viewport difference from 1280x720 feel evidence. Independent visual review found the
world clear of material issues: rounded lighting preserves the voxel silhouette, quiet banks frame
the play space, and soft shadows/material differences remain readable. The final cleanup also
joins the formerly 95%-sized UI icon cells.

Two integrated code-review rounds and the final icon delta review found no unresolved material
issue. A reported point-light shadow diagnostic defect was rejected after checking installed
Bevy 0.19.1: its default is explicitly false. The fill now states that choice explicitly. Clay PBR
material properties were neutralized as a useful diagnostic refinement. A proposed bevel-divot
defect was rejected after a new long-crease regression proved identical cross-sections at cell
interiors and boundaries; the visible terrace ticks were actual contour turns. Fine substrate
therefore uses sharp geometry as an authored scale decision, not a topology workaround.

## Final native evidence

`surface-accepted-interaction-chain/interaction-chain` has 3,757 sequential frames at 60 fps.
Food notice, approach, inspection and consumption match the previous accepted craft recording;
comfort, play and first refusal timing are unchanged. Later fixture scheduling differs by at most
one frame. Final comfort at 61.316 seconds stops speech and clears mouth ownership in that frame.
Independent dense-frame and trace review found no material continuity or causality regression.

`surface-accepted-interaction-chain/dialogue-races` records 2,179 sequential frames. Delayed
request supersession prevents stale speech/captions, subtitles-off speech retains mouth ownership,
and restoring subtitles does not resurrect old text. Timing matches the accepted craft baseline
within one frame at speech onset.

`surface-accepted-quiet-observation/quiet-observation` records 10,812 sequential frames over
180.200 seconds. Every creature-state record and authoritative event matches the previous baseline.
All six private-life episodes retain their timing. Independent review of eight overviews and dense
bell, ball, sleep and cave samples found no visible seams, shading jumps or contour failures. The
face remains readable inside the cave and the tail stays connected.

`surface-accepted-bad-conditions/bad-conditions` records 2,603 sequential frames over 43.383 seconds.
Creature states match the previous baseline. AI, recognition and microphone failures remain
technical; deferred attention resolves without stale mouth ownership. A fallback caption expires
one simulation tick earlier after a slightly earlier real TTS completion, consistent with existing
expiry scheduling rather than a changed surface or simulation rule.

These four final native experiences are fresh recordings after the final icon geometry change.
Multiple independent visual and temporal reviews found no accepted material regression in the
exercised scope.

Final `surface-ui-shipping` review covers all 28 Normal/Large captures after the icon change,
including settings, bindings, trays, speech, focus and disabled controls. No clipping, overlap or
material readability issue remains in those captures. All ten `surface-{sharp,beveled}-final`
comparison captures were reviewed independently. Beveled remains the selected treatment for
its subtle bell and stone highlights while Mop stays continuous and expressive.

`surface-native-complete` records 5,405 sequential frames at 60 fps over 90.083 seconds.
Its native macOS event sequence contains 18 pointer presses/releases and 41 keyboard
presses/releases, without focus loss after activation. Settings tabs, bindings, Large text and
reduced motion were exercised. Screenshot 06 confirms “Hello Mop, how is the water today?”;
Enter produced `talk_accepted` at 12.616 seconds. The pointer dropped a berry at 16.733 seconds,
opened the moving creature's context at 18.483 seconds, and F4 comforted it at 20.233 seconds.
Presentation controls were restored and compose resumed at 24.116 seconds. Independent trace
and screenshot review found no material issue. Player settings and backup SHA-256 hashes are
unchanged before and after testing. The only native log warning was the known shutdown
window-destruction warning.

The headless gate passes 468 tests, 27 dialogue fixtures, 11 recognition fixtures and spoken-input
replay. Native fake-AI runs and all final recordings completed successfully. No accepted material
finding remains in this pass's exercised scope.

Native evidence consists of screenshots, dense sampled video frames and synchronized traces. It
does not establish human full-speed perception, physical speaker/controller feel or other desktop
hosts. Reference audio is authored timing evidence, not captured device output.
