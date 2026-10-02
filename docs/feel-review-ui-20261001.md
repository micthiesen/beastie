# Full UI and feel review, 2026-10-01

C3/C4 native stills establish the redesigned interface, readable Normal/Large typography,
compact panels, complete controls and corrected data receipts. Native pointer/keyboard checks
establish editing and ordinary interaction behavior. C6 completed all seven baseline experiences
and a supplemental direct-toy run. Its sock transfer closes the pickup jump. The longer review
found an inherited one-frame tail-side flip at a tank boundary, verified corrected in C7. C8
also selects a clear bottom-foraging destination after a moved ball occupied the old fixed
anchor. Its longer run exposes a separate neighboring-toy overlap during later sock contact.
That contact correction, independent source review and the 735-test gate are complete.
**Final C9 review is complete: ten native routes and two fresh repeats are accepted by the
visual, causality and audio lenses, with no known material issue remaining within the recorded
coverage.** Available evidence and unperformed hardware/playback checks are distinguished below.

The review covers every existing `UiMode`, `UiAction`, input route, settings page, shared
component and meaningful failure state. Twenty-one ImageGen targets and their reconciled
mockup errors are in [the target index](style-reference/ui-20261001/README.md); the
[implementation contract](ui-redesign-20261001.md) defines scope. Voxel creature/tank identity,
offline play, simulation authority, compatible saves and graceful AI/audio failure are retained.
The user authorized implementation, verification and direct push to main.

## Accepted observations and corrections

| Observed finding | Evidence | Implemented correction and acceptance focus |
| --- | --- | --- |
| Fragmented rectangular controls and crowded secondary screens | Historical native captures and source geometry | One rounded teal/brass component language; every page mapped to a generated target; compare fresh native stills at Normal and Large |
| Large text silently shrank to fit old bounds | Renderer and lettering implementation | Preserve requested role sizes, use proper bounds and grapheme-safe ellipsis; verify font geometry and native legibility |
| Inspect vanished without showing a result | Actual action dispatch discarded returned snapshot | Stable qualitative inspection card preserving selected target; verify live truths and all target families |
| Rename used a distant banner and 512-character talk field | View and input source | Dedicated 24-character field with local count, bounded Unicode input and controller parity |
| Missing utility hints, no press receipt, modal reverse traversal broken | Source review | Contextual utility hints, brief pressed state, correct Shift-Tab and rebind focus restoration |
| Technical advice and exported paths clipped into a narrow strip | Source geometry | Wrapping notice with explicit dismissal; verify long text and modal occlusion |
| Food-capacity instruction named an unavailable cleanup action | Action inventory | Explain that old food must clear; no invented cleanup command |
| Reset preservation was not reachable through Recover backup | Persistence path review | Validate and promote preserved reset generation; retain retry paths and test repeated reset and malformed files |
| Scripted data operations could reach real player storage | Startup and scenario source | All scripted settings/save/transcript files isolated under capture-local `.local-data`; test real reset/export/recovery there |
| Lower-layer miniatures could protrude through newer notices | Renderer depth arithmetic | Shared semantic layer stride and bounded miniature depth; verify adjacent-layer ordering and native overlays |
| Panels could pass clicks through to concealed controls or world objects | Source review of picking and hit regions | Opaque pointer blockers, covered controls excluded from focus, speech under modal layers |
| Persistent microphone failure covered a settings row | Layout review | Dismissible error notices retain the actual failed device state; right-side pages use the free left column |
| Leaving rename through a care action turned its draft into chat | Input review | Clear name ownership on mode transition and visibly disable the duplicate dock field |
| Maximum-length Large captions could lose their ending | Text bounds and protocol limit | Lossless bounded continuation pages with Back/Next, complete grapheme reveal and final-page reading time |
| Export basename could alias its source log during scripted checks | Persistence review | Separate `exports` namespace and preservation regression test |
| A reset request could leave old recognition blocking new input | Ownership review | Cancel and reclaim the displaced worker request before allowing a new owner |
| Unreadable primary reset source prevented valid sibling recovery | Persistence review | Try the independent last-good reset generation while preserving unreadable evidence |
| Backspace and bounded text insertion could split a joined emoji or combining character | Input helper review | Delete complete graphemes across native/controller routes and reject an overflowing inserted grapheme whole; preserve canonical scalar limits |
| Wide letters could still hide the end of a draft despite approximate tail fitting | View character-budget heuristic versus measured font wrapping | Use exact font-width tail fitting in editable fields; verify wide-glyph suffixes at Normal and Large |
| Repeated keys could edit an unfocused draft or repeatedly toggle settings | Native input dispatch review | Route repeats only to focused text editing; verify button focus isolation and held function keys |
| Native select-all inserted its shortcut letter and paste was unhandled | Candidate1 native CUA: `hello`, primary-modifier+A, `Mop` produced `helloaMop`; Paste timed out waiting for clipboard read | Add conventional bounded field selection/replacement/paste and event-ordered modifiers; verify both fields and stale clipboard ownership |
| Close hint appeared in the opposite top corner while Settings already labelled Close locally | Candidate1 native Settings after dock gear click | Keep utility guidance local and suppress redundant close hints when no useful adjacent placement exists |
| On-screen keyboard obscured Mop's face | All four Candidate1 keyboard screenshots in `after-surfaces-app` | Compact the keyboard into the lower band with the same readable type and complete controls |
| Inspection cards used excessive blank space for short descriptions | All 18 Candidate1 inspection screenshots in `after-surfaces-app` | Size the card to its content while preserving longest-name and discovered-fact cases |
| Large dock activity lost its useful ending | Candidate1 native settings final state displayed `content · finishi…` | Preserve the full finite activity description, use measured two-line layout and keep authored Large text size |
| Continuation speech covered the creature with mostly empty space | Candidate1 `normal-preview-caption-final-page.png` and `large-preview-caption-final-page.png` | Compact opposite-side pages sized to complete page content; preserve every word and reveal stability |
| Large speech and reactions were partially covered by Settings | Candidate1 `large-preview-speech-settings.png` | Explicit clear-column placement for Settings; complete-group suppression and retained reading time for tighter modals |
| Unicode caption rendered supported emoji as crossed missing-glyph boxes | Candidate1 `large-preview-unicode-caption-first.png` and `large-preview-unicode-caption-final.png` | Bundle a licensed monochrome emoji outline fallback with shared shaping for geometry and measurement |
| Previous data notice concealed a newer export result | Candidate1 `data-export-empty.png` and `data-export-after-recovery.png`; export files exist but old higher-priority status remains | Route transcript operation results through the current notice channel; recapture actual exports |
| Released toys remain visually merged with the resting creature | Valid baseline quiet-observation at 00:35.100/01:00.200 (bell) and 01:07.200/01:30.200 (ball, not carried) | Surface contact, quiet-rest separation and continuous release anchors; C6 quiet confirms clear bell/ball rests and preserved private-life rhythm |
| Accepted food/toy attention is obscured by generic activity or prior recovery | Valid first-five baseline 00:36.150–00:40.150 and ball acceptance at 00:54.216; initial layout work retained the old summary priority | Name authoritative targets during anticipation and simultaneous recovery without replacing the prior payoff or inventing contact |
| Spoken mouth animation uses the coarse simulation clock | Valid dialogue-races baseline has mouth phase 2 throughout all 65 speaking frames; `Game::update_frame` samples `WorldState.elapsed_ms`, which changes only on one-second core ticks | Drive articulation from actual owned speech playback progress; retain cancellation and no-audio fallback |
| Subtitles-off reply focuses an invisible reaction and swallows normal typing | Valid dialogue-races focus remains `reaction/laugh` with no caption; source always assigns that focus and returns before text editing when it is not `compose/input` | Choose visible compose focus without captions; preserve an already-open page and its focus on asynchronous reply arrival |
| Expired comfort intention strands a newer private journey | Valid dialogue-races at 00:08.083 clears travel while plant activity remains in Approach until the next input at 00:32.483; source unconditionally cleared travel on affection expiry | Expire the expression but release only its own Player/CursorSocial journey; preserve replacement private, relationship and cursor travel |
| Content-sized cards still reserve excessive blank space | Candidate 2 `large-preview-caption-modal-restored.png` has two rendered lines with about 150 px unused below; `large-inspect-open_water.png` has about 160 px between body and Close | Share actual font/fallback wrapping metrics between view layout and rendering; size stable complete pages and inspection content from those metrics |
| A previous export receipt covers the reset confirmation's lower rim | Candidate 2 `large-reset-cancel-default.png`, banner begins around y457 while modal ends around y468; actions remain readable | Clear the previous transient receipt when entering confirmation; preserve fresh failure reporting |
| Rename receipt overlaps the next food choice | Candidate 2 live native input: save `Élan 👩‍🔬`, then click Feed; name receipt crosses the food-card label/bottom area | Place notices in available panel-free bands, including the top band for bottom-anchored choices |
| Large one-action inspection strips truncated “Spend a moment” | C3 edge captures for cave, food, plant and open water | Shorten the authored heading to “Look closer”; all eight C4 Normal/Large strips fit |
| Carried sock jumped upward when released at the floor | Final behavior source review and edge projection regression | Use the same clamped mouth anchor for authoritative carry, release and rendering |
| Direct Play emitted delight/impact without changing the toy | C4 interaction-chain at 00:24.166 shows a roughly 20 px chin/ball gap and no ball response; baseline first-five/bad-conditions traces confirm the same missing direct payoff | Shared one-time ball impulse, bell strike and sock carry/release at contact; C6 chain, first, bad-conditions and seed-12 direct runs confirm physical response and exact ownership |
| Completed offline toy owner suppressed the return greeting | Independent direct-toy lifecycle review: accepted Approach became Recovery before the one-shot PlayerReturn trigger | Finish offline recovery before return handling; preserve online deferred-Talk recovery, exactly-once rewards and no stale contact cues |
| Sock pickup jumped sideways into the mouth | C5 seed-12 direct-toy frames 2227 → 2228, 37.116 → 37.133 s: roughly 79 px lateral jump while the head barely moves | Transfer the actual rendered pose to the moving mouth over 200 ms; keep authoritative contact immediate, effects/picking aligned, early release continuous and reset/load free of replay |
| Tail switched sides in a single frame during a floor-adjacent turn | C6 first-five 262.783 → 262.800 s: roughly 142 px tail-tip displacement with a nearly fixed head; baseline 101.833 → 101.850 s shows the same inherited fault | C7 preserves the prior feasible turn through tank-boundary constraints; affected tail displacement falls from 1.55146 to 0.06748 world units with identical authoritative state |
| A moved ball occupies the fixed bottom-foraging destination | C6 relationship 89–92 s shows the ball over the forehead/left eye during forage; fixed Bottom=(5000,9200) lies inside the Ball=(4785,8085) head/toy envelope | C8 quiet and relationship verify a clear bottom column, precise arrival and unchanged authored Act/Recover durations; no additional contact or reward |
| Sock contact holds the head inside a neighboring bell | C8 first-five at 203.283–205.266 s holds head `(6794,10000)` with zero velocity beside Sock `(7794,10000)`, inside stationary Bell `(6650,8700)`; exact 202–215 s frames show crown/forehead and then brow overlap until recovery clears it | Select a clear alternative surface of the intended toy when the original contact position intersects another uncarried toy; preserve the physical payoff and unrelated toy state; C9 five-minute recording verifies a clear held pose, continuous carry/release and one response |

## Verified interface and input evidence

All counts below refer to original native PNGs individually reviewed at full resolution.
Capture manifests retain file dimensions, hashes and scenario provenance. Expanded redesign
fixtures establish added coverage; only the unchanged `ui-detail-review` and `voxel-craft-ui`
fixtures provide same-scenario before/after comparisons.

| Evidence | Established result |
| --- | --- |
| C1, 152 stills in `after-surfaces-app`, `after-settings-app`, `after-presentation-app`, `after-data-app` | Established the shared surface language and exposed the first follow-up findings; superseded as acceptance evidence |
| C2, 188 stills; binary SHA-256 `3a3df902536cd4024c08250ec46bfdf8492017001c513b307abb148f09e156de` | Exposed exact text-height mismatch and stale-notice overlaps; retained for comparison |
| C3, 188 main stills: 56 surfaces, 44 settings, 72 presentation, 16 Normal/Large data | No material visual finding in these batches; exact content sizing, continuation, selection, wide tails, notice placement and reset confirmation verified |
| C3, 28 `final3-detail` and 28 `final3-voxel` stills | Matched against the 56 valid baseline stills and ImageGen targets; no actionable visual finding |
| C4, 23 `final4-edge` stills | Full “Look closer” strips, long names, joined emoji and populated-export receipt fit at both text sizes |
| C4, nine `final4-sizes` and nine `final4-sizes-large` stills | Normal/Large HUD stays within 640×360, 1280×720, 1920×1080, 2560×1440, 3200×1800 and 3840×2160; fullscreen is actually 3024×1898 and restores 1280×720 |
| C5, 23 `final5-edge` stills | Independently reviewed with no material finding; a real fixture reply exports as one preserved record (`backend=fixture`, `fallback=false`), and the receipt correctly says “1 turn” |
| C6, two `final6-capacity` stills | Actual thirteenth food drop is rejected after twelve accepted berries; Normal/Large both show the complete, accurate “Tank is full. Wait for old food to clear.” receipt |

All roots in that table are under `/tmp/beastie-ui-20261001-*`. C3 binary SHA-256 is
`ec55afa54459a685b5e5d52f16ac438588b2b4f0c13a52c8422a52259690e583`; C4 is
`926adf439133b222a88cfca1e6d6ca5aa99d95b98a7d01e861a68e97a4f5aae0`.
Their `candidate3/provenance.json` and `candidate4/provenance.json` retain source patch,
untracked-file hashes, compiler and build command. Independent inventories and findings are
`/tmp/beastie-ui-20261001-native-visual-final3-*.md`,
`...-native-visual-final4-edge.md` and `...-native-visual-final4-sizes.md`.

The settled size fixtures wait 15 seconds after changes. They establish HUD layout, including
Large two-line activity labels, not resize latency, every modal at every size or physical
readability at 640×360. The C4 edge export and its preserved `.bak` each contain one identical
valid sanitized record, SHA-256
`e05e7fbf3f1a287c8e23147bf2d57b84636df33f87002ce3bb3e978a8de7854f`.
The recorded `fallback_reason=worker_unavailable` is real evidence of populated fallback
export, not successful worker dialogue. The captured “1 turns” copy predates the singular fix.

Native C2 pointer/keyboard evidence is in
`/tmp/beastie-ui-20261001-native-input-ledger.md` and the native tool transcript. It covers
Command+A selection, replacement typing, joined-emoji paste and complete-grapheme Backspace
in message/name fields; rebind swap/cancel/defaults; all three food placements and cancellation;
title Settings return and Quit. Native pointer clicks exercised the keyboard's letters, Space,
punctuation, Delete and Save after a scripted controller-route open. Separate fixture-worker
runs showed successful captions alongside Settings, generated two TTS WAVs, and accepted a new
draft after a subtitles-off reply. CUA clipboard-read timeouts are retained as tool failures;
subsequent native screenshots independently show the correct pasted value.
C5 real-input rechecks confirm message selection/paste/grapheme deletion, Large settings,
rename followed immediately by Feed with a clear top-band receipt, and the complete native
`mushroom` hover label. These use fresh isolated storage and are recorded in the same ledger.

## Matched baseline and preserved strengths

The frozen baseline is commit `0f393d86f84b98485ce17cb5ff8d7a4f0a59a8e4`, copied to
`/tmp/beastie-ui-20261001-baseline-source` without altering the shared checkout. The immutable
executable and provenance are in `...-before-binary`; binary SHA-256 is
`e0aee692acc06d12debf8284b03895d5169d9996138ff20929daa593696aed8a`.
The untouched baseline produced 28 valid PNGs each in `...-before-valid-attempt` and
`...-before-extra`. Historical committed screenshots informed design but are not new native
behavioral evidence.

All seven baseline experiences passed capture validation with that executable, seed 42,
1280×720 output and an explicit 1,800,000 ms recording deadline. Their exact scenario hashes,
paths and timings are indexed in `/tmp/beastie-ui-20261001-baseline-ledger.json`; each bundle
retains its manifest, attempts, movie, synchronized traces, filmstrips and reconstructed audio.

| Experience | Frames | Last playback timestamp | Capture wall time |
| --- | ---: | ---: | ---: |
| first-five-minutes | 18,022 | 300.350 s | 677.275 s |
| quiet-observation | 10,812 | 180.183 s | 313.011 s |
| interaction-chain | 3,997 | 66.600 s | 98.303 s |
| dialogue-races | 2,179 | 36.300 s | 61.470 s |
| bad-conditions | 2,603 | 43.366 s | 71.328 s |
| relationship-over-time | 5,850 | 97.483 s | 136.483 s |
| relationship-over-time-no-ai | 3,255 | 54.233 s | 74.983 s |

Baseline traces preserve several strengths: delayed dialogue, queued TTS and active speech
cancel coherently; subtitles-off speech retains its actual owner; speech ducks the creature
layer and bed while preserving physical-cue gain; reconstructed audio does not clip.
Technical failure remains distinct from creature refusal, uncertain speech causes no action,
and return callbacks cite recorded history and precede fallback expression with movement.
The no-AI relationship run retains deterministic fallback captions, so it is not wholly
nonverbal. Ordinary cave shelter intentionally stays silent; the cave-settle cue belongs to
a salient familiar-place relationship beat, as clarified in the audio event map.

C4 motion bundles are `...-after-feel-chain-v4/interaction-chain`,
`...-after-feel-quiet-v4/quiet-observation` and
`...-after-feel-dialogue-v4/dialogue-races`. Their manifests match the C4 binary and baseline
scenario hashes. The chain's 00:24.166 defect above is intermediate evidence, not C5 acceptance.
Trace comparison established that deferred Talk did not cause the missing physical payoff.
The readable private diagonal ball nudge and bell strike must remain intact in final comparison.

### Complete C6 comparison and C7 boundary correction

C6 completed eight valid bundles with 49,612 synchronized frames. Every manifest artifact hash
was checked. Its executable SHA-256 is
`e2b19b4c02350b7b6acc30d6b4f3dce4ab3a68cac192a38908b2089ed1512b40`; provenance is in
`...-candidate6`, and the bundle index is `...-final6-ledger.json`.

| C6 experience | Frames | Last playback timestamp | Capture wall time |
| --- | ---: | ---: | ---: |
| first-five-minutes | 18,021 | 300.333 s | 511.083 s |
| quiet-observation | 10,812 | 180.183 s | 311.651 s |
| interaction-chain | 3,998 | 66.616 s | 174.029 s |
| dialogue-races | 2,179 | 36.300 s | 63.628 s |
| bad-conditions | 2,603 | 43.366 s | 59.662 s |
| relationship-over-time | 5,850 | 97.483 s | 234.209 s |
| relationship-over-time-no-ai | 3,255 | 54.233 s | 81.863 s |
| direct-toy-contact, seed 12 | 2,894 | 48.216 s | 70.791 s |

All seven unchanged cases use the baseline's seed 42 and exact scenario hash. The additional
seed-12 direct run has a matched untouched-baseline capture in `...-before-feel-direct-v1`.
Independent visual, causal and audio reports are `...-visual-<label>-final6.md`,
`...-causality-<label>-final6.md` and `...-audio-<label>-final6.md`; chain/dialogue share a report,
and relationship's causal report also covers audio.

- Direct ball, bell and sock play each produce their actual response once, with the exact
  interaction owner. Rejection remains unrewarded. The C6 sock pickup preserves its preceding
  pose at contact and converges over 200 ms, removing C5's roughly 79 px one-frame jump.
- Chain contact is physical at 25.166 s. Recovery survives deferred Talk and finishes on the
  next tick. Quiet cave life remains intentionally silent.
- Affection expiry at 8.083 s in dialogue-races preserves the newer plant journey, which now
  completes at 18.083 s. Subtitles-off replies retain compose focus. Mouth phase follows owned
  playback, and cancellation immediately releases it.
- Quiet bell and ball rests are clear at 35 s, 70 s and the final 180.116 s state. All six private
  activities/rests and the ambient bubble cadence remain. Bottom forage at 157.117 s is a
  separate accepted overlap, addressed in C8.
- Relationship speech owners and queued cancellation are correct. The C6 berry is consumed
  before deferred speech starts, so C6 does not establish physical-cue/speech overlap at the
  earlier baseline's 48.433 s boundary. The baseline and unchanged reducer tests retain that
  ducking evidence; C6 establishes the actual affection/speech overlap at 14.333 s.
- The no-AI return starts moving before its fallback caption. It remains playable without
  inference, but is not a wholly nonverbal route. Reconstructed PCM is unclipped; this is not
  physical output audition.

Two suspected issues were dismissed on precise evidence. Relationship lower-cheek occlusion at
70.5/85.666 s is brief interrupted travel, not a stuck resting pose. The fixture's direct
`SessionCommand` submission bypasses the ordinary 100 ms input press receipt, field clearing and
UI confirmation; it does not establish a missing receipt in the real input path. Neither
observation justifies a general collision system or a new conversation-waiting mode.

C7 changes only articulated body constraint projection. Its first-five executable SHA-256 is
`3ae5fdf1ecd65d666068d7a27f4d11c3463c793d96ba5b8ba3b2f9a05c73caf7`.
The complete 18,021-frame run takes 592.053 s wall time for 300.333 s authored playback.
Inputs, events and markers are byte-identical to C6, and every creature state is identical.
At the defective 262.800 s boundary, tail displacement falls from 1.55146 to 0.06748 world units
and the affected segment rotation from 3.130224 to 0.08875 radians. Both independent reviewers
inspect adjacent original frames and all remaining alert clusters: no additional body issue.
Three pending-TTS frames differ, but not actual speaking ownership; asynchronous completion is
not claimed byte-identical. Reports are `...-visual-first-final7.md`,
`...-causality-first-final7.md` and `...-body-turn-review.md`.

### C8 candidate and additional contact finding

C8 is frozen in `...-candidate8`, executable SHA-256
`98e2c7f948cc488a74b132ae5d0d732d9a5d8acefaf175b5f468793d03eba5a1`, built through
`cargo xtask dev --fake-ai --smoke` with two jobs. The frozen tracked patch hash is
`e1791f66f77dcc8d6e0fb796052bbcfcd4763f2a207b14db7c26ee840a40d825`.
The audit at C8 capture time matched all 38 tracked non-document patches and 13 new
non-document files with that frozen candidate. The later neighboring-contact correction
supersedes that source and requires a new frozen executable and native review.

Quiet foraging now reaches `(3424,9200)` at 154.100 s before entering Act at 155.100 s.
Act and Recover each retain two seconds; the nearer clear destination shortens approach,
not authored activity duration. Native frames at 155.100, 157.117 and 159.100 s show a clear
face beside the moved ball. All six activity bouts, both physical responses, toy state,
relationship state and memories are preserved. The minimum normalized ball-clearance distance
during Act/Recover is 1.08997, outside its quiet-rest ellipse. All twelve ambient bubbles
retain their baseline timestamps; no new forage sound or duplicate contact cue appears.

Relationship foraging reaches `(3665,9200)` at 89.383 s before Act at 90.383 s. Native
88.5–94.5 s frames close the lingering forehead overlap. All event batches, timestamps and
ordering match C6; only position, velocity and facing change. Return owner `2770001` and the
single 65.283 s ball contact/reward remain intact. Brief ordinary travel projection at
72–73 s is still outside the stationary-clearance claim. Actual C8 audio includes separate
affection/speech overlap at 14.333–14.483 s and food/speech overlap at 52.433–52.516 s.
All 164 speaking frames have the correct owner, with both open and closed mouth phases;
cancelled TTS never plays and reference PCM has no clipping.

Focused C8 source review and five new regressions cover the observed ball placement,
all-toy/carried exclusion, 343 bounded layouts, precise arrival and one complete forage cycle.
The precise-arrival test starts 170 units short of the clear endpoint, inside a nearby ball's
envelope: the old generic 220-unit threshold would stop there, while the new six-unit threshold
first reaches the safe endpoint without paying a contact or prematurely entering Act.
The independent source report is `...-bottom-clearance-review.md`. C8 per-experience visual,
causal and audio reports use the same names as C6 with `final8`; full capture counts and final
acceptance are recorded below.

C8's first-five run retains the final 300,000 ms simulation endpoint and all authored markers,
with 18,023 recorded frames ending at 300.366 s. Precise bottom arrival legitimately changes
which phase the later food input interrupts, and therefore later private-life selection. The
run accounts for eleven activities as nine completed and two interrupted, including an extra
private sock visit. Direct feed/play/talk and memory counts remain intact. At 54.200 s food
consumption followed by the 54.233 s ball invitation, the dock says “watching the ball” while
the prior food recovery finishes, directly closing the acknowledgement overlap finding.

The extra sock visit supplies the accepted neighboring-toy failure above. The head is stationary
at `(6794,10000)` during Sock Act from 203.283 through 205.266 s; Bell `(6650,8700)` is also
stationary and uncarried. This is 0.4861 of the bell's contact ellipse and 0.4452 of its quiet
envelope. Original frames show the bell across the crown/brow for roughly two seconds, then
across the left face edge during recovery until separation improves at 209–212 s. Eyes and
mouth remain readable, but the composition appears physically merged. Both independent lenses
accept this as a material contact-pose issue. All physical response ownership is otherwise
correct; no new body-turn issue appears. C8 is retained as valid failure evidence, not final
acceptance. Reports: `...-visual-first-final8.md` and `...-causality-first-final8.md`.

### Final C9 source

C9 is frozen in `...-candidate9`, executable SHA-256
`afe07e9de28f4507c37b1fe5395a03726a6c66f74f360b2524bad418bf79793e`.
Its tracked patch hash is `3e1f4f293aab0ed08ee30f5ea17ebf88fcca722ceaa75a8941df96c496ed5c7a`.
The final normal development smoke is `...-candidate9-smoke-final`. A content-hash audit of
38 changed tracked source files and 13 new non-document files ties the reviewed executable
to the committed source, independently of later documentation edits.

The original intended-toy surface remains exact when it clears neighboring uncarried toys.
Otherwise, a bounded ring of 128 integer directions supplies alternative contact points with
candidate-side asymmetric radii, tank limits and the neighbors' quiet-rest envelopes. Stable
distance/coordinate ordering chooses the closest available sample. Arrival checks actual head
clearance, not merely proximity to a safe target. This is sampled contact selection, not a
continuous-space pathfinding guarantee.

If no sampled contact is clear, the direct/private/relationship approach interrupts once
without a payoff or invented refusal. Source review caught a missing relationship cleanup:
a familiar-place toy beat could otherwise reinstate impossible travel each tick and hold
deferred language indefinitely. The final branch interrupts only the exact matching beat ID
and target, releases movement and dialogue ownership, and schedules ordinary idle behavior.
Five regressions establish unchanged clear targets, carried-neighbor exclusion, bounded
determinism, convergence and exact contact in the observed sock layout, direct/private
equivalence, and fully blocked direct/private/FamiliarPlace/SharedToy cleanup with later input.
The SharedToy regression initially lacked the required 45-second memory age; its public-trigger
fixture was corrected without relaxing lifecycle assertions. Independent source closure is
`...-neighbor-contact-review.md`.

### C9 native contact closure

The first fresh five-minute run completes all authored inputs and markers, with 18,021 frames
through 300.333 s. Sock activity 8 now reaches `(7788,8200)` above the sock and holds Act from
205.250 to 207.250 s. The minimum neighboring-bell quiet-envelope distance throughout Act and
Recover is 1.025, outside the 1.0 boundary. Exact native samples show the crown, brows and eyes
clear; the bell projects only over the trailing body. One owned sock response/rustle occurs at
207.250 s, followed by continuous pickup, release and completion at 209.250 s. The unrelated
bell stays put and gains no response or reward. A representative original frame is
[preserved with the native examples](style-reference/ui-20261001/implemented/clear-sock-contact.png).

The alternative approach takes two additional simulation seconds. Later private activity starts
shift accordingly, while authored two-second Act/Recover durations and direct feed/play/talk,
memory and development totals stay unchanged. Final Bell activity 11 responds at 299.250 s and
is still legitimately in Recover at the five-minute endpoint; its completion is outside this
recording, not claimed as observed. The clear Bottom arrival, food-to-ball acknowledgement,
body-turn continuity, earlier sock response and deliberately silent optional-voice fallback all
remain intact. Reference PCM has zero clipping. Independent reports are
`...-visual-first-final9.md`, `...-causality-first-final9.md` and `...-audio-first-final9.md`.

The fresh quiet run preserves all 10,812 creature states, semantic event batches and audio
commands from C8. All six private bouts complete, both physical toy responses retain exact
owners, and clear foraging ends with Ready handoff. Ambient bubble cadence and quiet gaps are
unchanged. Its synchronized recording is 180.183 s; it is not a host-performance measurement.

C9 relationship also preserves the direct ball response at 65.283 s, the return beat and exact
save/load creature state. The changed, then interrupted sock approach leaves the creature higher,
so subsequent food consumption occurs at 50.166 s instead of 52.166 s. Later activity selects a
bell visit and a valid ComfortRitual, rather than the C8 late Bottom bout; C9 quiet and first-five
provide the fresh Bottom evidence. Cancelled reply 3 never plays or drives the mouth. The actual
food/speech overlap at 50.366–50.450 s preserves physical gain and speech/ambience ducking.

Chain and dialogue creature states and semantic event batches match C6 exactly. Ball contact,
deferred Talk, refusal, cave return, affection expiry and later cancellation remain correct.
All 196 subtitle-off dialogue frames retain `compose/input` focus. Bad conditions now reach the
ball one second earlier because of the preceding changed refusal route, still with one exact
response and no bell payoff. No-AI relationship event batches match C6; the return precedes its
fallback caption, completes at 52.133 s and resumes private life without speech or mouth motion.

The supplemental food route uses seed 8 and real drops. Mushroom rejection at 11.033 s emits
one spit/reject cue, preserves hunger and creates one rejection memory. Berry consumption at
29.083 s emits one eat cue and its normal benefit; subsequent Comfort and private activity
remain usable. No toy reward is fabricated when food interrupts private play. These are new
native semantic cases, not matched historical before/after footage.

The reduced-effects route uses the actual settings actions, with all three switches visibly On
before and after play. Its 48 per-simulation creature states through 47 seconds exactly match
the normal seed-12 direct-toy run. Ball, bell and sock outcomes, separate private-ball ownership,
continuous pickup/release and subsequent speech/Comfort remain intact. Speech has correctly
owned open and closed mouth phases; this particular route has no simultaneous physical/speech
overlap and does not independently prove ducking. Both supplemental reference mixes have zero
clipping. Reports include `...-causality-food-reduced-final9.md` and the matching audio/visual
case reports.

### Final recording inventory

All twelve C9 bundles completed successfully, with **76,991 frames**. Binary/scenario identity,
artifact byte counts and every recorded artifact hash validate. The first ten form the complete
baseline plus supplemental pass; the last two are fresh repetitions after the final source
change. [The durable evidence index](style-reference/ui-20261001/evidence.json) retains target,
manifest, video and trace hashes, seeds, reference-audio limits and the final source audit.
Raw bundles use `/tmp/beastie-ui-20261001-after-feel-<label>-v9/<experience>`.

| Label | Experience | Frames | Recorded timeline | Capture wall time |
| --- | --- | ---: | ---: | ---: |
| first | first-five-minutes | 18,021 | 300.333 s | 469.094 s |
| quiet | quiet-observation | 10,812 | 180.183 s | 289.603 s |
| chain | interaction-chain | 3,998 | 66.616 s | 109.506 s |
| dialogue | dialogue-races | 2,179 | 36.300 s | 65.524 s |
| bad | bad-conditions | 2,603 | 43.366 s | 73.955 s |
| relationship | relationship-over-time | 5,850 | 97.483 s | 158.521 s |
| relationship-no-ai | relationship-over-time-no-ai | 3,255 | 54.233 s | 96.082 s |
| direct | direct-toy-contact | 2,894 | 48.216 s | 93.775 s |
| food | food-refusal | 2,894 | 48.216 s | 80.221 s |
| reduced | reduced-effects | 3,568 | 59.450 s | 97.470 s |
| first-repeat | first-five-minutes | 18,023 | 300.366 s | 517.923 s |
| direct-repeat | direct-toy-contact | 2,894 | 48.216 s | 85.194 s |

The fresh five-minute repeat preserves all 304 creature state transitions and semantic event
batches from the first C9 run, including clear late-sock contact and continuous transfer/release.
Two extra presentation frames account for a 33–34 ms capture offset; simulation timing is
unchanged. The fresh direct repeat matches every creature frame, all 52 distinct transitions
and the exact contact timestamps. It ends with Ready handoff and no carried toy.

Across the twelve bundles, all 26 physical toy responses have exactly one response-specific
source and matching direct/private owner. All 552 speaking frames have the correct playback
owner, and no nonzero mouth phase occurs outside speaking. All reconstructed PCM scans have
zero clipping or saturation. These totals include independent repeated bundles; they do not
claim physical speaker audition. Consolidated reports are `...-causality-repeats-final9.md`
and `...-audio-final9-summary.md`.

Final visual review covers all 135 filmstrips and 27 original captures, plus the selected exact
sequences described per case. Eleven unique movies decode completely; the fresh direct repeat's
movie, state trace, filmstrips and originals are SHA-256 identical to its accepted first run.
The fresh five-minute repeat independently closes the changed late-sock hold and transfer,
clear Bottom arrival and tank-boundary body continuity. Reduced-effects near-freeze detections
are intentional settled periods with later visible changes, not missing frames. The final
visual disposition is `...-visual-completion-final9.md`; no further fix or capture is requested.

## Excluded attempts and evidence limits

| Attempt or limit | Disposition |
| --- | --- |
| Four baseline launches missed the renderer's 60-second readiness deadline | No usable frames. Invalid evidence, not UI findings. A scratch-instrumented build reached readiness; the untouched baseline then succeeded. Cause remains unproven; no production renderer fix was made for it. |
| Initial first-five recording stopped at 197.9 s playback after its 360 s wall deadline | Incomplete `...-before-feel` evidence. Replaced by the validated `...-before-feel-v2` run in the table. |
| Baseline chain v2 is black at 42.867–66.617 s | Invalid `...-before-feel-chain-v2`; replaced by visible-window `...-before-feel-chain-v3`. |
| C3 “populated” export contains zero records | Valid empty-export presentation only. Superseded for populated-export semantics by the inspected C4 one-record files. |
| Rapid fullscreen captures contain blank transition frames | Rejected; replaced by C4 settled Normal/Large size batches. |
| A CUA app lookup raced the completed C6 no-AI run and launched an ordinary title window | Stopped only the new PID. It created a fresh elapsed-zero save with sequence zero and no prior save/backup; that file was moved to `...-accidental-title-launch`, restoring the absent save path. Recording files were unaffected. Subsequent app lookup requires a fresh live scripted-process check. |
| Presentation fixtures supply synthetic microphone, speech and selection state | Layout evidence only. They cannot certify microphone, recognition, model, speaker or shortcut behavior. |
| Corrupt/unreadable startup recovery was not launched natively | Focused recovery tests and shared-notice native layout are covered. Scripted native runs intentionally start fresh instead of reading the normal save path, so they do not prove this startup file route. Actual in-game reset/recovery was verified separately. |
| Motion files use fixed 60 fps output and reconstructed reference audio | No continuous full-speed subjective playback, physical speaker audition, host-output capture or real-time host-performance claim is made. |

The host is an Apple M2 Pro, 12 logical CPUs, 32 GB memory, using Metal. Initial load exceeded
30 with substantial compression; an unbounded build was stopped and bounded two-job builds
used thereafter. The baseline build took about 59 minutes. A sampled compiler was doing normal
monomorphization, not demonstrably deadlocked. Capture wall times above deliberately remain
separate from authored playback times.

The first-five baseline requested TTS but produced no WAV because the external voice executable
was absent. It proves text-only fallback, not voice or mouth sync. eSpeak NG 1.52.0 and
pcaudiolib 1.3 were then installed through Homebrew; offline 22,050 Hz mono PCM synthesis passed.
The matched C6, C7, C8 and C9 first-five runs deliberately use an unavailable voice executable, retaining
the same text-only condition. Shorter one-shot completion intervals on the slow capture timeline limit reference
envelope judgments; they do not prove host sound truncation. Physical microphone, gamepad and
speaker behavior remain unverified. The controller route was exercised through semantic
setup and native pointer clicks, not a physical gamepad.

The ordinary native profile is `dev-perf`, retaining assertions and overflow checks. The host
uses Homebrew Rust/Clippy 1.98.1 while the repository requests 1.97.1; rustup is absent. Results
are not pinned-toolchain or cross-platform verification.

## Verification and final evidence

C9 `cargo xtask verify` passed **735 tests, zero failures and seven explicitly ignored native-GPU
benchmarks/oracles**, plus formatting, strict workspace Clippy, asset/model contracts, dialogue
fixtures 27/27, STT fixtures 11/11 with WER 0, spoken-input replay and the normal workspace build.
The complete log is `...-verify-candidate9.log`; `...-verify-candidate9-summary.json` records totals.
The exact final source also passed `cargo xtask dev --fake-ai --smoke` on M2 Pro/Metal.

The staged whitespace check reports one original trailing space at line 21 of the upstream
`assets/licenses/noto-emoji-OFL.txt`, retained verbatim. All other staged files pass that check.

C5 `cargo xtask verify` passed 717 tests, with seven explicit native-GPU tests ignored, plus
Clippy, fixtures, replays and the normal build. Its log is `...-verify-candidate5.log`.
Normal `cargo xtask dev --fake-ai` smoke passed on M2 Pro/Metal. The linker reported a
large debug unwind-table warning; the build completed. This gate predates C6 pickup transfer.

Earlier gate failures were corrected: two test imports, new-toolchain constant-slice/Copy lints,
an outdated keyboard navigation expectation, and center-contact/two-second-arrival fixture
assumptions. Fake-shell success cases still timed out under a single-thread low-load retry,
so their startup allowance increased from 40 ms to two seconds; the intentionally stalled
case retains 40 ms. Production inference deadlines and failure assertions were not relaxed.

Independent general, layout/accessibility, recovery, typography, behavior and direct-toy
producer/presentation reviews completed. The offline-return regression found in the latter
was fixed and re-reviewed. Reports are `...-final-source-review.md`,
`...-typography-final-review.md`, `...-final-behavior-code-review.md`,
`...-direct-toy-core-review.md` and `...-direct-toy-view-review.md`.
These reviews did not execute builds or native checks. C5 tests cover exact physical ownership,
saved continuation, deferred typed/spoken handoff, sock release and offline return; their final
gate and native adjudication belong in the completion record below.

| Final acceptance item | Completion record |
| --- | --- |
| Frozen C9 source and executable | Independent source review closed; `...-candidate9/provenance.json` and content hashes retain the final executable/source identity |
| Final `cargo xtask verify` and native smoke | 735 passed, zero failures, seven explicit GPU exclusions; normal final `dev --fake-ai --smoke` passed on M2 Pro/Metal |
| C9 matched motion and supplemental suites | All seven baseline experiences, seed-12 direct toys, actual food refusal and reduced-effects routes accepted; 56,074 first-pass frames |
| Two valid final native review passes | Complete ten-case pass followed by fresh first-five and direct-toy repeats after the final neighbor-contact correction; all three review lenses closed without another material finding |
| Final evidence audit | All 21 targets and UI families mapped; six original native examples and twelve motion bundles retained; 423 artifact entries verified, with source identity and hardware/toolchain limits explicit |
| Completion | Implementation, generated targets, native examples, review, contract and refreshed STATE ship together on main; the exact commit and push result are reported in the session handoff |
