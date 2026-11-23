# Performance measurement

## Scope

The trial microbenchmark measures the deterministic rules engine without Bevy, rendering, disk I/O or UI rebuilding. It is a repeatable starting point, not a measurement of full-game frame rate or memory use.

## Trial rules benchmark

Run from the repository root:

```sh
cargo run --release -p omega-rules --example bench_trials
```

The example performs 50 warm-up runs followed by 1,000 measured seeded trials. It reports total time, average nanoseconds per trial and average samples per trial.

For a useful comparison:

1. Use the same commit, Rust toolchain and release profile.
2. Close unrelated CPU-heavy applications and keep the machine on AC power.
3. Run the command five times and record the median, not the fastest result.
4. Record CPU model, operating system, Rust version, commit SHA and the exact command.
5. Do not compare results from different hardware or toolchains as if they were equivalent.

## Full application measurements

The microbenchmark does not measure Bevy frame pacing, startup, save loading, UI layout, GPU usage or batch-worker memory. Those require a manual run of the complete application.

Record at least:

| Measurement | Scenario | Result |
|---|---|---|
| Startup time | Cold launch to interactive island | Not measured |
| Idle frame rate | Island, 60 seconds | Not measured |
| Workshop update | Repeated part selection and parameter edits | Not measured |
| Batch duration | 1,000 seeded trials | Not measured |
| Peak memory | 1,000-trial batch and 10-minute session | Not measured |
| Save load | Restart with a populated campaign | Not measured |

Do not replace the placeholders with estimates. A performance target is accepted only after the scenario, hardware and measurement method are recorded.
