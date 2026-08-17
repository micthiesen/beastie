# Interaction continuity rework

Date: 2026-08-17
Status: implemented and verified

## Outcome

Every visible or audible beat must retain one truthful owner from player input through
acknowledgement, movement, outcome, and recovery. The same particular fish must remain visually
continuous throughout that beat.

The implementation is complete only when all of these are true:

1. A rejected toy cannot acquire an accepted-play outcome merely because Mop reaches it.
2. Toy history and reward occur at the authoritative physical payoff, not before contact.
3. A superseded dialogue turn cannot later create caption, TTS, mouth motion, transcript
   presentation, or status residue.
4. Speech receipt and speech response do not fire the same full-level cue twice.
5. Delight, affection, and comfort use distinct acting and preserve the canonical fish identity on
   every frame.
6. The native evidence loop detects zero-frame startup promptly and either recovers with a traced
   fresh attempt or fails with useful diagnostics.

This is a structural continuity pass, not a set of timing tweaks. It may refactor the relevant
action, destination, worker, presentation, asset, save, fixture, and feel-tooling seams when that
produces a cleaner final design. Preserve the simulation-first and offline boundaries.

## Product rules

- Proximity is not consent. Reaching a toy does not decide whether it was accepted.
- Input acknowledgement, physical contact, resolved outcome, and recovery are different moments.
- A single semantic event must not mean both "the play request was accepted" and "toy contact
  happened."
- Direct player action may supersede conversation. When it does, all products of that turn are
  invalid together.
- TTS never becomes an independent narrator. It belongs to a current creature turn.
- Subtitles may be disabled, but the internal speech owner must still exist and remain current.
- Perceptual acknowledgement and generated response are separate audio roles.
- Emotional intensity may change pose, never species, base palette, anatomy, or scale.
- Failed capture infrastructure stays technical, bounded, and separate from creature behavior.

## Architecture

```text
player toy input
    -> simulation receipt
    -> typed travel purpose
    -> physical arrival
    -> exactly one resolved outcome
    -> history / reward / presentation

player language input
    -> dialogue turn owner (shell/session generation + dialogue request id)
    -> optional caption
    -> optional independently numbered TTS request mapped to that turn
    -> audible playback + mouth owned by that turn

direct superseding input
    -> invalidate turn owner
    -> discard pending dialogue/TTS completion
    -> clear caption/mouth/playback
```

Core owns toy acceptance, travel purpose, contact, outcome, memory, and relationship changes.
Session owns semantic dialogue validity. The game shell owns asynchronous request generations and
playback lifetime. View projects typed events and owned state. Assets provide the authored body but
cannot alter semantic meaning.

## Toy interaction contract

### Separate receipt from payoff

Replace the overloaded `ToyPlayed` start behavior with an explicit lifecycle. Names may differ,
but the semantic distinctions are required:

```rust
enum ToyInteractionOutcome {
    Accepted,
    Rejected,
    Interrupted,
}

enum GameEvent {
    ToyPlayAccepted { toy: ToyId, interaction_id: NonZeroU64, origin: ToyOrigin },
    ToyRejected { toy: ToyId, interaction_id: NonZeroU64, origin: ToyOrigin },
    ToyContacted { toy: ToyId, interaction_id: NonZeroU64, origin: ToyOrigin },
    ToyPlayed { toy: ToyId, interaction_id: NonZeroU64, origin: ToyOrigin },
    ToyInteractionInterrupted { toy: ToyId, interaction_id: NonZeroU64, origin: ToyOrigin },
}
```

`ToyPlayAccepted` is immediate receipt. It may orient gaze and begin purposeful movement, but it
does not play the impact cue, lower curiosity, improve the relationship, record `PlayedWith`, or
count as a genuine toy visit. `ToyContacted` or the final `ToyPlayed` event occurs only when the
accepted interaction reaches its physical target. That payoff performs those mutations exactly
once.

Rejected input emits `ToyRejected` and the authored take-away/refusal act immediately. It may gaze
at the exact toy and may use a short authored recoil or stare target, but it must not install a
generic toy destination that can resolve through idle arrival logic.

Persist one authoritative `ToyInteraction { id, toy, origin, phase, relationship_context }`.
`origin` is `Player` or `Autonomous`; phases are receipt, approach, contact, resolved, recovery, or
interrupted. Legal transitions are monotonic. Contact and payoff each occur at most once. An
interruption before contact removes the interaction without payoff; an interruption during
recovery cannot relabel or repeat an already completed payoff. IDs are nonzero, allocated
monotonically, and never reused within a save lineage.

For accepted direct play, select any exact shared-toy relationship context at receipt against the
pre-payoff state, persist its motif, subject, evidence, mode, and interaction ID, and begin it only
at matching contact. Do not reselect after approach, even if unrelated history becomes eligible.

### Give travel a reason

`AquariumCreatureState.destination: Option<SemanticDestination>` is insufficient because arrival
cannot distinguish idle curiosity, an accepted direct action, refusal staging, relationship travel,
or an initiative. Replace it with, or pair it with, a typed travel owner:

```rust
struct TravelTarget {
    destination: SemanticDestination,
    purpose: TravelPurpose,
}

enum TravelPurpose {
    IdleVisit { visit_id: NonZeroU64 },
    ToyInteraction { interaction_id: NonZeroU64 },
    RefusalStare { interaction_id: NonZeroU64 },
    FoodAction { action_id: NonZeroU64, food_object_id: u64 },
    CursorSocial { action_id: NonZeroU64 },
    Relationship { beat_id: NonZeroU64 },
    Initiative { initiative_id: NonZeroU64, requested_at_ms: u64 },
}
```

Do not infer purpose later from `current_intention`. Intention can change or be interrupted while
travel remains. The destination owner must be explicit and validated when it is created.

Arrival dispatches on `TravelPurpose`:

- `IdleVisit` records genuine arrival, favorite-place evidence, routine evidence, and a bounded
  settlement. An idle toy visit may resolve autonomous play once.
- `ToyInteraction` verifies the stored interaction ID and toy, then resolves physical play once.
- `RefusalStare` ends or renews only that refusal without positive evidence or payoff.
- `FoodAction` retains the existing exact action-ID, food-object-ID, semantic-food, and outcome
  invariants; it is never dispatched through idle arrival.
- `CursorSocial` owns direct approach/flee/comfort travel and cannot become an idle visit.
- `Relationship` advances only the exact standalone beat, never any matching motif.
- `Initiative` satisfies or clears only the exact request that installed it.

Every destination creation, replacement, interruption, and clear site must install or remove its
matching purpose. If food/action and cursor paths remain separately represented, their owner must
be equally typed and validated and the narrower invariant must be documented. No generic,
unowned destination may remain.

The old `record_genuine_arrival` must no longer handle every semantic destination indiscriminately.
It becomes the `IdleVisit` branch or an equally narrow helper.

### Interruption and replacement

- A new direct toy request explicitly ends the prior toy interaction before establishing another.
- Food action, sleep, attended speech, or another direct action may interrupt a pending accepted toy
  before contact. An interrupted interaction records no play history or reward.
- Repeating a rejected toy renews the refusal presentation without manufacturing a positive visit.
- Each valid repeated player offer records exactly one rejection memory. Holding, arrival ticks,
  and presentation refreshes record none.
- Autonomous play uses the same contact/payoff lifecycle, but its origin is distinguishable in
  events, persisted state, and traces.

### Mutation table

| Moment | Player accepted | Player rejected | Autonomous |
|---|---|---|---|
| Receipt | attempt count and visual receipt only | one offer/rejection memory and refusal receipt | no player attempt or relationship mutation |
| Contact | exact stored relationship context becomes eligible; no duplicate receipt | impossible | genuine visit, favorite/routine evidence, curiosity change |
| Payoff | one `PlayedWith`, curiosity delta, bond/trust/resentment delta, impact/delight | impossible | one autonomous play presentation; no player relationship reward or `PlayedWith` social memory |
| Interrupted | no contact/payoff mutation | refusal may finish normally | no payoff unless contact already completed |

Keep existing numeric deltas unless native evidence or focused balance tests justify changing them.
Accepted context must not change acceptance or the authoritative deltas.

### Save migration and validation

Persist travel purpose and any pending toy interaction because a save may occur mid-approach. Bump
both the core save version and `SESSION_SAVE_VERSION`: production session saves embed the core
world and therefore require their own explicit migration path.

Migration must classify old destinations conservatively:

- an old toy destination with `Intention::Play` is ambiguous because old direct play may already
  have applied eager canonical mutations. Clear it to idle hover while preserving those mutations;
- an old toy destination with `Intention::RefuseAndStare` becomes `RefusalStare`, never accepted
  play;
- an ambiguous old destination becomes an idle hover with the destination cleared rather than
  inventing an outcome;
- existing relationship history, exact food action context, preferences, routines, and visit
  evidence remain intact.

Validation rejects mismatched toy IDs, impossible purpose/intention combinations, a pending
interaction without its target, reused zero IDs, and resolved interactions that remain installed.
Initialize allocators above every migrated live or historical ID. Test standalone core v1 through
v5 fixtures and production session v2/v3 fixtures, preserving exact relationship and food owners.

Pending toy interactions advance deterministically during offline progression exactly once. Their
canonical contact/payoff mutations may occur offline, but compacted resume events must suppress
hours-old impact, delight, and receipt presentation. Persist the final resolved state and retain an
ID-tagged compact outcome for validation. Test mid-approach save with zero and nonzero offline
advance, including repeated resume.

## Dialogue and TTS ownership contract

### One turn identity

Use a composite owner of shell/session generation plus dialogue `request_id`. Increment the
generation on load, reset, session replacement, and supersession so loading an older save cannot
make an old request current. Dialogue and TTS request IDs remain independent counters: store an
explicit mapping from the composite dialogue owner to the independently assigned TTS request ID.
`TtsManager::request` returns that accepted ID or `None`, not a boolean, and mouth/pending ownership
is installed only after successful enqueue. The current turn owner must be carried through:

- `DialogueManager` pending work and returned `DialogueTurn`;
- session acceptance;
- caption/reveal state;
- TTS request and completion;
- active speech player;
- mouth animation;
- feel trace.

Do not discard a completion's request ID before checking it. `SpeechReveal` and `SpeechAnimation`
must retain the owning dialogue request ID. The audio trace must record the speech owner on request,
start, stop, completion, and cancellation.

### Supersession

Define one `supersede_dialogue_turn` operation in the game shell. The direct commands already
identified by `clears_speech` call it before applying their new semantic action. It must:

1. invalidate the active dialogue generation;
2. tell `DialogueManager` that the current request is no longer presentable;
3. invalidate any queued TTS request for that dialogue owner;
4. stop active speech for that owner;
5. clear caption, reveal, mouth timing, and animation for that owner;
6. clear pending UI only if it belongs to that owner;
7. retain unrelated ambience, UI audio, physical effects, and canonical dialogue history already
   accepted before the interruption.

Manager cancellation must be bounded and reusable. A canceled slow request cannot keep the UI
pending or block the next talk until its original timeout. Use a per-request generation or cancel
token checked by the worker exchange rather than the manager's process-lifetime shutdown flag.
Late replies and WAV completions are drained and discarded by owner.

A dialogue reply superseded before semantic acceptance never calls `accept_dialogue_turn` and
adds no dialogue history or transcript. A reply accepted before a later direct action keeps its
canonical semantic history/transcript, while its still-live caption, TTS, mouth, and turn-scoped
status presentation are canceled. Fallback status carries the same turn owner; worker, microphone,
and device diagnostics do not, and must survive unrelated turn cancellation.

Per-request fallback/retry/duplicate-suppression metadata travels with its completion rather than
being read from mutable manager-wide atomics. A rejected TTS enqueue or `wav = None` clears only
that turn's pending TTS and mouth owner while leaving caption-only dialogue visible; it cannot
attach old mouth timing to the next completion.

Session-level relationship invalidation remains authoritative for relationship turns. Ordinary
turns additionally honor explicit shell supersession. Never weaken the exact relationship motif,
subject, evidence, mode, or action-ID checks already implemented.

### Caption and subtitles

With subtitles enabled, a matching nonempty caption must exist no later than speech start and
persist through the full audible duration plus the existing bounded release. There must be zero
`speaking && !speech_present` frames.

With subtitles disabled, audible speech may have no visible text, but trace state must still expose
the current dialogue owner and `subtitles_enabled = false`. Acceptance tests must not mistake this
intentional mode for orphan speech.

Voice-disabled and TTS-failed turns remain caption-only and create no mouth or speaking owner.

## Speech audio recipe

- `SpeechPerceived(Glanced | Attended)` keeps one restrained `creature/curious` receipt.
- `TalkAccepted` remains a visual acknowledgement and pending-state transition, but does not play a
  second identical full-level curious cue.
- Typed talk retains immediate visual acknowledgement. If later native review proves that typed
  input needs sound, use a quiet differentiated receipt with an explicit input-channel event rather
  than restoring the duplicated spoken cue.
- TTS owns the creature-voice ceiling while active. Existing ambience and creature duck envelopes
  remain unchanged.
- No response chirp may overlap TTS unless the overlap is an intentionally authored two-part vocal
  phrase and has a dedicated fixture.

## Positive reaction art contract

Replace all four frames of:

- a new `assets/generated/creature-v1/reaction/delight-south-*.png` clip;
- `assets/generated/creature-v1/reaction/affection-south-*.png`;
- `assets/generated/creature-v1/reaction/comfort-south-*.png`.

Use the canonical fish base and stable mood/action frames as primary references. The current brown
reaction frames are negative references only and must not be used as identity anchors. Generation,
manual pixel curation, or a hybrid is acceptable. Final selection matters more than frame variety.

Every selected frame must preserve:

- gold/olive fish palette and dark plum outline;
- fish body mass and taper;
- tail and fin anatomy, with no paws, arms, curled mammal tail, or upright torso;
- eye placement and characteristic head filament;
- canonical 80x80 canvas, hard alpha, nearest-neighbor pixels, and stable scale;
- opaque bounds and centroid within the promotion thresholds below so entry and exit do not jump.

Delight should be bright, alert pleasure at accepted toy or food play. Affection should be a warm,
slightly sleepy lean toward the player. Comfort should read as notice, soften, lean, and settle.
Map `PresentationCueKind::Delight`, `Affection`, and `Comfort` to their own clips. Keep the existing
heart effect separate from the body. It is valid to repeat a strong frame and let deterministic
buoyancy provide life when generated in-betweens drift.

Update `assets/manifest.toml` with exact provenance, references, rejected variants, curation, and
measured palette ceilings. Do not claim exact identity in provenance unless the runtime contact
sheet proves it.

### Identity review fixture

Add a deterministic render/feel fixture that shows, at native 2x scale:

```text
canonical south idle -> delight frames 0..3 -> canonical south idle
canonical south idle -> affection frames 0..3 -> canonical south idle
canonical south idle -> comfort frames 0..3 -> canonical south idle
```

The asset gate continues checking dimensions, hard alpha, palette, and completeness. Record the
approved canonical south reference's opaque bounds and centroid, then require every promoted frame
to keep opaque width and height within 85 to 115 percent of that reference, each centroid axis
within 6 source pixels of the reference, and consecutive-frame centroid movement within 3 source
pixels per axis. Any intentionally wider pose needs an asset-specific manifest exception with a
reviewed contact sheet. These are diagnostics for gross scale and anchor drift, not a claim that
geometry can judge species. The native contact sheet and full transition remain the semantic
identity authority.

## Feel-runner reliability

The runner currently applies one timeout equal to authored duration plus 60 seconds. A process that
never writes frame zero therefore wastes the full experience duration and produces no diagnosis.

Add a separate startup watchdog:

- have the recorder atomically flush a dedicated first-frame heartbeat only after `record_frame`
  has successfully written both the framebuffer to FFmpeg and the corresponding state record;
- allow a bounded startup window, initially 10 seconds on macOS and configurable for diagnostics;
- poll child exit concurrently. A process exit, panic, scenario error, or recorder error before
  frame one is `early_exit` or its concrete error and is never retried;
- only if the child is still live after the deadline with no completed-frame heartbeat, capture
  process status and a short macOS `sample` when available, kill it, close the invalid MP4, and
  classify the attempt as `zero_frame_startup`;
- retry that class in a fresh process up to two times by default;
- never retry an authored scenario error, validation error, state error, or early-but-nonzero video
  as if it were a drawable stall;
- retain each failed attempt under the requested output root and put its reason in a top-level
  attempts manifest;
- hash and accept only the successful attempt. If all attempts fail, return nonzero with paths to
  every diagnostic.

The watchdog is tooling, not proof that Metal is fixed. A partial attempt never becomes feel
evidence. Preserve the existing `--game` exact-binary path and normal debug default.

## Deterministic fixtures and tests

### Core and session

- Direct accepted toy: receipt is immediate, arrival produces exactly one contact/payoff, one
  memory, one curiosity delta, and one relationship delta.
- Accepted toy interrupted before contact, at the contact boundary, and during recovery: no
  missing, duplicated, or relabeled payoff; pre-contact interruption has no memory or reward.
- Direct rejected toy through and beyond arrival distance: no `ToyPlayed`, impact, delight, positive
  history, favorite, routine, or relationship reward.
- Repeated rejected toy: each valid offer adds exactly one rejection memory; arrival ticks and
  presentation refreshes add none, and positive state remains unchanged.
- Autonomous toy visit: genuine idle arrival still produces one physical play payoff and retains
  prior quiet-life behavior.
- Save/reload at accepted approach and refusal stare preserves exact purpose and deterministic
  outcome. Nonzero offline advance resolves once without replaying stale presentation. Legacy core
  and session saves migrate safely.
- Cross-product validation rejects mismatched purpose, destination, toy, interaction ID, and
  relationship owner.
- Exact toy relationship context selected at receipt survives unrelated eligibility changes,
  interruption, save/reload, and contact without reselection.

### Dialogue, TTS, and view

- Slow dialogue superseded by play before reply: late reply is discarded, pending clears promptly,
  another talk can be accepted, and no semantic history/transcript is added.
- Reply accepted, TTS pending, then play: late WAV is discarded and never starts.
- Reply already accepted, then superseded: canonical history/transcript remains, but no new or
  continued presentation occurs after supersession.
- TTS active, then direct action: only the matching speech owner stops; unrelated audio survives.
- TTS enqueue rejection and `wav = None`: caption remains, pending mouth/TTS clears, and the next
  completion cannot inherit stale timing.
- Turn-owned fallback status clears on supersession; global worker/microphone diagnostics survive.
- Subtitles on: matching nonempty caption begins by speech start and lasts through audible speech
  plus bounded release.
- Subtitles off: speech remains valid and traced with the owner and disabled-subtitle state.
- Voice-disabled and TTS-failed turns are caption-only with no speaking/mouth owner.
- Spoken turn: one curious receipt, no second identical cue on `TalkAccepted`, then TTS.
- Typed turn: immediate visible receipt remains even with voice unavailable.
- Relationship dialogue supersession continues rejecting stale motif/action replies.

### Assets and presentation

- Render tests assert the new delight, affection, and comfort IDs, frame count, stable source size, and
  cue-relative frame-zero start.
- Contact-sheet review covers all twelve reaction frames and all three enter/exit transitions.
- Same-seed quiet, interaction, relationship, and AI-off relationship footage preserves the
  approved fish identity on every positive-cue frame.
- Rejection footage never switches to delight unless a genuinely new accepted interaction begins.

### Feel evidence

Update the interaction scenario so markers bracket:

- accepted toy receipt, travel, contact, and recovery;
- first and repeated rejected bell through the old false-arrival timestamp;
- talk followed by a superseding direct action before reply;
- talk followed by supersession after caption but before TTS;
- active TTS interrupted by a direct action;
- typed and spoken receipt audio.

The scenario may use explicit fixture delays to make each race deterministic. Marker validators
must assert contemporaneous interaction ID, travel purpose, dialogue owner, speech owner, caption
owner, and outcome, not marker names alone.

## Implementation slices

### 1. Truthful toy lifecycle

Introduce typed travel purpose and an exact toy interaction identity. Split receipt from contact,
move reward/history to the payoff, migrate saves, and add exhaustive core/session tests.

### 2. Cancelable language presentation

Carry request ownership across dialogue, caption, TTS, mouth, playback, and trace. Implement one
supersession operation and manager-level per-request invalidation. Add deterministic race tests.

### 3. Audio simplification

Remove the duplicate `TalkAccepted` curious cue, preserve spoken perception receipt and typed
visual acknowledgement, and verify reference-mix spacing.

### 4. Creature-identity art

Create, curate, validate, and integrate distinct delight, affection, and comfort frames. Review the
full contact sheet and native transitions before accepting them.

### 5. Fast-failing native evidence

Add first-frame watchdog, attempt classification, bounded retry, and retained diagnostics. Capture
valid bad-conditions and familiar-place bundles before final adjudication.

### 6. Integrated native comparison

Replay the exact before scenarios at normal speed. Inspect full footage before traces, compare
event-aligned filmstrips, and keep only changes that improve continuity without weakening direct
receipts, relationship causality, quiet life, or graceful degradation.

## Acceptance

### Causality

- Every toy event has one interaction identity and unambiguous lifecycle meaning.
- Rejection cannot create play, reward, history, favorite, routine, impact, or delight.
- Accepted play resolves once at physical contact and survives save/reload deterministically.
- Existing food and relationship exact-subject invariants remain green.

### Language continuity

- Superseded dialogue and TTS perform no new or continued presentation after supersession.
- Pending cancellation is prompt and does not block the next valid talk.
- Caption, TTS, mouth, transcript presentation, and trace agree on the same owner.
- Spoken acknowledgement occurs once before response; typed acknowledgement remains immediate.

### Creature identity

- Delight, affection, and comfort remain unmistakably the same gold fish in every frame at native
  2x scale.
- Onset, frame progression, held final pose, and exit contain no palette, anatomy, scale, or
  centroid snap.
- The reactions remain emotionally distinct without relying only on the heart effect.

### Evidence and safety

- `interaction-chain`, `bad-conditions`, `quiet-observation`, `relationship-over-time`,
  `relationship-over-time-no-ai`, and all five `relationship-breadth` experiences produce valid
  synchronized bundles from normal debug builds.
- Zero-frame startup fails within the startup bound, retains diagnostics, and never produces a
  manifest that looks valid.
- Same-seed before/after evidence closes every finding in
  [feel-review-interaction-continuity.md](feel-review-interaction-continuity.md).
- `cargo xtask verify` passes.
- No release or final build is required for iteration or acceptance.

## Implementation ledger

During implementation, append every material improvement that falls out of this contract before
making it. For each entry record:

- discovery and evidence;
- why it belongs to interaction continuity;
- affected authoritative or presentation owner;
- test and native acceptance evidence;
- final disposition.

The mandate includes cohesive improvements discovered while implementing or reviewing these
changes. It does not include unrelated new features.

### 2026-08-17: reaction identity source selection

- Discovery and evidence: a fresh image-generation attempt preserved the broad fish concept but
  changed eye scale, body mass, shading, and pixel density. The already approved play, notice, and
  content frames remain exact runtime identity anchors and express the required emotional arc.
- Why it belongs: accepting another attractive near-match would repeat the species-continuity
  failure this pass exists to remove.
- Owner: authored creature body presentation.
- Test and native evidence: asset palette/alpha checks, explicit bounds/centroid diagnostics,
  twelve-frame contact sheet, and the quiet/interaction/relationship native replays.
- Disposition: curate distinct delight, affection, and comfort clips from the pinned canonical
  runtime frames; retain the generated attempt only as a rejected provenance note.

### 2026-08-17: toy travel owner conflicts discovered in code review

- Discovery and evidence: standalone relationship scheduling could replace an active accepted-toy
  travel target without ending the toy interaction. Autonomous toy travel also allocated its
  interaction identity only on arrival, leaving mid-approach saves without an exact owner.
- Why it belongs: both paths violate the same one-owner continuity rule and can orphan or recreate
  a physical payoff.
- Owner: authoritative simulation travel and toy interaction state.
- Test and native evidence: focused relationship-preemption, autonomous mid-approach save/reload,
  validation, and same-seed quiet/interaction replay.
- Disposition: block standalone relationship beats while any toy interaction is active; create and
  persist autonomous interaction identity at travel start; reject approach state without a matching
  travel owner.

### 2026-08-17: cancellation boundary and evidence integrity gaps

- Discovery and evidence: the direct `React` UI path cleared visible speech without invalidating
  pending dialogue. Runner timeout cleanup killed only the game PID, not its worker descendants.
  Feel state also lacked subtitle mode and caption/mouth owners needed to prove speech continuity.
- Why it belongs: late replies, surviving workers, and incomplete ownership traces can each make a
  superseded turn appear current or contaminate the evidence used to judge it.
- Owner: game-shell dialogue generation, feel process lifecycle, and feel trace schema.
- Test and native evidence: late-reply-after-react regression, descendant cleanup test, owner-rich
  rapid-interruption trace, and valid subtitle-on/off captures.
- Disposition: route reactions through full turn supersession; contain and reap the whole game
  process tree on every failed attempt; trace subtitle, dialogue, caption, TTS, speech, and mouth
  owners explicitly.

### 2026-08-17: nested worker process groups

- Discovery and evidence: live native inspection showed the game and FFmpeg in the runner-owned
  process group, but the dialogue and TTS workers intentionally created their own process groups.
  Killing only the game group after a hard startup timeout could therefore bypass worker cleanup.
- Why it belongs: retry isolation is false if workers from a failed attempt can survive into the
  next evidence run.
- Owner: feel-runner Unix process-tree containment.
- Test and native evidence: recursive descendant-group termination test plus live PID/PPID/PGID
  inspection during a retry-producing baseline capture.
- Disposition: enumerate and terminate the full descendant tree, including nested process-group
  leaders, before reaping the game group; keep the Windows Job Object path unchanged.

### 2026-08-17: completed caption lost its trace owner

- Discovery and evidence: final-candidate `first-five-minutes` frames at `00:26.216` onward showed
  a visible caption and active owned speech, but `caption_owner` became null as soon as the
  typewriter reveal completed.
- Why it belongs: the caption remained correctly visible, but the evidence could no longer prove
  that it belonged to the audible turn for the rest of playback.
- Owner: game-shell caption presentation and feel trace projection.
- Test and native evidence: trace assertion across reveal completion and the full audible duration,
  followed by rapid-interruption recapture.
- Disposition: retain or derive the caption owner for as long as the owned caption exists, not only
  while its incremental reveal timer is active.

### 2026-08-17: delight onset reversed current facing

- Discovery and evidence: final relationship footage at `01:07.250` to `01:07.300` approaches the
  ball with the tail on screen-right, then switches in one frame to the fixed delight pose with the
  tail on screen-left. The contact sheet shows the same idle-to-delight reversal.
- Why it belongs: species identity is repaired, but instantaneous facing reversal still breaks the
  physical continuity of the exact contact payoff.
- Owner: authored reaction projection and current-facing presentation.
- Test and native evidence: focused facing-preservation render test, regenerated contact sheet, and
  a fresh accepted-toy contact filmstrip.
- Disposition: preserve or mirror the delight clip from current facing at cue onset without changing
  its semantic owner or timing.

### 2026-08-17: in-flight toy save proof was narrower than the contract

- Discovery and evidence: core tests covered uninterrupted exact shared-toy context and accepted
  offline resume separately, while session migration tests covered only ambiguous legacy play.
  They did not combine unrelated eligibility change plus save/reload before shared-toy contact or
  current session save/reload of both accepted approach and refusal stare.
- Why it belongs: the persisted owner is only trustworthy if every current save layer preserves it
  without reselection or outcome invention.
- Owner: core toy interaction state and production session save migration/resume.
- Test and native evidence: focused current-version core and session round trips asserting exact
  interaction ID, toy, origin, outcome, travel purpose, relationship subject/evidence, and one
  eventual payoff or refusal completion.
- Disposition: add the complete round-trip matrix and make only the minimal implementation repair if
  a test exposes a real gap.

### 2026-08-17: final native dialogue races covered only pending TTS

- Discovery and evidence: the final rapid interruption enqueued and canceled TTS in one frame, and
  the interaction-chain speech completed normally. Focused tests covered slow pre-reply and active
  playback cancellation, but the final native fixtures did not exercise those two boundaries.
- Why it belongs: ownership must remain visible under the asynchronous timings that originally
  produced the defect, not only in pure manager tests.
- Owner: fixture dialogue timing, active speech owner, and feel scenario markers.
- Test and native evidence: a bounded fixture-only reply delay superseded before acceptance, plus a
  normal owned TTS playback interrupted by a direct action with zero later residue.
- Disposition: add deterministic inactive-by-default fixture timing and explicit native markers for
  both boundaries; keep production worker timing unchanged.

### 2026-08-17: first dialogue-delay seam blocked the scenario thread

- Discovery and evidence: the first `dialogue-races` capture requested talk at `00:01.033`, but its
  authored 150 ms comfort did not execute until `00:02.083`; delayed reply and TTS had already
  started at `00:01.933`. The runner rejected the bundle because the superseded owner reached TTS.
- Why it belongs: a timing seam that freezes frame/scenario progress cannot reproduce a real slow
  asynchronous worker and can certify the wrong race ordering.
- Owner: capture-only dialogue completion scheduling.
- Test and native evidence: frames and direct semantic commands continue during the delayed request,
  comfort invalidates the owner before completion, and the late reply produces no acceptance or TTS.
- Disposition: move fixture delay off the update/scenario thread into cancel-aware asynchronous
  completion delivery; retain fail-closed race validation.

## Completion evidence

The rework is implemented across core save version 6, session save version 4, the game-shell
dialogue/TTS boundary, owned view/audio projection, curated reaction art, and the native feel
runner. The final debug-build evidence is:

- `target/feel/interaction-continuity-final-interaction-v2/interaction-chain`: repeated bell refusal
  uses interaction IDs 3 and 4 and never emits `ToyPlayed`; the spoken receipt emits one curious
  cue; its final turn starts owned TTS and a direct comfort stops that exact playback at
  `01:01.333` with no residue.
- `target/feel/interaction-continuity-final-dialogue-races-v4/dialogue-races`: a delayed reply is
  superseded at `00:01.216` before semantic acceptance or TTS; a later subtitles-off turn has an
  audible speech/mouth owner and no caption, then restores subtitles.
- `target/feel/interaction-continuity-final-relationship-v2/relationship-over-time`: the active
  player-return beat is interrupted at `00:60.200`; dialogue turn generation 11/request 3 is
  accepted at `00:60.233`, its TTS request 3 is enqueued and canceled at `00:60.250`, and that WAV
  never starts. The later accepted ball contact preserves incoming facing through delight.
- `target/feel/interaction-continuity-final-breadth-v2`: trusted berry, mushroom grudge, familiar
  cave, familiar plant, and familiar ball all pass exact motif, subject, mode, and evidence
  validation. The final binary also completed quiet observation, bad conditions, and the AI-off
  relationship arc in their respective `interaction-continuity-final-*-v2` roots.
- `target/feel/reaction-identity-contact-sheet.png`: delight, affection, and comfort retain the
  approved gold fish across onset, progression, held pose, and exit while remaining distinct.

Every accepted final bundle uses manifest version 5 and game binary SHA-256
`18cbb2c303a8ddd7fdc7d78b699df7f83b43bcacca21fc99cb9f30e0f44bb1c4`. The full
`cargo xtask verify` gate passes with 78 declared runtime assets. Real zero-frame failures during
final dialogue, bad-conditions, AI-off, and relationship-breadth capture were classified within the
startup bound, retained with diagnostics, and retried without contaminating the accepted bundle.
No descendants remained after those attempts; focused process-tree tests additionally prove nested
worker-group termination. Independent final visual, causality, and dialogue/audio adjudication
found no remaining material continuity issue. Large recordings remain ignored review artifacts
under `target/`; the contract, scenarios, tests, manifests, and tooling are the durable record.

## Non-goals

- New toys, foods, habitats, progression systems, or conversation features.
- A global UI redesign.
- Retuning the already accepted quiet-life destination cadence without new evidence.
- Cloud inference, remote APIs, or model changes.
- Replacing ambience, global duck envelopes, or direct food audio that this pass found clean.
- Windows validation, controller validation, packaging, release, or final-build work.
- Treating automatic silhouette metrics as a substitute for visual identity review.
