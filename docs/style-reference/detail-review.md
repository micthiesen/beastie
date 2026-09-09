# UI and tank detail review

This follow-up addresses the user's alignment, padding, border and fixture findings.
Earlier reviews checked overall composition but missed measurable detail problems.
Judge UI at native size and in enlarged crops as well as in full-scene comparisons.

## Findings and corrections

| Observed issue | Correction |
| --- | --- |
| Feed icon was 7 px left of its plate center at 1920×1080 | Center occupied mesh cells, then place icons at exact rectangle centers, including half layout units. |
| Chat placeholder was about 30 px above the field center | Separate vertical centering from horizontal alignment; preserve left padding and a stable typing baseline. |
| Settings names/categories and header did not align with controls | Share row/header centers across text, switches, gear and Close. |
| Bindings names and Bindings/Data headers drifted relative to keys and Back | Use explicit matching row/header bounds at Normal and Large sizes. |
| Food and creature action sheets had unequal, cramped outer gutters | Equal outer padding and clear space between focus rims, without increasing sheet height or obscuring the creature. |
| Secondary dialogs retained thick square utility-panel borders | Reuse the restrained stepped rim treatment for food, food-drop instructions, actions, bindings, data, reset, rename and keyboard panels. |
| Rename highlighted the Ball slot | Focus the name field for keyboard/pointer use; retain the controller keyboard flow. |
| Controller rename used message-entry wording | Use the same New name placeholder and Name action as the pointer flow. |
| Tank rails showed parallel lines, corner overlaps and a cyan strip above the hood | One coherent perimeter with flush front planes and an opaque hood; remove the decorative wire strips. |
| The rougher frame still reflected sharp scenery | Use coated metal below the shader's mirror-reflection threshold. |
| Irregular highlights remained on the dry sill | Restrict water caustics to the submerged interior. |
| Lamp lenses hung below flat brown boxes | Symmetric dark sockets with recessed warm lenses; match the practical-light positions to the fixtures. |

The centered overhead key from the preceding pass remains. Creature identity,
simulation positions, care semantics and the title's actual actions are preserved.
Title/menu alignment and toy-card padding did not need further changes.

## Repeatable review

```sh
cargo xtask dev --fake-ai \
  --script fixtures/scenarios/ui-detail-review.jsonl \
  --capture-dir target/captures/ui-detail-review
```

The scenario captures 28 states: gameplay, food choice/drop, toy selection, creature
actions, all settings pages, bindings, data, reset confirmation, rename, title and
the controller keyboard, at Normal and Large text sizes. Reset is only displayed,
never confirmed. Scripted play uses a disposable session.

Review full screenshots, then crop the HUD, field, settings rows, header controls,
each dialog's outer/focus gutters, tank corners, sill and lamps. Measure visible
icon/text extents against their intended bounds. Check actual typing, focus and
hover separately; semantic scripts alone do not prove native input behavior.

## Evidence

- [Before/after scene and detail crops](implemented/detail-comparison.jpg).
- Before: `/tmp/beastie-detail-review-before/captures`.
- Expanded intermediate review: `/tmp/beastie-detail-review-pass1/captures`.
- Final 28-state review: `/tmp/beastie-detail-review-final/captures`.
- Final food-drop and controller naming recaptures after their last corrections:
  `/tmp/beastie-detail-review-final/last-captures`.
- `cargo xtask verify`: 555 tests, 27 dialogue fixtures and 11 STT fixtures passed;
  spoken-input replay passed. The existing macOS linker unwind warning remains.
- Regression tests cover generated icon centers, actual glyph translation without
  horizontal drift, shared settings/binding row centers, header alignment and
  action-sheet focus clearance.
- Final pixel measurements put the Feed icon at x890.5 against a plate center of
  x891, and placeholder ink at y991 against the field center of y990. Horizontal
  text padding is unchanged. These subpixel differences are ordinary raster coverage.
- Native pointer/keyboard checks verified typing, Feed, creature actions and Rename
  without selecting a toy. Controller naming wording was recaptured separately at
  `/tmp/beastie-detail-review-final/name-captures`. Independent code and visual review
  covered the integrated changes.
- The final banner change initially tripped the modal-depth test after its identifier
  changed. Retaining the established identifier fixed the issue; the test remains intact.

Captures establish macOS visual behavior, not Windows/Linux performance or a new
renderer timing result. Earlier timing reports remain historical measurements.
