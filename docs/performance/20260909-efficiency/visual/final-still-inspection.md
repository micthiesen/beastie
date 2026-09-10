# Final native still inspection, 2026-09-09

No new material visual regression found in the inspected final captures. This is a sampled still review, not a claim that the renderer is pixel-identical or that every dynamic shadow is unchanged.

Inspected the complete 1920x1080 final PNGs: `final-gold/gameplay.png`, `title-screen.png`, `settings.png`, `settings-large.png`; and `final-ui/normal-keyboard.png`, `large-keyboard.png`, `normal-rename.png`, `large-speech.png`, `normal-food.png`, `large-toys.png`, `normal-context.png`, `large-controls.png`. The image viewer may scale complete images to fit its display; original-resolution crops were used for detailed comparisons.

Compared baseline-perf and final crops side by side, with 6x amplified RGB differences: gameplay face at (880,315)-(1070,475), title face at (505,345)-(630,466), gameplay bell's floor shadow at (1170,710)-(1280,791), Normal settings panel at (978,120)-(1853,827), and Large keyboard at (30,18)-(1885,763). Small face and floor crops were enlarged 3x using nearest-neighbor sampling. Reproduction is `final-still-crops.py`; reviewed composites are the corresponding `final-still-*-comparison.png` files. The script also generated Normal keyboard and title rock crops, which were not separately inspected in this pass.

Findings:

- Creature outlines, eyes, eyebrows, mouth, cheek colors, and expression remain intact. Enlarged face comparisons expose small changing shadow tones at eyebrow/eye boundaries and a few cheek/edge pixels. These agree with the already accepted sampled shadow-map tradeoff; there is no new facial distortion or broad shading change in these stills.
- The bell's soft floor shadow remains in the same position and keeps its recognizable shape. The enlarged comparison shows sparse changes around its boundary, not a detached or disappearing shadow.
- Title composition, buttons, water surface, caustic character, rock/plant silhouettes, and toy appearance remain visually coherent at the full capture scale. Earlier accepted primary-raster edge coverage differences are not evidence of authored geometry changes.
- Normal and Large settings and keyboard content remain legible and correctly positioned. The panel comparison preserves translucency, row spacing, outlines, toggle positions and labels. The amplified difference is sparse; no missing text, shifted icons, clipping, or changed panel opacity was found.
- Rename, speech/reaction, food, toy, context, and Controls panels show the expected labels, highlights, disabled states and readable layout. There is no apparent stale retained-label content or overlap in these selected states.

Limits: 12 complete final images and five paired detailed comparisons were visually inspected from the 36 available matched states. Aggregate metrics and the final motion review belong to the root review and are not duplicated here. Stills cannot establish absence of temporal flicker, input latency, performance, or correctness at untested resolutions/camera distances. No game process, GPU test, build, or live-source edit was performed for this inspection.
