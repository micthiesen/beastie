# UI target set, 2026-10-01

These are original ImageGen design references for the complete UI and feel redesign. They are
not captures of Beastie and are not loaded by the game. Full generation prompts are preserved
in [prompts.json](prompts.json). Implementation decisions and verification live in
[the contract](../../ui-redesign-20261001.md) and
[the review record](../../feel-review-ui-20261001.md).

The common direction is smooth dark teal surfaces, restrained brass rims, ivory text, mint
secondary text, generous controls and deliberate hierarchy around the voxel creature and tank.
The generated component and embodied boards cover shared interactions across every page.

Six [original native captures](implemented/captures.json) show the implementation:
[toy selection](implemented/toy-card.png), [Large settings](implemented/settings-large.png),
[Unicode name editing](implemented/name-editing.png),
[full-tank feedback](implemented/food-capacity.png),
[quiet foraging](implemented/quiet-forage.png) and
[clear sock contact](implemented/clear-sock-contact.png). These are unedited 1280×720 PNGs, reviewed
individually. Their manifest records source paths, hashes and candidate versions. The C3/C5
screens establish unchanged UI layouts; the C6 still establishes actual full-tank feedback.
The C8 forage and C9 sock frames are extracted from their native movies. Toy and body motion have separate
synchronized evidence in the review record; a single still cannot establish continuity.

[The evidence index](evidence.json) preserves all target hashes and final native recording
identities, seeds, frame counts, source audit and verification totals. Large recordings remain
in scratch storage; the review records their findings and limitations.

| Target | Complete surface coverage | Authored verification source |
| --- | --- | --- |
| [gameplay](targets/gameplay.png) | Persistent dock, identity and qualitative summary, toy miniatures, Feed, compose, Send, microphone, Settings | `ui-detail-review; surfaces; presentation` |
| [title](targets/title.png) | Title logo, tagline, Continue, Settings, Quit, version and title return | `ui-detail-review; native input` |
| [settings-display](targets/settings-display.png) | Text size, motion, flashes, shake, six scales and fullscreen | `settings; renderer-window-sizes; feel/reduced-effects` |
| [settings-sound](targets/settings-sound.png) | Effects, speech volume, spoken replies, subtitles, microphone and text speed | `settings` |
| [settings-controls](targets/settings-controls.png) | Bindings, local data, title navigation | `settings` |
| [food-choice](targets/food-choice.png) | Berry, mushroom and pellet chooser, placement and cancellation | `ui-detail-review; surfaces` |
| [toy-choice](targets/toy-choice.png) | Ball, bell and sock chooser and physical toy miniatures | `voxel-craft-ui; surfaces` |
| [context-inspect](targets/context-inspect.png) | Creature care actions, three selected toy cards, plant, cave, food and open-water inspection | `ui-detail-review; surfaces` |
| [speech](targets/speech.png) | Progressive and complete speech, reactions, long speech and Large text | `presentation; feel/dialogue-races` |
| [speech-continuation](targets/speech-continuation.png) | Large long captions, complete bounded pages, Back/Next, progressive reveal and reading time | `presentation` |
| [speech-compact](targets/speech-compact.png) | Revised compact captions, content-sized continuation, final-page shrink and clear Settings composition | `presentation; native input` |
| [rename](targets/rename.png) | Dedicated name field, empty/valid/long Unicode input, count, cancel and save | `surfaces` |
| [bindings](targets/bindings.png) | Six binding rows, every capture prompt, conflict, cancel and reset defaults | `ui-detail-review; surfaces; settings; native input` |
| [data](targets/data.png) | Recovery, transcript opt-in, enabled/disabled export, local file feedback | `data-flow` |
| [reset](targets/reset.png) | Reset confirmation, safe default, actual reset and recovery | `data-flow` |
| [keyboard](targets/keyboard.png) | Message and name keyboards, letters, punctuation, delete, close and submit | `surfaces` |
| [notices-microphone](targets/notices-microphone.png) | Technical notices, food-capacity feedback, long paths/errors, disabled/idle/listening/recognizing/unavailable/error microphone | `presentation; capacity; data-flow; feel/bad-conditions` |
| [components](targets/components.png) | Normal, hovered, focused, selected, disabled, pressed, typography and common surfaces | `all ui-redesign fixtures` |
| [text-editing](targets/text-editing.png) | Focused caret, whole-field selection, replacement, bounded paste and long draft suffixes in message/name fields | `presentation; native input` |
| [embodied-feel](targets/embodied-feel.png) | Food approach/acceptance/refusal, toy contact/refusal, comfort, sleep, quiet life and relationship acting | `feel baseline suite; food-refusal; reduced-effects` |
| [toy-contact](targets/toy-contact.png) | Surface approaches from either side, bell contact, directional ball response, true sock carrying/release and separated quiet rest | `feel/quiet-observation; interaction-chain; direct-toy-contact` |

Fixture shorthand refers to `fixtures/scenarios/ui-redesign-*.jsonl` unless a different name
is shown. `ui-detail-review` and `voxel-craft-ui` are existing same-name scenarios used for matched
before/after captures; `native input` denotes real pointer/keyboard checks recorded separately.
The new capture fixtures extend those scenarios with settings, presentation and local data.
`data-flow-large` repeats the complete data flow at Large text in its own fresh local storage.
`edge-cases` adds populated transcript export, compact inspection strips, long names and
joined emoji at both text sizes. `feel/direct-toy-contact` uses seed 12 so all
three authored preferences accept direct play, exposing each physical response and recovery.
The supplemental `ui-polish` suite contains `food-refusal` (seed 8, real mushroom refusal and
berry consumption) and `reduced-effects` (seed 12, all three accessibility reductions enabled
through actual controls, followed by toy contact/release and speech).
Presentation previews are explicitly synthetic layout states. They prove
rendering coverage only; actual input, save recovery and semantic outcome evidence are separate.

## Reconciled illustration details

- Volume controls retain the actual five quarter-volume choices. An illustrative 80% is not
  a new setting. Transcript opt-in remains under Save & data.
- Speech remains creature expression, with no guaranteed response or generated world facts.
  Technical notices remain visibly separate from creature speech.
- Microphone input is hold-to-talk with the player's current binding. Illustrative tap copy
  is not adopted. Voice remains optional and text remains equivalent.
- The continuation board's incidental broom is not adopted; no new cleanup action exists. Long
  captions use pages within the current speech only, never a conversation history.
- Native review supersedes the original continuation board's broad panel with the compact
  caption board. Its invented Brightness/Background Mood rows, portrait badges and tinted emoji
  are illustrative only; actual settings and the monochrome font fallback define implementation.
- Insets and state examples on boards describe alternatives, never simultaneous dialogs.
- The toy-contact board's two sock positions describe successive carried/released states,
  not two socks. Its idealized poses guide readable contact; authoritative world geometry
  and the creature's existing articulated motion remain the implementation authority.
- The text-editing board's incidental mascot, taglines and clipboard toast are not adopted.
  Selection and the changed field contents provide the editing feedback within the existing dock/dialog.
- The creature, tank, actual food and toy miniatures remain voxel geometry. Utility icons,
  rounded panels and outline typography use retained vector geometry in the same renderer.
- Inspection exposes qualitative facts and discovered preferences only. No precise need,
  relationship or personality meters are introduced.
- Reset always defaults to Cancel and preserves recoverable local data. Backup recovery must
  work in the real file flow, not merely look reassuring in a picture.
