# Native string-buffer comparison — 2026-11-27

Related: T-111, ADR-0052. This is a focused before/after measurement of the
single-buffer native string construction change, not a competitive claim.

## Workload

[`benchmarks/string_interp.vtx`](../string_interp.vtx):

```text
fn main():
    var i: Int = 0
    var total: Int = 0
    while i < 200000:
        let text: Str = "item-{i}-{i + 1}"
        total = total + len(text)
        i = i + 1
    print(total)
```

Each iteration evaluates a four-part interpolation (two literal chunks and two
integer expressions) and sums its Unicode scalar length. The interpreter, the
pre-change native binary, and the post-change native binary were verified to
produce identical stdout before timing.

## Builds

- **Before:** release build of commit `f1675ac` in a detached worktree at
  `/tmp/opencode/vantyx-before` with `CARGO_TARGET_DIR` outside the repository.
- **After:** release build of the T-111 working tree at the repository root.
- Both used the same command shape:
  `vantyx build benchmarks/string_interp.vtx -o <output>`, which invokes
  clang with `-O2`.
- Compiler: clang 21.1.8 (Debian). Rust: 1.100.0. Python: 3.14.10.
- Machine: Intel Core i7-8565U @ 1.80 GHz, 4 cores, 3,924,440 kB RAM;
  Linux 7.2.15+kali-amd64 (glibc 2.43).

## Method

- Timing and peak RSS use `benchmarks/measure.c` with `wait4(2)`; wall time
  includes process startup, and peak RSS is the child's `ru_maxrss`.
- Nine alternating runs of the before and after binaries on the same machine,
  in one batch, with no other build or benchmark running.
- No warm-up run is included in the table; both binaries had already executed
  once for output verification. Report the median, minimum, and maximum.
- One earlier batch was taken immediately after the release build and showed
  roughly double the wall time for both binaries while the machine was still
  thermally/load-compressed. It was discarded rather than mixed into this
  table; the peak-RSS values in that batch were already in the same range as
  the retained batch.

## Raw runs

| Run | Before ms | Before RSS KiB | After ms | After RSS KiB |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 61.825 | 33,400 | 39.180 | 17,444 |
| 2 | 61.015 | 33,400 | 33.795 | 17,540 |
| 3 | 57.361 | 33,396 | 37.106 | 17,744 |
| 4 | 59.893 | 33,292 | 35.590 | 17,556 |
| 5 | 58.970 | 33,420 | 34.522 | 17,564 |
| 6 | 57.562 | 33,400 | 33.506 | 17,652 |
| 7 | 58.306 | 33,172 | 34.414 | 17,676 |
| 8 | 62.766 | 33,304 | 33.668 | 17,556 |
| 9 | 61.759 | 33,156 | 34.276 | 17,564 |

## Summary

| Metric | Before | After | Change |
| --- | ---: | ---: | ---: |
| Median wall time | 59.893 ms | 34.414 ms | −42.5% |
| Min wall time | 57.361 ms | 33.506 ms | |
| Max wall time | 62.766 ms | 39.180 ms | |
| Median peak RSS | 33,396 KiB (32.61 MiB) | 17,564 KiB (17.15 MiB) | −47.4% |
| Min peak RSS | 33,156 KiB | 17,444 KiB | |
| Max peak RSS | 33,420 KiB | 17,744 KiB | |

## Interpretation

- The pre-change backend allocated one string per interpolation part plus one
  per intermediate concatenation and retained all of them because v0.1 never
  frees native allocations. The post-change backend appends into one growing
  buffer and returns it, so the intermediate strings no longer accumulate.
- Peak RSS falls by about 47% on this loop. The remaining RSS is dominated by
  the one result string retained per iteration, as expected under the
  documented leak model; this change does not add reclamation.
- Wall time falls by about 42% in this run. This is a single-machine
  microbenchmark, not a language-wide performance claim. Absolute times vary
  with CPU frequency and background load.
- The two-operand concatenation path and one-part interpolation path are
  unchanged, so the measurement does not imply a gain for programs that only
  concatenate two strings.
