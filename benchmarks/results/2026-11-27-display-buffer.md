# Native display-buffer comparison — 2026-11-27

Related: T-112, ADR-0053. This is a focused before/after measurement of the
single-buffer native collection display change, not a competitive claim.

## Workload

[`benchmarks/display_ops.vtx`](../display_ops.vtx):

```text
fn main():
    var xs: List[Int] = []
    var i: Int = 0
    while i < 10000:
        push(xs, i)
        i = i + 1
    print(len(str(xs)))
```

The program builds a 10,000-element `List[Int]` and measures the Unicode
scalar length of its rendered form. The interpreter, the pre-change native
binary, and the post-change native binary were verified to produce identical
stdout (`58890`) before timing.

## Builds

- **Before:** release build of commit `3e9afd8` in a detached worktree at
  `/tmp/opencode/vantyx-t112-before` with `CARGO_TARGET_DIR` outside the
  repository.
- **After:** release build of the T-112 working tree at the repository root.
- Both used the same command shape:
  `vantyx build benchmarks/display_ops.vtx -o <output>`, which invokes clang
  with `-O2`.
- Compiler: clang 21.1.8 (Debian). Rust: 1.100.0. Python: 3.14.10.
- Machine: Intel Core i7-8565U @ 1.80 GHz, 4 cores, 3,924,440 kB RAM;
  Linux 7.2.15+kali-amd64 (glibc 2.43).

## Method

- Timing and peak RSS use `benchmarks/measure.c` with `wait4(2)`; wall time
  includes process startup, and peak RSS is the child's `ru_maxrss`.
- Nine alternating runs of the before and after binaries on the same machine,
  in one batch, with no other build or benchmark running.
- Both binaries had already executed once for output verification, so no
  separate warm-up run is included.
- The after binary finishes in a few milliseconds, so its wall time is
  dominated by process startup and scheduler noise; peak RSS is the more
  informative after metric here.

## Raw runs

| Run | Before ms | Before RSS KiB | After ms | After RSS KiB |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 759.796 | 568,336 | 4.782 | 2,600 |
| 2 | 805.753 | 568,244 | 7.928 | 2,344 |
| 3 | 795.034 | 568,224 | 6.186 | 2,456 |
| 4 | 725.935 | 568,232 | 5.639 | 2,556 |
| 5 | 752.417 | 568,344 | 10.168 | 2,524 |
| 6 | 733.661 | 568,200 | 6.788 | 2,556 |
| 7 | 806.974 | 568,096 | 5.263 | 2,520 |
| 8 | 754.707 | 568,396 | 4.873 | 2,456 |
| 9 | 723.549 | 568,352 | 5.737 | 2,464 |

## Summary

| Metric | Before | After | Change |
| --- | ---: | ---: | ---: |
| Median wall time | 754.707 ms | 5.737 ms | −99.2% |
| Min wall time | 723.549 ms | 4.782 ms | |
| Max wall time | 806.974 ms | 10.168 ms | |
| Median peak RSS | 568,244 KiB (554.9 MiB) | 2,520 KiB (2.46 MiB) | −99.6% |
| Min peak RSS | 568,096 KiB | 2,344 KiB | |
| Max peak RSS | 568,396 KiB | 2,600 KiB | |

## Interpretation

- The pre-change backend rendered a list by repeatedly concatenating the
  growing result, so displaying `n` elements performed `n` concatenations and
  copied `O(n²)` bytes. Because v0.1 never frees native allocations, every
  intermediate prefix stayed live until process exit; the retained prefixes
  for a 58,890-byte result account for the roughly 555 MiB peak.
- The post-change backend appends every element and separator into one
  `VantyxStrBuilder`, transfers that buffer to the caller, and no longer
  retains intermediate prefixes. The result is unchanged byte-for-byte.
- The remaining after RSS is dominated by the list storage and the one
  temporary display string per scalar element. Removing those per-element
  strings would require typed display appenders and is not part of this
  change.
- This is a single-machine microbenchmark with unpinned CPU frequency, not a
  language-wide performance claim. The algorithmic and retained-memory
  difference is the point; absolute times vary with host state.
