# Native comparison with matched profiling overhead

The apparent final p99 regression does not reproduce when the41c7006 renderer uses the same objc2 development-profile override and redundant Bevy diagnostic collector removal. This is one serial matched pair per scenario; the small percentile differences are not strong evidence of a p99 improvement.

|Scenario/renderer|p50ms|p99ms|maxms|frames>33ms|CPUtaskW|CPUcores|
|---|---:|---:|---:|---:|---:|---:|
|World baseline|8.315792|9.273875|10.076459|0|0.473918|0.781333|
|World final|8.327542|9.247167|9.706375|0|0.351537|0.616798|
|Churn baseline|8.319625|9.309083|10.474209|0|0.483129|0.802315|
|Churn final|8.329000|9.204750|9.762458|0|0.312851|0.675130|

The final reduces observed kernel-accounted task CPU energy approximately26% in world and35% in churn, and CPU time approximately21%/16%. These are CPU-only counters, not GPU/whole-device battery power. The earlier unmatched8.64–8.70ms baseline p99 and~1.34W report energy must not be used to isolate architecture improvements, because those runs included different validation and diagnostic overhead.

`baseline-matched-src` was recreated from git archive41c7006. Only Cargo.toml and ray_stats.rs changed: the version-specific objc2 dev-perf assertion override, removal of Bevy diagnostic plugin/legacy scanning, and directly affected test/metadata updates. Existing custom timestamp slots, shadow-map APIs and renderer code remain original. There is no fixture CLI relaxation. `baseline-matched.patch` and `baseline-matched-source.json` record the exact diff,basecommit and before/after hashes.

Both binaries used dev-perf. Final SHA25644d9aecdc4030829583bf7b3e2373e851ac59b7ccb3dc299bf1fd94bb2e202cd. The serial order was matched-baseline-world, matched-final-world, matched-baseline-churn, matched-final-churn. `run-native-energy.py` launched the same current fresh-save scenario scripts with fake AI, --render-report and --render-uncapped, recording exact commands and binary hashes in each*-energy.json. Native viewport1920×1080 requested AutoNoVsync; approximately8.33ms wall cadence is observed, not a claim of uncapped physical presentation. Report wall statistics exclude initial30frames, and CPUenergy summaries use samples at least3seconds after launch.

Compilation finished before all four native intervals. Root was asked to pause PNG analysis during sampling. All runs exited successfully. Raw per-frame reports, task counters and logs are retained as matched-*.json/log. This bounded result supports no measured p99 regression under matched instrumentation; it does not establish a universal tail bound, cross-device performance, or sustained thermal behavior. Root owns the wider final verification evidence.
