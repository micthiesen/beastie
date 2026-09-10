# Renderer energy evidence, September 9, 2026

This continues the [efficiency evidence](../20260909-efficiency/README.md) from
commit `41c700631f090c1f4ffd57aee8d9000c03c6dc69`. The
[experiment ledger](../../renderer-energy.md) records decisions and limitations.
All native GPU measurements here are Apple M5 Max / Metal. Shader translation
checks do not establish Windows or Linux performance or native appearance.

## Measurement rules

Run one timed GPU workload at a time, with compilation outside its measurement
interval. Offscreen schema 4 uses bounded ten-frame batches and retains raw
timestamps. Use its summed batch envelopes per dispatched frame, or its warmed
within-batch completion intervals. A whole-run span includes CPU gaps and is not
a GPU execution measurement. Native wall intervals include presentation and
scheduling; independent pass percentiles can overlap and must not be added.

CPU-energy JSON contains raw `proc_pid_rusage` counters, verified process identity,
binary SHA-256, monotonic sample times and the Mach timebase conversion. It reports
kernel-accounted task CPU energy, excluding GPU, display, other processes and
battery losses. Compare equal workload classes, resolution, presentation mode and
settled phases. CPU rates from an ordinary windowed game and an unpaced scripted
report run are not interchangeable. No physical battery-life percentage is claimed.

Read wall p99.9 and maxima as well as p99. The initial indexed implementation
retained a good p99 while adding five spawn frames over 16 ms. Those rejected
intermediate results are preserved, rather than silently replaced by final runs.

## Reproduction packages

- [Lighting experiments](lighting-evidence.tar.gz): cross-pixel transport,
  separate coarse transport, packed invocations, water transfer and unlit-work
  experiments. Includes shader sources, a verified common-harness materializer,
  raw reports and rejected compilation attempts. Its README explains historical
  scratch-path assumptions and the invalid primary-only ablation in those
  prototypes. Full-render timing is the basis of their rejection.
- [Shadow experiments](shadow-prototypes-package.tar.gz): static maps, indexed
  positions, the twelve-view atlas and Depth16, with verified source materializers,
  controls, reports and selected images. Individual gains are sequential and are
  not additive percentages against one common baseline.
- [Single-map soft shadows](pcss-evidence.tar.gz): a faster blocker-search
  prototype rejected because it lost the ball's broad floor shadow and weakened
  contact. Includes source reconstruction and the visibly failing comparison.
- [CPU and hidden-window utilities](energy-validation-archive.tar.gz): macOS
  energy samplers, phase summaries, bounded stack profiling, scratch-only hidden
  instrumentation, objc2 development-profile controls, matched baseline/final
  profiling controls, availability investigation and raw samples. Its
  README distinguishes kernel CPU accounting from unavailable GPU/device power.
- [Viewport density experiments](density-package.tar.gz): paired 640×360 and
  3840×2160 controls, twelve-frame blink and movement comparisons, source
  reconstruction and reviewed native-pixel filmstrips. The policy follows the
  scene's uniform fit, so margins in wide or tall windows do not increase map
  density. Device limits and the tested maximum viewport retain ray fallback.
- [Shadow simplification and background preparation](lod-evidence.tar.gz): the
  frozen tolerance sweep, 63-frame image oracle, native-pixel filmstrips and
  accepted/rejected churn runs. Includes verified async source reconstruction and
  the production integration delta. Earlier synchronous source snapshots were not
  frozen; their retained reports do not imply byte-identical reconstruction.
- [Vertex-cache ordering](shadow-order-package.tar.gz): verified reconstruction,
  four bit-identical images and paired GPU reports. The measured difference was
  too small to retain this additional preparation step.
- [Final integrated validation](final-integrated-evidence.tar.gz): final source
  reconstruction, hashes, serial GPU/native reports, build/test logs, 36-still
  metrics, 504-frame semantic traces, selected native-pixel comparisons and
  combined LOD/density motion filmstrips. Its README separates earlier rendering
  validation from the final profiler cleanup and records corrected failures.
- [Fullscreen baseline comparison](fullscreen-baseline-comparison.tar.gz): full
  3024×1898 baseline/final PNGs, native crops and RGB metrics establishing that the
  narrow water-colored top strip predates this pass. The top 190 rows are exact.

[native-prototype.patch](native-prototype.patch) reconstructs the early native
shadow-control implementation from the base commit. Its adjacent report manifest
identifies measured binaries and notes that the preserved source subsequently
added a `OnceLock` for the unchanged light table. It preserves the rendering
algorithm and controls, not an assertion of byte-identical binary reproduction.
The final production implementation removes experimental runtime controls.

Archives contain no executables, model weights, copied asset trees or complete
videos. `evidence-sha256.json` identifies retained artifacts. Source patches and
materializers use ordinary scratch directories and do not mutate the shared
checkout or require checking out an old revision in place.

## Final result index

[final-results.json](reports/final-results.json) summarizes the raw final GPU,
native and matched-instrumentation reports alongside their names. The six clean
final scenarios contain 22,324 measured frames with no frame over 16 ms. Matched
world/churn controls maintain p99 near 9.2 ms. Original baseline report binaries
use different profiling overhead and development checks; do not compare their
p99 or report-mode CPU energy directly with the final binary. GPU batch timings
and ordinary CPU-energy intervals do not enable that redundant collector.

[final-binaries.json](reports/final-binaries.json) and
[final-source-hashes.json](reports/final-source-hashes.json) identify the integrated
source and final executables. Earlier binary manifests are retained because the
map/still/motion validation preceded the report-only cleanup and diagnostic
binding fix. The source reconstruction in the final archive targets the final
implementation. It does not claim byte-identical reconstruction of every earlier
validation executable. Review [the full result](../../renderer-energy.md) for the
CPU-memory tradeoff and limits on energy and portability claims.

## Native commands

Build once, copy the resulting binary to an immutable task path, and measure it
directly. These are ordinary optimized development builds with assertions and
overflow checks in application code, not release bundles. The pinned objc2
dependency's debug checks are disabled in `dev-perf`; ordinary dev/test keeps them.

```sh
cargo xtask verify
cargo build -p beastie-game --profile dev-perf
target/dev-perf/beastie-game --fake-ai \
  --script fixtures/scenarios/renderer-perf-world.jsonl \
  --render-report /tmp/beastie-world.json --render-uncapped
target/dev-perf/beastie-game --fake-ai \
  --script fixtures/scenarios/renderer-perf-ui.jsonl \
  --render-report /tmp/beastie-ui.json --render-uncapped
target/dev-perf/beastie-game --fake-ai \
  --script fixtures/scenarios/renderer-perf-churn.jsonl \
  --render-report /tmp/beastie-churn.json --render-uncapped
target/dev-perf/beastie-game --fake-ai \
  --script fixtures/scenarios/renderer-perf-sustained.jsonl \
  --render-report /tmp/beastie-sustained.json --render-uncapped
target/dev-perf/beastie-game --fake-ai \
  --script fixtures/scenarios/renderer-window-sizes.jsonl \
  --capture-dir /tmp/beastie-window-sizes
cargo xtask feel --suite gold-reference --game target/dev-perf/beastie-game \
  --output /tmp/beastie-motion
```

Window-size captures are correctness checks, not capture-free frame-time evidence.
They cycle all six window scales from the current setting, then fullscreen and
back; launch windowed to capture all six sizes. Scripted settings are not saved.
The sustained scenario exercises about 93 seconds of world and UI transitions;
it is not a thermal-endurance certification.

Explicit GPU correctness tests remain ignored by the portable gate. Build the
test executable with `cargo test -p beastie-game --profile dev-perf --no-run`, then
run each test separately with `--exact --ignored --nocapture`. The map oracle is
`ray_benchmark::shadow_oracle::moving_shadow_maps_preserve_bounded_image_error`.
It compares current maps/LOD with both full shadow geometry and original rays on
identical frames. `BEASTIE_BENCH_SIZE=low` selects 640×360; `4k` selects 3840×2160;
otherwise it uses 1920×1080. `BEASTIE_DENSITY_MOTION=1` selects twelve consecutive
blink/motion frames, and `BEASTIE_CACHE_ORACLE_OUTPUT` writes the error report.
`BEASTIE_MAP_ORACLE_CAPTURE_DIR` saves native PNG references for visual inspection.
These switches exist only in test code.
