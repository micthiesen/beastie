# Voxel craft pass

## Outcome

Make time in the aquarium feel like a finished, distinctive game. The creature remains central;
the controls feel deliberately designed, expressions carry meaning without captions, and the
environment has tangible depth. All non-text visuals remain solid geometry. This contract covers
UI, creature acting, tank art, and reusable art definitions together.

## Design

Use a restrained aquarium instrument palette: deep ink surfaces, desaturated teal secondary
surfaces, warm ivory type, brass structural accents and coral for selected or consequential actions.
Give the control deck a clear name/status row and an aligned input/action row. Center icons in
their controls, distinguish disabled controls, and use deliberate type hierarchy rather than
shrinking arbitrary text until it fits. Settings and secondary panels share spacing, surfaces,
control states and typography. Normal and large text must both fit the actual native window.
Keyboard/controller focus and pointer targets follow the same declarative layout.

Keep the fixed camera and authoritative interaction plane. Layer ground, rock, plants and rear
forms in depth; use light to reveal the forms without cluttering open swimming water. Toys must
remain distinct and their displayed positions must agree with picking and semantic contact.

Author coherent acting recipes for curiosity, affection, refusal, sleep and speech. Coordinate
gaze, eyelids, brows, mouth, fins and head pose around authoritative action phases. Anticipation
must not claim a payoff early. Recovery returns cleanly to ongoing life. Reduced motion preserves
meaning while removing decorative oscillation. Preserve the constrained continuous body chain.

Keep palettes, proportions and acting parameters in focused reusable Rust art definitions.
Simulation state, model output, saves, worker protocols and emotional truth remain unchanged.
New presentation helpers belong in view/game, never core. Runtime art needs no external service.

## Sequence and evidence

1. Establish the visual language and UI layout while independently authoring creature and tank art.
2. Integrate the profiles and compare native UI, action and quiet-life evidence against the accepted
   voxel migration captures with the same scenario and seed.
3. Run independent code, visual, causality and timing reviews; record and fix accepted findings.
4. Run the full baseline, exercise native pointer and keyboard input and normal/large text, then
   complete two native review passes after the final material change.
5. Pass `cargo xtask verify`, update this contract, the review record and STATE, commit and push.

The review record must distinguish screenshots and dense frame/trace inspection from full-speed
human perception, speaker listening and physical controller testing. Missing hardware coverage is
an evidence limit, never a fabricated pass. No release packages are required.

## Acceptance

- Every control surface shares a coherent visual language, legible hierarchy and comfortable
  alignment. Text, icons, focus and input targets agree at supported sizes.
- Normal play leaves the creature central; dialogue and technical feedback remain distinguishable.
- Named acting recipes are visibly distinct, truthful, interruptible and accessibility aware.
- Tank depth improves composition without obscuring the creature or interactive objects.
- Art tuning is centralized by domain, with no sprite assets or image-based runtime visuals.
- Existing offline play, optional worker fallback, saves and deterministic behavior remain intact.
- Repeated independent review leaves no accepted material finding unresolved; native evidence and
  the headless gate support the final implementation.

## Implemented result

Completed 2026-09-08. The care deck uses centered geometric icons, enamel surfaces, explicit type
roles, selected/focused states, three settings pages and local modal navigation. Normal and Large
text share measured layout; short speech bubbles use the same typography metrics as their text.
Technical recovery stays in status UI, and labeled controls no longer cover neighbors with tooltips.

`beastie-view/src/ui_art.rs` owns UI palette and typography. `beastie-game/src/creature_art.rs`
owns creature proportions, palettes, motion tuning and coordinated acting recipes.
`beastie-game/src/environment.rs` owns tank art, plant profiles and distinct toy/food geometry.
The fixed 12-degree overhead camera reveals substrate, cave and plant depth while camera-aligned
UI preserves screen placement. UI meshes do not cast tank shadows. No sprite assets were added.

The [review record](feel-review-voxel-craft.md) records repeated code reviews, the full seven-case
native baseline, final UI and interaction recaptures, actual macOS input, and evidence limitations.
No accepted material finding remains unresolved in the exercised scope. Full-speed human judgment,
physical audio/controller checks and native Windows/Linux acceptance remain explicit follow-ups.
