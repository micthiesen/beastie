# Voxel craft review

## Scope and baseline

The [craft contract](voxel-craft-pass.md) covers UI, acting, tank depth and reusable art profiles.
The comparison baseline is commit `c1bdf43`, with accepted migration evidence under
`target/feel/voxel-accepted-interaction/interaction-chain`,
`target/feel/voxel-accepted-quiet/quiet-observation` and
`target/captures/voxel-accepted-aquarium`.

## Initial findings and decisions

- **UI hierarchy, session-wide:** migration controls placed tiny icons in the upper-left corner
  of large empty buttons. All panels carried equally heavy brass outlines. The new care deck
  centers and labels controls, separates communication from care, uses subdued surfaces and
  reserves warm accents for selection. Type roles distinguish identity, body, controls and status.
- **Settings density:** the former two-column inventory squeezed fifteen controls into eight rows.
  Three pages now group comfort/display, sound/speech and controls/data. Each has six or fewer
  controls, consistent label/value alignment and a local exit. Large text grows by a deliberate
  proportion rather than doubling every label and subsequently shrinking values.
- **Modal ownership:** initial craft Bindings, Data and Reset captures focused the underlying
  compose field. They now focus their own first relevant action; Reset defaults to Cancel.
  Selecting or reopening settings preserves focus on the selected tab.
- **Technical versus creature voice:** early captures exposed startup settings recovery as speech
  with reaction buttons. Startup notices now distinguish welcome speech from technical recovery
  through a typed notice. Infrastructure reports occupy the status area and retain creature identity.
- **Tank depth:** the initial frontal view hid authored depth. A fixed 12-degree overhead view
  exposes the cave and substrate; UI geometry receives the matching rotation to preserve native
  text and hit alignment. The camera ray still resolves the actual world plane and hit volumes.
- **Acting:** central recipes coordinate eyes, brows, fins, tilt and geometric mouth shapes.
  Supporting gestures ease into an owned expression; semantic facial meaning remains immediate.
  Continuous body-chain motion and authoritative private-life/interaction ownership are preserved.

## Native iteration

`target/captures/craft-ui-pass-01` contains six normal UI panels. Independent review accepted the
new grouping and typography and identified modal focus, missing tray headings, disabled-state
contrast and redundant identity ornament. Those findings were implemented.

`target/captures/craft-ui-pass-02` contains eighteen normal/large panels. The camera tilt exposed
coplanar panel/button surfaces and UI casting shadows into the tank. Panel depth bands now place
buttons above their enclosing surface; UI geometry cannot cast or receive world shadows.
`target/captures/craft-ui-pass-03` recaptures the same eighteen panels after those corrections.

`target/feel/craft-interactions-01` contains promoted interaction-chain and dialogue-races bundles.
Independent visual review compared dense 8–14 second approach/contact frames, 27.4–30.4 second
suspicion/departure frames and 60.4–62.2 second speech interruption frames with the accepted
migration run. No material regression was identified. At 61.316 seconds the speech, caption and
mouth owner clear with comfort. Body length and settled tail continuity remain intact.
Independent timing review found sequential 60 fps captures and causal action/audio ownership,
including deferred reply cancellation and subtitles-off speech.

## Code review

The first independent general, UI and acting reviews found the settings-tab focus reset and
stale lint/test expectations. Those were corrected. Acting review found no defensible ownership,
phase or world-projection failure. Root review found that the full-water food-drop hit region could
claim instruction typography; world targets are now excluded from control text layout and a
regression test covers that case. The startup technical notice regression is covered headlessly.

## Completion evidence

Completed 2026-09-08. Later independent general code reviews found no additional material issue
after the settings-focus correction and final startup, tooltip, text-capacity and scenario changes.
The last caption change replaces obsolete font-spacing constants with the actual Dialogue role
metrics and has a focused headless regression.

`target/feel/craft-baseline-01` contains all seven promoted baseline experiences. Independent
visual reviews covered all overview sheets and dense arrival, cave/wake, bell, ball, relationship
return, comfort and interruption sequences. Independent temporal review covered sequential frames,
causal action ownership, worker failure/deferred responses and AI-disabled behavior. No accepted
material regression remains. Quiet observation retains body length and tail continuity; relationship
acting remains legible without AI. This baseline precedes the final UI-only corrections below.

Actual native testing found redundant hover labels covering neighboring controls, a generic
creature-context "action" label, and oversized empty short-speech panels. These were corrected to
world/reaction-only hints, a "play" label, and typography-derived caption dimensions. Compose
capacity also derives from the body type role so a normal sentence fits at Large.

The two native evidence passes after the final material caption change are:

- `target/captures/craft-ui-shipping`: all 28 Normal/Large UI states, including short speech,
  controller keyboard, rename and creature context. Independent still-frame review checks layout,
  clipping and focus presentation.
- `target/feel/craft-interactions-final/interaction-chain`: 3,758 sequential frames at 60 fps.
  Independent review of all overviews and dense arrival, refusal and speech-to-comfort frames
  found no material visual/acting regression. Food is consumed, play begins, the disliked bell is
  rejected twice, and comfort stops active TTS at 61.333 seconds. The capture completes successfully.

`target/feel/craft-native-complete` records actual macOS pointer and keyboard events over 5,405
sequential frames, with 13 host screenshots. Settings tabs retain focus when Enter follows pointer
selection; bindings, Large text, Reduced motion, food selection/drop, creature picking and F4
comfort were exercised. `host-screens/06-typed.png` shows the complete Large sentence
"Hello Mop, how is the water today?"; `07-submitted.png` shows the reply. Privacy-safe traces redact
characters but independently confirm native key events and submission at 14.833 seconds, food
drop at 18.983 seconds and comfort at 22.466 seconds. `craft-native-typing` separately proves Normal
typing/submission. These actual-input runs precede only the final caption-size correction.

Earlier `craft-native-accepted` and `craft-native-verified` typing attempts lost focus; they are
not typing passes. Per-process character events resolved the host automation issue. Final settings
and backup hashes remained unchanged by scripted testing. Local preference recovery text was also
observed in the status area rather than attributed to the creature.

`cargo xtask verify` passes 460 tests, 27/27 dialogue fixtures, 11/11 recognition fixtures and the
spoken-input replay. Native `cargo xtask dev --fake-ai` and final recordings complete. The debug
linker emits the existing compact-unwind size warning; the final interaction shutdown has only the
benign Bevy unknown-window-destroy warning. No release binaries or installers were rebuilt.

One final gate attempt failed the unchanged worker test
`warm_server_reuses_one_pid_and_receives_flags_auth_and_bounded_payload`: its record contains a
first process start with no health/request record, then a replacement serving both requests.
The isolated regression passes. This supports a transient startup failure rather than failure to
reuse an established server; the precise startup cause was not established. Retain this observation
if the existing fake-server test flakes again instead of attributing it to presentation changes.

Evidence uses macOS debug rendering, screenshots, dense frame samples, synchronized traces and
reference audio timing. It does not claim full-speed human perception, physical speaker listening,
physical controller feel or a Windows/Linux native window run. Reference audio is not host output.
