# Native display-temporary reclamation — 2026-11-27

Related: T-114, ADR-0054. This is a focused before/after measurement of freeing
nested native display temporaries, not a competitive claim.

## Workloads

1. [`benchmarks/display_ops.vtx`](../display_ops.vtx), the tracked 10,000-element
   `List[Int]` display workload used by T-112.
2. An ad hoc 100,000-element `List[Int]` display, to make the retained
   per-element temporaries large enough to stand out from process overhead:

   ```text
   fn main():
       var xs: List[Int] = []
       var i: Int = 0
       while i < 100000:
           push(xs, i)
           i = i + 1
       print(len(str(xs)))
   ```

3. An ad hoc 50,000-element `List[Pair]` display where every struct has two
   `Bool` fields, to measure the cost of making nested `Bool` renderings
   heap-owned while freeing the enclosing struct renderings:

   ```text
   struct Pair:
       left: Bool
       right: Bool

   fn main():
       var pairs: List[Pair] = []
       var i: Int = 0
       while i < 50000:
           push(pairs, Pair(left=true, right=false))
           i = i + 1
       print(len(str(pairs)))
   ```

The interpreter, the pre-change native binary, and the post-change native
binary produced identical stdout for every workload before timing: `58890`,
`688890`, and `1500000` respectively.

## Builds

- **Before:** release build of commit `fc453cf` in a detached worktree at
  `/tmp/opencode/vantyx-t114-before`, with `CARGO_TARGET_DIR` outside the
  repository.
- **After:** release build of the T-114 working tree at the repository root.
- Both built the native workload with the same command shape:
  `vantyx build <source> -o <output>`, which invokes clang with `-O2`.
- Compiler: clang 21.1.8 (Debian). Rust: 1.100.0. Python: 3.14.10.
- Machine: Intel Core i7-8565U @ 1.80 GHz, 4 cores, 3,924,440 kB RAM;
  Linux 7.2.15+kali-amd64.

## Method

- Timing and peak RSS use [`benchmarks/measure.c`](../measure.c) with
  `wait4(2)`; wall time includes process startup, and peak RSS is the child's
  `ru_maxrss`.
- Nine alternating runs of the before and after binaries on the same machine,
  in one batch per workload, with no other build or benchmark running.
- Both binaries had already executed once for output verification, so no
  separate warm-up run is included.
- The tracked 10,000-element workload and the 50,000-struct workload finish in
  a few milliseconds, so wall time is sensitive to scheduler noise; peak RSS is
  the more informative metric for those runs.

## Raw runs

### Tracked `display_ops.vtx` (10,000 Int)

| Run | Before ms | Before RSS KiB | After ms | After RSS KiB |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 5.611 | 2,312 | 5.310 | 2,080 |
| 2 | 4.802 | 2,456 | 4.473 | 2,308 |
| 3 | 5.605 | 2,328 | 5.541 | 2,128 |
| 4 | 4.756 | 2,344 | 5.293 | 2,184 |
| 5 | 4.740 | 2,652 | 4.562 | 2,180 |
| 6 | 4.826 | 2,464 | 4.547 | 2,396 |
| 7 | 4.755 | 2,440 | 4.367 | 2,072 |
| 8 | 5.021 | 2,448 | 4.283 | 2,172 |
| 9 | 4.979 | 2,460 | 6.900 | 2,344 |

### Ad hoc 100,000 Int

| Run | Before ms | Before RSS KiB | After ms | After RSS KiB |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 39.212 | 6,556 | 36.244 | 3,448 |
| 2 | 30.564 | 6,424 | 22.333 | 3,616 |
| 3 | 26.412 | 6,552 | 20.922 | 3,616 |
| 4 | 24.896 | 6,552 | 22.422 | 3,600 |
| 5 | 25.819 | 6,424 | 20.277 | 3,728 |
| 6 | 25.137 | 6,696 | 22.081 | 3,592 |
| 7 | 26.947 | 6,740 | 21.224 | 3,716 |
| 8 | 28.494 | 6,560 | 21.339 | 3,616 |
| 9 | 25.722 | 6,500 | 22.357 | 3,716 |

### Ad hoc 50,000 `Pair(Bool, Bool)` structs

| Run | Before ms | Before RSS KiB | After ms | After RSS KiB |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 33.553 | 9,224 | 29.750 | 5,580 |
| 2 | 42.842 | 9,116 | 24.940 | 5,588 |
| 3 | 24.274 | 9,240 | 19.682 | 5,580 |
| 4 | 24.779 | 9,344 | 20.309 | 5,724 |
| 5 | 26.590 | 9,404 | 20.447 | 5,532 |
| 6 | 22.318 | 9,468 | 20.877 | 5,400 |
| 7 | 23.055 | 9,368 | 19.970 | 5,624 |
| 8 | 25.532 | 9,564 | 22.131 | 5,388 |
| 9 | 26.302 | 9,532 | 29.495 | 5,520 |

## Summary

| Workload | Metric | Before | After | Change |
| --- | --- | ---: | ---: | ---: |
| `display_ops` 10k | Median wall time | 4.826 ms | 4.562 ms | −5.5% |
| `display_ops` 10k | Median peak RSS | 2,448 KiB | 2,180 KiB | −10.9% |
| 100k Int | Median wall time | 26.412 ms | 22.081 ms | −16.4% |
| 100k Int | Median peak RSS | 6,552 KiB | 3,616 KiB | −44.8% |
| 50k Bool structs | Median wall time | 25.532 ms | 20.877 ms | −18.2% |
| 50k Bool structs | Median peak RSS | 9,368 KiB | 5,580 KiB | −40.4% |

Peak-RSS min/max ranges:

| Workload | Before min–max KiB | After min–max KiB |
| --- | ---: | ---: |
| `display_ops` 10k | 2,312–2,652 | 2,072–2,396 |
| 100k Int | 6,424–6,740 | 3,448–3,728 |
| 50k Bool structs | 9,116–9,564 | 5,388–5,724 |

## Interpretation

- Before T-114, list display appended each rendered element without freeing it.
  The 100,000-element Int run therefore retained 100,000 small strings until
  process exit; the 50,000-struct run additionally retained one rendered
  string per `Pair`. After T-114 each temporary is freed immediately after
  `vantyx_sb_append_str` copies it, so the allocator reuses those blocks.
- The 50,000-struct run shows that the net effect is a large memory reduction
  even though `Bool` fields now allocate owned `"true"`/`"false"` copies:
  freeing the enclosing per-struct renderings dominates the two extra small
  allocations per struct. The owned `Bool`/`Unit` variants exist so the free
  point can be unconditional; they are not the source of the memory win.
- Wall time also fell in these batches, but the runs are short and unpinned.
  The retained-memory reduction is the supported claim; the timing numbers are
  recorded for transparency and should not be treated as a stable speed claim.
- Output bytes, ordering, quoting, cycle markers, depth limits, and runtime
  errors are unchanged; the differential and sanitizer suites verify behavior.

## Limits

- Single machine, unpinned CPU frequency, nine runs per workload. Absolute
  values are not comparable with other dates or hosts.
- The 100,000-element and 50,000-struct sources are recorded inline above, not
  added to `benchmarks/run.py`; they are focused before/after probes, not
  cross-language suite workloads.
- This is not general reclamation. Final display results, user strings,
  collection storage, and every other native allocation remain intentionally
  unreclaimed in v0.1. Typed display appenders that avoid the per-element
  allocation entirely remain deferred.
