---
name: Performance Benchmarking
description: Design fair, reproducible benchmarks for compilers, runtimes, AI systems, and numerical code with warm-up, repetitions, statistics, profiling, memory measurement, regression thresholds, and raw evidence.
---

# Performance Benchmarking

## When to use

- Before making or reviewing any speed, throughput, latency, memory, or cost claim.
- Comparing compilers, backends, algorithms, model providers, or hardware.
- Adding a performance regression test or investigating a slowdown.
- Profiling to find a bottleneck before optimizing.

Do not use this skill to justify an optimization after the fact with cherry-picked numbers. The question and methodology come first.

## Required tools

- A benchmark harness (`hyperfine`, `criterion`, `pyperf`, JMH, custom runner) or a deterministic script.
- A profiler (`perf`, flamegraph, `py-spy`, Nsight, `rocprof`, `cachegrind`, `massif`).
- Memory measurement (`/usr/bin/time -v`, RSS sampling, `heaptrack`, `tracemalloc`, `dhat`).
- A stable environment: recorded CPU, GPU, OS, compiler, runtime, flags, and background load.
- Optional: CPU pinning, frequency scaling controls, cgroups, thermal monitoring.

## Safety constraints

- Do not benchmark on a shared or production machine without permission; record contention and thermal state.
- Do not run untrusted benchmark code outside a sandbox or resource limit.
- Do not report a single run, best run, or rounded number without raw samples.
- Do not change the workload, algorithm, input, or output work between variants.
- Do not hide regressions by changing the baseline or excluding slow cases.
- Separate measured effects from projections and noise.

## Workflow

1. **State the question and metric.** Example: "Does change X reduce p50 wall time for workload Y at size Z without increasing peak RSS?" Define the metric, direction, and acceptable noise before running.

2. **Freeze the variants.** Record commit/tree state, dirty diff, compiler/runtime versions, dependency lockfile, flags, and environment. Build each variant from a clean, reproducible state. Include uncommitted changes in the record.

3. **Create a deterministic workload.** Fix input, seed, data source, and output. Add a correctness check so a fast wrong answer is rejected. Avoid network and background services unless they are part of the measurement.

4. **Control the environment.** Pin CPU/GPU where possible, disable frequency scaling or record it, close background load, warm caches, and monitor temperature. For containers, record CPU/memory limits.

5. **Warm up.** Run the workload at least once to page in code, initialize allocators, and populate caches. Exclude warm-up from samples and state how many iterations were used.

6. **Repeat and randomize.** Use enough repetitions to characterize noise (at least 5 for microbenchmarks, more for noisy systems). Randomize or interleave variants to avoid drift. Keep every sample.

7. **Report statistics.** Report min, median, mean, max, standard deviation or IQR, and confidence intervals. State the outlier policy. Do not compare medians without a measure of spread.

8. **Measure memory separately.** Record peak RSS, steady-state RSS, allocations, and per-iteration growth. Use a consistent method and state whether allocator caches or intentional leaks are included.

9. **Profile before optimizing.** Use sampling or instrumentation to identify the bottleneck. Distinguish CPU, memory bandwidth, cache, I/O, lock contention, and launch overhead. Keep profiles as evidence.

10. **Compare compilers/backends fairly.** Same source, flags, target, input, and runtime. Isolate codegen differences; inspect assembly or IR when relevant. Record compiler versions and optimization levels.

11. **Benchmark AI systems correctly.** Report tokens/second, time-to-first-token, end-to-end latency, batch size, context length, concurrency, cost, and quality. Use the same model revision/provider and account for network variance.

12. **Detect regressions.** Store a baseline with environment metadata and raw samples. Set a noise-aware threshold. Run in CI with the same hardware class, or mark the result as non-comparable. Investigate before updating the baseline.

## Validation and acceptance criteria

- The question, metric, and decision rule are stated before measurements.
- Variants differ only in the intended change; everything else is recorded and controlled.
- Correctness is checked as part of the benchmark.
- Raw samples and exact commands are stored, not just a summary.
- Statistics include spread and a stated outlier policy.
- Memory methodology is explicit and comparable.
- Profiling evidence supports the claimed bottleneck.
- Regressions are reported as regressions; no workload substitution.
- The conclusion distinguishes measured effects, noise, and projections.

## Expected output

1. Benchmark question and decision rule.
2. Environment and variant manifest.
3. Workload description and correctness check.
4. Raw samples and analysis script.
5. Summary table with median, spread, and confidence interval.
6. Memory table and methodology.
7. Profile evidence and bottleneck analysis.
8. Conclusion, threats to validity, and regression threshold.

## Troubleshooting

- **High variance:** increase repetitions, pin the CPU, reduce background load, interleave variants, and report the variance instead of choosing the best run.
- **Release-only difference:** verify both binaries use the same flags and that the intended path is actually executed.
- **Compiler optimizes away work:** consume the result (checksum/print) and verify the output is non-trivial.
- **Benchmark too slow:** reduce input size while preserving the workload shape; state the new size and do not extrapolate blindly.
- **Thermal throttling:** monitor clocks/temperature, add cool-down, and report the thermal state.
- **Cache effects dominate:** separate cold and warm runs; report both and state which one matters.
- **AI latency is noisy:** separate network, queueing, prefill, and decode; use multiple runs and report percentiles.
- **Regression test is flaky:** use a hardware-stable runner, a noise-aware threshold, and a retry policy that records all attempts.

## Examples

```sh
# Hyperfine with warm-up and JSON export
hyperfine --warmup 3 --runs 20 --export-json /tmp/bench.json \
  './target/release/app --input data.bin' \
  './target/release/app-new --input data.bin'

# Peak memory
/usr/bin/time -v ./target/release/app --input data.bin 2>&1 | rg 'Maximum resident|Elapsed'

# CPU profile
perf record -g -- ./target/release/app --input data.bin
perf report --stdio | head -40

# Compiler comparison
for cc in gcc clang; do
  CC=$cc cargo build --release --target-dir target/$cc
  hyperfine --warmup 2 --runs 10 "./target/$cc/release/app --tiny"
done
```

## Related skills and review

- Pair with `gpu-numerical-performance` for kernels and hardware counters.
- Pair with `compiler-engineering` for codegen and optimization claims.
- Pair with `ai-systems-engineering` for latency, throughput, and cost.
- Reviewer checks: fair variants, raw data, statistics, memory method, profile evidence, and honest conclusions.
