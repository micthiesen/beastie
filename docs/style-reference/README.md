# Gold visual references

These original, unmodified user-supplied images are the gold visual references for
Beastie's UI and art direction. Future UI and rendering work should use them as
the visual baseline, alongside the game's [product philosophy](../game-design-philosophy.md).

- [Gameplay and selected toy](gameplay.jpg): Photo 3.
- [Settings overlay](settings.jpg): Photo 2.
- [Title screen](title-screen.jpg): Photo 1, the primary title composition.

The original title image reads “Mop”. The implemented game logo must read “Beastie”;
Mop is the default creature name and belongs in creature labels and dialogue. This
deliberate branding correction preserves the reference's lettering treatment and
composition. The original images remain unchanged.

The aquarium fills the screen: deep teal water, warm ivory highlights, golden
sand, natural green plants, sparse bubbles and asymmetric voxel scenery around
open swimming space. Warm frame lighting contrasts with cool water. Surface
ripples, caustics and gentle shafts give depth without obscuring the creature.

Gameplay controls belong in a compact dark teal bottom bar. Selected toys have
restrained in-world feedback and a small nearby card. Settings float center-right
over the living tank, with a narrow category column and spacious controls. The
title has illuminated voxel lettering and a minimal vertical menu directly over
the aquarium. Cream text, shallow fabricated plates and quiet active edges share
the world's visual language. Avoid debug panels, dense labels and bright RGB UI.

Actual care, dialogue, accessibility, privacy and save semantics take precedence
over mockup labels. No fictional controls or background creatures are implied.
Implementation differences and final native comparisons are recorded in
[implementation.md](implementation.md), with iterative findings and performance decisions
in [refinement.md](refinement.md), alongside these permanent references.
The [detail review](detail-review.md) records subsequent alignment, padding, frame
and light-fixture corrections, with a repeatable 28-state UI capture scenario.
