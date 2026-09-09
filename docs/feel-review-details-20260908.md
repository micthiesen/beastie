# Detail feel review

Date: 2026-09-08. Review and implementation complete.

The lived experience is quiet observation punctuated by care, speech and settings adjustments.
The existing design remains the baseline. The user explicitly authorized implementing refinements
without another approval phase; the [contract](detail-polish.md) records the accepted scope.

## Observations

### D1: bindings merge into a single face

- Evidence: `target/captures/presence-02/large-bindings.png` and Normal counterpart.
- Observation: six button backgrounds abut vertically; the first focused row also touches Back.
- Intended reading: six independently adjustable keys, separate from panel navigation.
- Severity: medium clarity/craft issue. Layer: view layout.
- Acceptance: visible row gutters and header clearance at both text sizes, including focus rings;
  all bindings remain operable without clipped labels.

### D2: shadow lobes beneath toys

- Evidence: `docs/evidence/presence-20260908/opening.png` and fresh before first frame.
- Observation: ball and bell ground shadows look like several offset hard silhouettes.
- Intended reading: a soft area-light shadow grounding suspended belongings.
- Severity: low rendering craft issue. Layer: fixed six-direction binary shadow visibility.
- Acceptance: same-seed comparison reduces discrete lobes while preserving stable motion,
  readable face and reasonable measured native cost.

### D3: stepped water color

- Evidence: opening frame, broad unoccupied water; `environment::setup` emits 26 constant-color slabs.
- Observation: horizontal color changes cross otherwise quiet water.
- Intended reading: continuous depth behind deliberately stepped solid silhouettes.
- Severity: low rendering craft issue. Layer: authored backdrop geometry/colors.
- Acceptance: continuous gradient without cracks, extra animation or changes to tank bounds.

### D4: rebinding loses the originating row

- Evidence: `target/feel/detail-native/`, actual pointer selection of Food and F6 key entry.
- Observation: the value changes correctly, but the focus outline returns to Push to talk.
- Intended reading: finish adjusting Food and continue navigating from Food.
- Severity: low input continuity issue. Layer: shell's unconditional focus reset after capture.
- Acceptance: accepted and cancelled captures return focus to their originating binding row,
  including Normal/Large native input and regression coverage.

### D5: a recording can pass without being usable review evidence

- Evidence: rejected `target/feel/detail-final-01/first-five-minutes/`.
- Observation: 401 live pointer motions and six clicks changed the controlled sequence. Video
  contained six black intervals, including 148.950–300.350 seconds, despite passing codec/hash checks.
- Severity: high evidence-integrity issue. This does not establish a shader defect.
- Acceptance: controlled captures reject live input and black video; direct native input remains
  available for manual interaction review.

## Non-findings and limits

Initial independent causal/audio review finds truthful food and toy cues, varied quiet life,
unclipped sound reconstruction and memory-backed nonverbal return. Existing Normal/Large food,
context, speech, settings and data frames show no text clipping. These areas remain in the review
rubric; green traces do not certify human emotional response or physical listening.

## Implementation comparisons

The fresh five-minute baseline is promoted under
`target/feel/detail-before-20260908/first-five-minutes/`, binary
`83b919951c394a28a3983a31434f2e7f1225b578334ccae6f44c908172378579`, seed 42,
1280x720 Metal. Its manifest records dirty source because implementation began during recording;
the executable remained the unchanged baseline until promotion. The remaining baseline suite was
stopped to make room for controlled rendering comparisons. Its partial quiet attempt is invalid
and is not used. Broader baseline causal/audio review uses the prior validated evidence linked
from the presence review.

`target/captures/detail-before/` and `detail-candidate-01/` compare the same five surface/acting
checkpoints at 1920x1080. Independent visual review supports the continuous backdrop and softer
toy shadows, with pupils, catchlights, eyebrows, mouth and voxel silhouettes retained. No new
defect appeared in sampled first-five overview and dense talk/food frames.

The first candidate used twelve shadow rays everywhere and four coverage rays everywhere. It is
rejected for cost despite its visual benefit: native wall-frame median rose from 24.920 ms to
41.662 ms, and p95 from 25.661 ms to 50.055 ms. These include CPU, GPU backpressure, pacing and
capture overhead, not isolated GPU timings. Reports are in `/tmp/beastie-detail-*-timing.json`.
The next candidate retains six shadow samples on uniformly lit/occluded surfaces, refining mixed
visibility to twelve; an opposite coverage pair refines detected world discontinuities to four,
while UI always receives four samples. This was an intermediate candidate, not the retained renderer.

Further measurement rejected adaptive world coverage too. The selected renderer keeps the original
two-world/four-UI coverage and adds shadow samples only to mixed visibility on upward rough
receivers. On the same no-capture acting scenario, 1,075 measured frames after 30 warmup frames:

| Metal debug, 1920x1080 | Mean wall frame | Median | p95 |
|---|---:|---:|---:|
| Before | 20.628 ms | 16.832 ms | 25.257 ms |
| Adaptive coverage, ground refinement | 25.481 ms | 24.999 ms | 33.366 ms |
| Selected: original coverage, ground refinement | 20.891 ms | 24.640 ms | 25.328 ms |

The selected run's mean increased 1.28%, with nearly unchanged p95. Presentation cadence is
quantized, so the median's change must not be hidden or interpreted as an isolated GPU regression.
This paired measurement supports retaining the local quality/cost tradeoff; it is not a benchmark
of other GPUs or proof of 60 fps at this viewport. Both use the Apple M5 Max/Metal adapter. Preserve
the [scenario](evidence/details-20260908/timing-scenario.jsonl) and raw reports
[before](evidence/details-20260908/timing-before.json),
[rejected](evidence/details-20260908/timing-03.json),
[selected](evidence/details-20260908/timing-04.json). The selected measured executable hash is
`46e81aba26979bcf6d60db184bc3fbbe831fcfd2f6fb5b08b412ba2b52d8cbbe`; later focus restoration
does not execute in this benchmark. No temporal effects or new resource bindings were introduced.

## Rejected evidence and capture repair

`detail-final-01` is excluded from controlled and visual acceptance. It recorded 438 inputs versus
18 in the baseline; food moved from 54.166 to 51.183 seconds, the ball response from 64.233 to
65.250 seconds, and autonomous events from ten to twenty. The extra UI activity also added audio.
Black output started intermittently at 75.367 seconds and became continuous at 148.950 seconds.
The cause is unconfirmed; host focus, occlusion or display interruption are possibilities.

The runner now passes explicit `--feel-script-only`, records that mode in manifest version 8,
rejects unexpected input kinds and scans every video frame with FFmpeg black detection. It retains
`video-visibility.log` in the hashed evidence. The scan detects all six black intervals in the
rejected recording. The host drains live keyboard/controller/focus messages and skips live pointer
input in this mode; window close remains available. Direct captures without the flag retain native
input for interaction review. Headless tests cover flag constraints, message draining, input
validation and black-detection parsing.

Clean reruns use the normal debug executable in a temporary macOS app wrapper and a process-scoped
`caffeinate -di` lease. No release bundle or persistent desktop setting was changed. An early
ambiguous app lookup launched an older installed copy; it was stopped and is excluded from evidence.

## Final acceptance

The clean five-minute pass is `target/feel/detail-final-02/first-five-minutes/`, seed 42,
1280x720 Metal debug, executable
`bc5e359b71f40c71dbc7c57df4f6b19aa06af5532452b7d92a86447303430208`. Manifest 8,
all-frame visibility validation and artifact hashes pass. Native F5 and Feed input deliberately
sent during this run are ignored as intended; only the scripted input reaches the evidence.

Independent causal/audio comparison finds all 373 authoritative events identical in order,
value and simulation time, the same ten autonomous starts and identical final state at 300000 ms.
The seven markers, six UI actions and three session commands match; two baseline startup focus
records are absent. Playback alignment differs by 0–17 ms. All 29 playback-start records match
in sources, gains and owners. Eleven audio assets reconstruct with peak 0.414463, no clipping or
unavailable-output frames; 1,159 isolated bubble frames leave ambience unducked. These are
frame-clock reconstructions, not captured device sound or a physical listening judgment.

Independent final visual review examined overview strips 003, 007, 009 and 013, dense food/toy/talk
sequences and the full-resolution final capture. No material defect remained: expression and shelter
poses stay clear, shadows follow sampled movement, and the gradient remains subdued. The 28
Normal/Large UI captures under `target/captures/detail-ui-final/` separately establish readable
panels and row spacing; they predate the final shadow selection. Sampled images do not establish
human full-speed perception or inspection of every motion frame.

Independent code review found no actionable defect in the complete implementation or capture
repair. `cargo xtask verify` passes 524 tests, 27 dialogue fixtures, 11 STT fixtures and spoken-input
replay, including shader translation to Metal, SPIR-V and HLSL. Windows/Linux, lower-end GPU
performance, physical listening and controller comfort remain unclaimed.

The second accepted pass, `target/feel/detail-final-04/`, covers interaction-chain and dialogue-races
with the same final binary and clean video/input validation. Independent review found no material
visual or causal/audio regression. All 118 interaction-chain and 51 dialogue-race authoritative
events match the prior accepted presence baseline, including timestamps. Comfort clears active
speech/captions immediately; late replies stay superseded; all 196 subtitles-off frames have no
caption owner. Dialogue playback differs by one frame (92 versus 91); this is a recorded lifetime
difference, not a claim of identical device timing. Peaks are 0.424517 and 0.492644 with no clipping.

`detail-final-03` is excluded: it timed out after 126 seconds with only 57 seconds of simulation.
A duplicate ordinary debug instance launched by an app lookup was running beside it. That instance
was stopped; the retry completed. Avoid app lookups between capture processes because an absent
target can launch ordinary play. This failure does not establish a renderer regression.

Direct native captures `target/feel/detail-native-final/` and `detail-native-large-final/` omit
script isolation and cover actual Food-to-F6 acceptance, Escape cancellation on Cancel, and row
focus at both text sizes. Normal Tab advances from Food to Play; Large Food was restored to F2.
The old jump to Push to talk is absent. These are bounded manual checks, not canonical controlled
replays. Incidental draft text in the Large run was not submitted; script settings are not persisted.
An attempted final Back observation lost the CUA window and is not counted as additional evidence.

The required `cargo xtask dev --fake-ai` path also completed a bounded visible script, producing
`target/captures/detail-dev-final/dev-visible.png` at 1920x1080 with the final renderer. The retained
[opening](evidence/details-20260908/opening.png),
[Normal bindings](evidence/details-20260908/bindings-normal.png) at 00:34, and
[Large bindings](evidence/details-20260908/bindings-large.png) at 00:31 preserve representative
final evidence in the repository. All accepted findings D1–D5 are resolved; no known material
detail finding remains from these reviews. Hardware/perception limitations remain as stated above.
