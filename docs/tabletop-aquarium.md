# Tabletop aquarium design

## Outcome

Make the creature and habitat the first reading, with a compact, purposeful control strip. The
user rejected the prior renderer screenshot as bland, generic and unclear despite its technical
foundation. This pass changes composition and authored design, not the rendering technology.

## Contract

- Preserve ordinary-GPU ray tracing for everything, including outline text; no sprites or atlases.
- Reduce the permanent deck from 50 to about 33 layout units. Place identity on a small tank plate.
  Remove idle behavior text that reads as an unexplained status. Keep actionable technical feedback
  distinct from creature speech, and speech connected to the creature.
- Speech explicitly identifies its speaker; reaction controls use words instead of ambiguous pixel
  faces. Rename has its own input prompt, and sound settings name volume versus spoken replies.
- Give the compose field breathing room without a permanently dominant focus frame. Use explicit
  compact action labels, a distinct muted enamel care affordance and a quieter settings control on
  the rail. Voice and text remain first-class; existing food, toys, settings, binding, recovery and reset paths stay reachable.
- Replace repetitive stepped framing with continuous calm surfaces, thin purposeful edges and
  restrained selection accents. Normal and Large text must fit and remain distinguishable.
- Compose the habitat asymmetrically: planted mass and a low stone bank, open swimming space,
  a recognizable terracotta shelter, broad leaves and deliberately constructed toys. Preserve
  authoritative object positions, creature agency, contact and input projection.
- Context menus choose the opposite vertical region from the creature when opened, including
  midpoint positions and Large text. Keep their controls stationary until dismissal, even if the
  creature later swims behind the panel. Reserve space for technical feedback.
- Picking follows the rendered head and articulated body. Plants and the shelter use their
  actual mesh surfaces so empty gaps cannot select scenery. Use cached presented transforms
  and preserve nearest-hit ordering when the animal overlaps a leaf or shelter edge.
- Keep simulation, saves, audio semantics, offline operation and optional worker failure handling
  intact. No new game mechanics or distribution work belong to this pass.

## Acceptance

Compare native world/UI compositions against a0bb98e. Inspect all Normal/Large UI, speech,
technical feedback, reduced motion, food-drop and settings. Run repeated native interaction and
failure reviews plus actual keyboard/pointer events after layout changes. Independent visual and
code reviews must report specific defects, not equate readable text with good design. Fix accepted
findings, run cargo xtask verify and the normal native dev path, then document and push.

Native frames and synchronized traces provide composition and continuity evidence; full-speed
human perception and physical device feel remain unclaimed when unavailable.
