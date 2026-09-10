# Renderer efficiency evidence, September 9, 2026

This directory preserves measurements for the [architecture review](../../renderer-efficiency.md).
The previous renderer is commit `a19edc90450ed0b6beae6541270730693607ee8c`.
Native comparisons use the same optimized development profile on both sides, with corrected
GPU timing backported to the baseline. Experiments use the real frozen seed-42 geometry.
All device measurements are Apple M5 Max / Metal. No Windows or Linux device measurements
are implied by shader translation tests.

## Reading the reports

- `baseline-perf-native-*` and `bounce-perf-native-*`: initial matched native UI/world runs.
  Wall intervals include scheduling and presentation; named GPU pass latency is separate.
- `final-*-repeat*`: repeated native tests and process peak resident memory, when present.
  `final-probe-*`: corrected frozen-scene ablations of the integrated exact-cache renderer.
- `integrated-maps-*`: final source/binary hashes, mapped/ray control benchmarks and GPU oracles.
  Offscreen schema 4 uses bounded ten-frame batches, rejects impossible per-batch wall bounds,
  sums valid batch envelopes and excludes inter-batch intervals from warmed cadence.
- `final-still-comparison`, `final-motion-comparison` and `final-motion-manifest`: fresh final
  mapped-renderer evidence against the baseline. `visual/final-still-inspection.md` records
  reviewed states; `visual/final-motion-worst-comparison.png` preserves the worst decoded pair.
- `offscreen-*-isolated-resolved`: completion-isolated corrected baseline/hybrid GPU latency.
- `throughput-*`, `scheduling-*`, `bounce-o*`: paired small-stack, surface-dictionary, packed-ID,
  workgroup/light-table and static-bounce experiments. They are successive source snapshots,
  not interchangeable measurements of one identical baseline.
- `grid-*`, `compressed*`, `splitshadow-*`: rejected traversal/scheduling variants with paired
  controls. A rejection applies to that implementation and hardware, not every member of a family.
- `shadowmap-*`: approximate dynamic-shadow candidates. Separate quality acceptance from speed.
  The global span of bounded batches contains CPU gaps; use the recorded sum of per-batch GPU
  envelopes or the explicitly filtered within-batch completion intervals.
- `bounce-integrated-motion-oracle` and `one-sample-motion-oracle`: exact half-float cache
  comparisons across 15 real scene transitions and mutations, including the smaller-cache path.
- `still-comparison`, `motion-comparison` and the two motion manifests: image/semantic evidence
  for the integrated exact-cache renderer. Original motion manifests say `build_profile: debug`
  because the capture tool hardcodes that field even for supplied executables. These captures
  actually used `dev-perf`; their executable SHA-256 fields identify the compared binaries.
- `geometry-audit` and `geometry-sources`: representation counts. The global distinct-surface
  count is an opportunity audit; production dictionaries are per mesh.

Raw timestamp pairs are retained. An overlapping raster-start/compute-end span is not an
individual frame's GPU execution cost. Zero, stale or impossible timestamp samples are rejected,
not reported as zero-cost work. Schema evolution and warmup-boundary differences are described
in the architecture review. Do not sum pass percentiles or turn GPU time into claimed display FPS.

The [inspection record](visual/inspection.md) describes accepted primary-edge differences and
the limits of still, video and filmstrip review. The worst decoded video pair is retained in
`visual/motion-worst-comparison.png`; independent H.264 compression affects those pixels.

## Reproduce production measurements

Run only one native game or GPU benchmark at a time and keep compilation separate from timing.
Output paths below are examples; use fresh task-specific paths for a new experiment.

```sh
cargo xtask verify
cargo build -p beastie-game --profile dev-perf
target/dev-perf/beastie-game --fake-ai --script fixtures/scenarios/renderer-perf-world.jsonl \
  --render-report /tmp/beastie-world.json --render-uncapped
target/dev-perf/beastie-game --fake-ai --script fixtures/scenarios/renderer-perf-ui.jsonl \
  --render-report /tmp/beastie-ui.json --render-uncapped
target/dev-perf/beastie-game --fake-ai --script fixtures/scenarios/renderer-perf-sustained.jsonl \
  --render-report /tmp/beastie-sustained.json --render-uncapped

BEASTIE_BENCH_OUTPUT=/tmp/beastie-isolated.json \
  cargo test -p beastie-game offscreen_renderer_benchmark -- --ignored --nocapture
BEASTIE_BENCH_SUBMISSION=batched BEASTIE_BENCH_OUTPUT=/tmp/beastie-batched.json \
  cargo test -p beastie-game offscreen_renderer_benchmark -- --ignored --nocapture
BEASTIE_SHADOW_MAP_CONTROL=trace BEASTIE_BENCH_SUBMISSION=batched \
  BEASTIE_BENCH_OUTPUT=/tmp/beastie-ray-control.json \
  cargo test -p beastie-game offscreen_renderer_benchmark -- --ignored --nocapture
cargo xtask feel --suite gold-reference --game target/dev-perf/beastie-game \
  --output /tmp/beastie-motion-review
```

`cargo xtask dev` defaults to `dev-perf`, preserving assertions and overflow checks.
Use `--profile dev` for an unoptimized development build. These are ordinary executable builds,
not release binaries, installers or model bundles. For a controlled native pair, build and copy
both executables first, then invoke the copied binaries directly with identical scenario flags.

`BEASTIE_BENCH_PROBE` accepts `full`, `primary-only`, `no-shadows`, `no-bounce`,
`no-reflections`, `no-optics` and `single-primary-sample`. Probes diagnose costs; they are not visual
quality settings. `BEASTIE_BENCH_PNG` and `BEASTIE_BENCH_REFERENCE_PNG` save the candidate and
reference images. Offscreen tests exclude animated CPU extraction, uploads and window pacing.
Native scenarios are necessary to assess those costs and interaction transitions.

The sustained scenario contains two full world/UI cycles plus title/settings/continue changes,
with 91.5 seconds of authored waits. It is a short sustained workload, not a thermal endurance
certification or coverage of every possible save. Process RSS is not total GPU driver allocation;
the renderer's nominal attachment/cache bytes are not a driver memory measurement either.

## Reproduce rejected architectures and the baseline

[renderer-experiments.tar.gz](renderer-experiments.tar.gz) contains minimal source patches,
source hashes, raw paired reports, comparison images and `materialize.py`. It reconstructs
the baseline or each grid/BVH4/BVH8/split-shadow experiment from the recorded Git commit in a
new ordinary directory. Its README includes exact commands and validation limits, including
the later reconstruction of the BVH4 working snapshot. No compiled executables are included.
The archive SHA-256 is `d92132cc67d10399a95edf170586c9cde4451bcd4e41f58a2821cc6a309fc268`.
The split-shadow and shadow-map savings are not additive: splitting would retain another
reconstruction, dispatch and buffer transfer after maps already reduce the expensive rays.

Additional candidates preserve patches against the same captured base, source/artifact hashes,
verified materializers, commands and unchanged reports:

| Archive | Contents | SHA-256 |
| --- | --- | --- |
| [shadowmap-archive.tar.gz](shadowmap-archive.tar.gz) | Selected 1024 maps, resolution trials, rejected static-receiver variant, native/mutation/temporal evidence | `01e49d413efefb888d6cb488788d0aa3be1a56eb268088e15ba8a9ee3316efd9` |
| [fastnoise-archive.tar.gz](fastnoise-archive.tar.gz) | Rejected integer water hash, three frozen water times, original controls and image comparison | `a253189c87f54beb50eeb3f12145ab839f898f36a3c4c8a25f2e19328c48a64e` |
| [ellipsoid-evidence.tar.gz](ellipsoid-evidence.tar.gz) | Rejected conservative rounded-mesh bound, exact-image checks and corrected bounded-batch statistics | `3af5af0b3ed47c2d4301cad4530f412464d203f846eb54e0237bd6c4a3dc6f44` |

These contain no compiled binaries, copied asset trees or complete videos. Prototype source
reflects its measured snapshot; production integration subsequently removes rejected controls
and adds the high-resolution fallback and stronger benchmark reporting. An experiment archive
is not a second production renderer.
