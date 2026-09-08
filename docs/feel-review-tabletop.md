# Tabletop design review

## Baseline and calibration

Baseline a0bb98e, target/captures/ray-final-native/02-affection.png. The user finds the UI and
scenery generic and bland, and cannot immediately tell what labels mean. This overrides earlier
agent visual sign-off: legibility and absence of clipping were insufficient evidence of design quality.

Accepted findings: the 50-unit permanent deck competes with the animal; unlabeled behavior beside
the name is ambiguous; stacked outlined controls share equal emphasis; mirrored scenery and thin
plant silhouettes read as disconnected props. The approved direction is a crafted tabletop
aquarium, compact instruments, clear text roles and asymmetric habitat composition.

The existing simulation/input/audio ownership is a strength. Changes are presentation-only; all
technical feedback and access paths must remain intact. Contract: [tabletop-aquarium.md](tabletop-aquarium.md).

## Native iteration

`target/captures/tabletop-01` established the compact composition. Review found an oversized empty
name card and the old full-field focus outline still dominating the smaller rail. The second
candidate moves a smaller adaptive nameplate onto the frame, uses an input focus underline,
widens longer action labels, and reveals a physical tank apron above the rail.

`target/captures/tabletop-ui02` covers all 28 Normal/Large states. Review accepted the simpler
hierarchy and habitat, but found unresolved meaning in the rename prompt and speech reactions.
The rename field still said “Talk to Mop”; speech was an uncredited card above three unlabeled
pixel faces. These are accepted defects within this pass, along with tight Large action-title
spacing and ambiguous sound-setting labels. Implementation and fresh recapture follow.

## Code and causal review

Independent general and UI/accessibility reviews inspected the complete diff. The widened Large
speech minimum resolves the proposed reaction-label fitting concern. A request to trap focus in
every temporary panel was rejected as a new interaction policy: the visible global rail was also
available in a0bb98e, remains deliberate here, and is required for rename/text entry. No hidden
control or new destructive activation was identified. Local Close/Back and safe reset defaults
remain represented.

UI02 canonical interaction/race evidence in `target/feel/tabletop-interaction` validates all 50
manifest artifacts. All 3,757 interaction and 2,179 race creature states, events, inputs and audio
cue/command timelines match prior ray acceptance. Asynchronous speech differs by 1–3 frames;
supersession, caption ownership and direct interruption remain correct. Four dense strips found
no new habitat obstruction. Final UI03 recaptures are recorded below.

## Final UI and interaction acceptance

All 28 frames in `target/captures/tabletop-ui03` passed independent native-size review after the
meaning/spacing fixes. Normal/Large rename prompts agree with the field, speech names its speaker,
reaction labels are explicit, and the sound settings distinguish volume from enabling replies.
The larger transient speech controls overlap some tail in one Large frame, but preserve the face
and expression; this is accepted for readable choices. It is not evidence that arbitrary-length
text can never overlap the creature.

Final `target/feel/tabletop-final-interaction` contains 3,757 interaction frames and 2,179 dialogue
race frames. All 50 artifact hashes/sizes validate. All 5,936 creature states, events, inputs and
audio cue/command timelines match the previous ray-renderer acceptance. Asynchronous speech timing
differs by 1–3 frames; direct comfort still clears every owner at 61.316s, superseded replies never
return, and subtitles-off never produces a caption. Independent visual review inspected session
overviews, dense event strips and native speech/return frames for expression, contact and habitat
occlusion. No accepted material defect remains in those sampled scenes.

Final failure evidence in `target/feel/tabletop-final-bad/bad-conditions` validates all 23 artifacts,
2,603 frames and 43.383 seconds. Every creature state, all 2,593 events, 21 inputs and audio
cue/command timelines match prior acceptance. Asynchronous speech differs by 1–2 frames with
correct caption/mouth cleanup. Final color-only hierarchy tuning gives Feed a muted olive enamel
face and lets Settings/Close rest on the rail; `target/captures/tabletop-ui04` recaptures all 28
states after that change. It changes no layout, input, speech timing or simulation.

## Actual input finding and correction

The first 90-second OS-input run (`target/feel/tabletop-native-complete`, 5,405 frames) verifies both
full typed phrases, Backspace editing, both submissions, berry drop/consumption, F4 comfort,
restored Normal settings and bell-loop picking. It does **not** verify the claimed creature click:
at 21.000s the click (107,101) opens plant(2). Frame inspection places that point on Mop's visible
neck/body above the plant, while the old picker tests only the head. The expanded scenery proxies
also extend farther toward the camera than their real geometry.

The first correction caches each body's exact presented transform alongside the radii shared with its
mesh, then intersects those rotated/tapered volumes without normalizing the transformed ray.
Scenery depth now matches the authored plant/cave extents. Nearest-hit ordering is preserved;
there is no unconditional creature-priority shortcut. Regression tests compare cached transforms
with animated ECS entities, remove a segment to detect stale targets, and verify world ray distance
under rotation/nonuniform scale. Independent follow-up code review found no actionable defect.
The body ellipsoids do not claim exact picking of the decorative tail fin.

The next actual-input run (`target/feel/tabletop-native-verified`) successfully opens creature
context from head and body clicks, including the previously missing body case. It exposes a
separate composition defect: the fixed lower context menu covers Mop's face when Mop is low in
the tank. Context panels now choose the opposite vertical region, with clearance around the head
across the midpoint and at tank edges; food/toy choice menus retain their positions. This is an
accepted follow-up within the original creature-presence requirement, with explicit layout tests
and another native recapture required before completion.

Context placement is now anchored at opening, with a top panel at y4..48 or a lower panel at
y88..132, leaving the technical-feedback strip free. The creature continues living afterward;
it may swim behind a stationary menu, which is preferable to moving controls under the pointer.
The opening calculation uses authoritative position with generous face margins, while body picking
uses the preceding presented transforms. Full-height Normal/Large layout and stable-anchor tests
pass. No native release binary, installer or model bundle was rebuilt.

The final 28-state UI recapture (`target/captures/tabletop-final-ui`) passes independent review
after anchoring context placement. The 90-second `target/feel/tabletop-native-final` run verifies
head/body context, both typed phrases and submissions, editing, berry consumption, comfort and
bell-loop selection. A brief tail-tip occlusion behind the upper physical rim is accepted: the
face and body remain readable, and this pass does not change the camera or animation path.

That run added two early picking checks, shifting the creature away from the plant during the
later click. It therefore cannot close the original overlap finding. Repeating the original
16-action sequence in `target/feel/tabletop-native-overlap` reproduces plant(2) at 20.816s on
logical (107,101), despite the body cache and tighter proxies. Native input scripts derive a
point from authoritative position, which can lie on the visible neck rather than the rendered
head center. The screenshot still shows visible skin at that point. Scenery proxy volumes can
intercept rays through empty space between leaves; a precise scenery intersection is required.

## Capture-free cadence

A capture-free replay of the 28-state UI sequence, with each capture replaced by a 750ms wait,
measures 1,311 frames after 30 warmup frames on Apple M5 Max / Metal at 1920x1080. Debug fake-AI
wall-frame p50/p95/p99 are 16.66/25.19/25.88ms, close to the prior renderer's 16.69/25.33/27.44ms
UI sample. The logical geometry arena grows from 104,604,160 to 124,531,200 bytes with the fuller
habitat; this is not total GPU allocation. No build or second native game ran concurrently.
Report: `/tmp/beastie-tabletop-benchmark.json`. This sample precedes the final precise scenery
picking correction, which changes CPU pointer intersection rather than rendering. It does not
establish minimum hardware requirements or sustained thermal performance.

## Precise scenery picking correction

Plants and the shelter now use cached local mesh triangles, with a bounding-box rejection before
triangle tests. Instance transforms are captured after Bevy propagation, matching the preceding
rendered frame and plant sway. Cache invalidation follows mesh modifications/removals and hidden
or removed instances. Intersections are double-sided and retain the world ray parameter under
nonuniform scale. Head, body, toy and food targets retain forgiving volumes; closest distance
still wins. Tests exercise an actual authored leaf gap that the old proxy filled, solid leaves
from either side, the recessed shelter opening, transform movement, asset replacement and hidden
instance cleanup.

`target/feel/tabletop-native-scenery-final` repeats the original 16-action sequence without the
early extra checks that shifted timing. Its 2,704 recorded frames cover 45.050 seconds. At 20.700s,
logical (109,101) on visible Mop overlapping the same plant selects `target/creature` and opens
Comfort/Play/Inspect/Rename. Bell-loop picking succeeds at 34.616s. The trace records both accepted
talks, direct comfort at 22.400s and berry consumption at 33.983s. Native screenshots preserve
Large typing, Normal editing and the unobstructed context layout. Settings and backup hashes
remain identical to the pre-run baseline; scripted validation did not rewrite the user's settings.

Final `cargo xtask verify` passes 492 tests, 27 dialogue fixtures, 11 STT fixtures and spoken-input
replay. `cargo xtask dev --fake-ai` produced the final Normal/Large UI captures; the rebuilt normal
debug binary produced the final direct-input recording. The existing macOS compact-unwind linker
warning remains; it does not fail the gate.

Independent final code review found no concrete defect in cache invalidation, previous-frame
ordering, double-sided intersections or nearest-distance comparison. The shelter's doorway
correctly hits its recessed back rather than an invented front plane.

Independent native review extracted the final video's pre-click 20.650s and menu-open 20.700s
frames. The click lies on visible skin at Mop's right facial/eye edge, directly overlapping the
same broad-leaf plant beside the shelter. It opens all creature actions and keeps the face clear.
Large typed text, Normal edited text and the bell menu also pass. This closes the reproduced
overlap finding; the conclusion uses sampled frames and synchronized traces, not human full-speed
play or perceptual audio. No accepted material finding remains in the exercised scope.
