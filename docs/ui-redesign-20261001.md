# Beastie UI and feel redesign

> Superseded in part on 2026-10-04 by [the fun rework](fun-rework.md): speech is a compact bubble
> at the creature's head (no continuation pages, no Laugh/Disapprove/Comfort reaction controls),
> care verbs are single clicks, food drops in front of the creature, and world targets stay live
> under shallow menus.

## Authorized outcome

The player spends time with one particular voxel creature in a beautiful, legible aquarium.
Every existing UI surface and interaction belongs to one coherent visual family, guided by
new ImageGen targets rather than incremental decoration of the old interface. The user has
authorized the complete review, implementation, native iteration, required gate, commit and
push. No further design approval checkpoint is required.

Product authority remains [the philosophy](game-design-philosophy.md). The voxel creature,
tank, physical toys, deterministic simulation, offline play, graceful failures and absence
of permanent neglect damage are preserved. This is a redesign of the existing experience,
not a new progression system or conversation product.

## Visual contract

New targets and their complete prompts live in `style-reference/ui-20261001/`. Their UI
direction supersedes the earlier gold UI treatment: smooth restrained corners, dark teal
surfaces, delicate warm rims, readable ivory text, clear mint secondary text, spacious
controls and purposeful hierarchy. The aquarium and creature remain voxel geometry.
Existing outline-based typography and geometry can implement this direction without raster
screenshots or a new runtime dependency. Generated pictures are design references, never
authoritative game state or screenshots of the implementation.

The care dock groups identity, physical care, language and utilities. Settings use horizontal
categories and a full-width content column. Every secondary screen uses the same padding,
surface, typography, focus, disabled and dismissal treatments. Primary actions are clear;
destructive actions are subdued and never the default focus. Large text must actually stay
larger, with layouts sized for it rather than silently shrinking everything back down.

## Complete implementation scope

1. Title, Continue/Settings/Quit, logo treatment, version and title-to-play continuity.
2. Care dock identity and qualitative summary, three toy controls, Feed, text field, Send,
   microphone, Settings/Close, hover/focus/disabled/engaged/pending states.
3. Food chooser, three food placement states and cancel; toy chooser; all three selected
   toy cards; creature care menu; plant/cave/food/open-water inspection; stable placement.
4. Speech, progressive reveal, ownership/interruption, opposite-side placement, long text,
   reactions, subtitles off, technical receipt and graceful missing AI/audio states.
5. Settings Display, Sound and Controls, every option and selected/off/on/cyclic state,
   Normal/Large, six window scales, fullscreen and return navigation.
6. Input bindings, all six capture prompts, conflict/cancel/reverse navigation, defaults.
7. Dedicated rename dialog, valid/empty/long Unicode input, 24-character guidance, keyboard
   and controller parity; complete on-screen keyboard and its message/name states.
8. Save/data, backup recovery, transcript opt-in/export, reset confirmation with Cancel
   default, startup recovery and usable long technical notices.
9. Every microphone state, physical care receipts, accepted/refused food and toys, comfort,
   quiet life, relationship acting, reduced motion/flashes/shake and audio arbitration.
10. Visual quirks found during review, text clipping, target comfort, ambiguous icons and
    misleading instructions. Add newly accepted findings here before implementing them.

Accepted implementation-review additions: panels must block clicks and keyboard activation of
covered controls; speech and reactions must sit beneath modal surfaces; microphone failure notices
must be dismissible without pretending the device recovered; right-side pages reserve free left
space for notices. Name drafts must never become chat drafts when a care action replaces the
dialog. Long valid captions use readable continuation pages with complete Unicode graphemes and
enough reading time. Scripted transcript exports use a separate namespace from their source log.
Reset/recovery cancel displaced recognition work and retain a usable sibling backup even when
the primary restore file cannot be read. Text entry and deletion preserve complete Unicode
graphemes while retaining the canonical scalar-value limits for names and dialogue.
Editable fields retain the end of the draft using measured font widths, with leading ellipsis
when needed; approximate character budgets must not hide newly typed wide glyphs.
Key repeats edit only the focused text field. Holding a key over a focused control must not
change a hidden draft or repeatedly toggle window, audio or accessibility settings.

Native review additions: message and name fields need conventional primary-modifier
Select All and Paste, visible selection, replacement by typing, and selection-aware deletion.
Clipboard results must remain bounded and belong to the field that requested them; a delayed
result must not enter a different mode or submitted draft. Modifier chords must be interpreted
in event order so a quickly released shortcut cannot insert its letter. These are local text
editing operations, never automatic dialogue submission. Close hints must remain useful and
local rather than appearing in an unrelated corner when the nearby modal already labels Close.
The on-screen keyboard must keep the creature's face visible in the standard composition,
using the lower band without shrinking the authored text roles or removing controls.
Inspection cards must size to their short content, preserving readable longest-name/fact
cases without empty slabs covering the aquarium.
The care dock must retain complete finite activity descriptions at Large text. Remove
approximate pre-truncation and give the measured text enough vertical space for two lines;
do not abbreviate away the creature's current action or reduce the selected text size.
Continuation captions must use compact, content-sized pages opposite the creature rather
than a fixed full-width slab. Keep all words, stable bounds during reveal, readable Back/Next
and complete reaction targets. Settings reserves a clear column for speech. Where another
modal leaves no clear column, hide the complete caption/control group and retain reading
time until it can be shown again; never show half a sentence or a partially covered action.
Supported emoji must render through a bundled monochrome outline fallback, including joined
sequences, rather than crossed missing-glyph boxes. Primary interface lettering stays Atkinson;
measurement, wrapping, selection and rendering share the same fallback shaping. This does not
claim support for every writing system or require host fonts, color emoji or runtime downloads.
Data actions must show their newest result immediately. A previous missing-backup or recovery
notice must not conceal transcript opt-in or export feedback, including an empty export.
Toy contact and recovery must leave readable, physically distinct silhouettes. In quiet native
evidence, the bell remains hidden inside the resting head and a released ball remains embedded
in its cheek. Correct the authoritative approach/contact or its faithful projection as the
source evidence requires, preserving actual contact, carried-object meaning and quiet pacing.
Do not hide the toy, invent carrying, or move it arbitrarily only to improve a screenshot.
Carried and released sock anchors must share the same tank-edge clamp, so releasing near
the floor cannot jump upward from a visual mouth offset below the authoritative boundary.
The activity summary must make accepted food/toy attention legible during anticipation and
overlapping recovery. The valid first-five baseline shows food gaze labelled only `watching`
and an accepted ball invitation labelled only `settling down` after eating. Name the actual
current target without claiming contact, enjoyment or movement before it occurs. Preserve
the prior payoff and recovery pose; receipt must not depend on a vanished menu's pressed ring.
Speech articulation must follow actual playback progress, not the simulation's one-second
ticks. The voice-enabled baseline held one mouth phase throughout a short spoken reply;
source confirms this same coarse clock also applies in ordinary play. Preserve speech-owner
checks and immediate interruption while using the active sink's progress for mouth timing.
Timed affection expiry releases only its own Player/CursorSocial comfort journey. A newer
private activity, relationship approach or cursor destination keeps its travel owner and
steering; saving before the deadline preserves that continuation after reload.
An accepted reply with subtitles disabled must leave a visible, usable text focus instead
of focusing hidden reaction controls and swallowing typing. A reply arriving while another
page is open must retain that page and its focus; asynchronous expression does not own navigation.
Native candidate 2 still overallocates vertical space in inspection and continuation speech:
the layout's conservative character estimates disagree with the renderer's actual shaped
word wrapping. Use shared exact font/fallback metrics for content height and page capacity,
keeping a complete page's bounds stable during reveal. Short text must occupy short cards;
wide letters and joined emoji must remain fully visible. Preserve the renderer's existing
shaping and bounded caches rather than introducing separate approximate font rules.
Opening reset confirmation clears the previous operation's transient receipt, so an old
export banner cannot cover the confirmation's lower edge. A new reset failure must still
be reported normally; dismissal does not change reset or backup behavior.
Transient notices must also avoid the currently open interaction panel. Native rename followed
by Feed exposed the name receipt across the food cards. Place notices in the available band
above bottom-anchored selectors, and choose clear top/bottom space for centered dialogs;
retain their visible dismissal and do not let old receipts mask the next choice.
The compact one-action inspection strip must keep its heading whole at Large text size.
Use the concise heading “Look closer” rather than truncating “Spend a moment”.
Accepted direct toy play must commit the same physical object response as autonomous play:
ball impulse, bell strike and owned sock carry/release. The response occurs exactly once at
real surface contact, before a deferred conversation can claim the safe boundary. Rejection
never produces a positive response. Preserve exact interaction identity and saved continuation.
Offline completion must release the finished toy owner before return handling, so it cannot
suppress the creature's eligible player-return recognition or replay stale contact cues.
Vertical ball approaches must visibly meet the ball at payoff without restoring the old
head/object overlap; retain the extra separation for quiet rest.
Physical response cues have the exact toy interaction owner, separate from private-life
counters and from the creature's delight expression. Emit one toy-specific sound and visible
object effect rather than stacking a generic impact over it. Export receipts use singular
“turn” for one record and plural “turns” otherwise.
The sock's pickup transfers visibly from its preceding rendered pose into the mouth over a
short, bounded interval. Contact, ownership and rewards remain authoritative at the original
boundary; only the presented pose converges. Effects and picking follow the same rendered
object. A loaded carried sock starts in the held pose, clock resets cannot replay an old
transfer, and early release must remain continuous. Essential transfer continuity remains
with reduced motion enabled, without an added flourish.
The `interaction-chain` feel suite also includes `direct-toy-contact` with seed 12, whose
deterministic preferences accept all three toys. Inspect each real response and the sock's
carry/release in the continuous recording; the sparse named endpoints alone are not proof.

## Architecture and persistence

Tank-boundary turns must preserve the body's visible continuity. A floor-constrained segment
must not switch between opposite feasible directions in one frame as its desired heading
crosses the downward axis. Keep the articulated chain inside the tank with its existing joint
spacing, select a continuous feasible turn from the prior pose, and retain quiet curvature.
This corrects the inherited tail-side jump found in the five-minute native run; it does not
change authoritative travel, creature intent, rewards or activity timing.

Bottom foraging must finish at a clear spot on the bottom, rather than at a fixed horizontal
coordinate that can be occupied by a moved toy. Select the nearest deterministic clear column
at the existing bottom height using the shared head/toy envelope and quiet-rest clearance.
Ignore carried toys. Preserve other semantic destinations, contact ownership, rewards and
authored activity durations. Require precise arrival at the clear column before the stationary
act; the generic approach tolerance must not permit an early stop inside a nearby toy.
This is destination selection, not general collision avoidance;
brief occlusion during ordinary travel does not establish a stuck or invalid interaction.

A selected toy's contact surface must also account for other uncarried toys. C8's later private
sock play holds the head on the sock's left surface inside a stationary bell for two seconds,
then takes several seconds to clear its forehead. Preserve the existing surface when it is
clear; when it is blocked, choose a deterministic in-tank contact position on the selected toy
that clears neighboring toy envelopes. Keep actual selected-toy contact, carrying, exact owner,
one-time reward and authored Act/Recover durations. Do not move or hide the unrelated toy,
invent a response from it, or turn this bounded destination choice into general pathfinding.
If the bounded search cannot find a clear in-tank contact, interrupt the approach without a
positive payoff or fabricated refusal. Release its travel owner, retain a usable dialogue
handoff and let later valid input work normally. Do not wait forever or reward distant contact.

`beastie-view` owns declarative layout, semantic controls, inspection presentation and state
projection. `beastie-game` executes input, retained geometry/text, local settings and files.
Toy approach, contact and recovery use deterministic surface targets and physical responses;
reward rules, activity durations and save schema stay intact.
New view state is ephemeral and
must have compatible defaults. UI colors and shapes must remain cached; no extra world
lighting rays or continuous geometry rebuilds are justified by this redesign.

All generated pages share one component language. Artistic inconsistencies, illustrative
copy, or impossible settings values in mockups must be reconciled to actual game semantics
and documented. ImageGen does not determine facts or invent capabilities.

## Evidence and completion

Inventory: every surface maps to a generated target, implementation and native evidence.
Keep prompts and selected originals in the repository, large recordings in ignored/scratch
storage. Use fresh disposable scenarios with explicit recorded seeds: 42 for the baseline,
12 for accepted-toy routes and 8 for genuine food refusal. Never reset the player's save.
Compare the same authored scenarios/settings and retain binary/scenario hashes.

Run the full baseline feel suite and broad still coverage, native pointer/keyboard checks,
Normal/Large and viewport reviews. Review full-speed playback, synchronized filmstrips,
semantic traces and reference audio honestly within available tool capabilities. Loaded or
zero-frame runs are invalid evidence; wait and retry, keeping diagnostics. The current host
must be documented rather than inheriting earlier hardware performance claims.

After the last material change, complete two valid native review passes and independent
code/visual/causality/audio review. Fix every accepted material issue, rerun affected evidence,
run `cargo xtask verify` and `cargo xtask dev --fake-ai`, update the final contract, review and
STATE, commit all finished changes and push to main. No release bundle is part of this pass.

The October 2 completion is recorded in the [final review](feel-review-ui-20261001.md): all
interface families are implemented, the ten-case C9 native pass and two fresh repeats are
accepted, and the 735-test headless gate and normal native smoke pass. The review preserves
the specific toolchain, device and subjective-playback limits; it does not infer unperformed
checks from synthetic layout states or fixed-rate recordings.
