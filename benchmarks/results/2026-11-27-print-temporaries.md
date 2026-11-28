# 2026-11-27 — Reclaiming top-level native `print` renderings (T-121)

Focused before/after measurement of `vantyx_print_owned` for statically owned
`print` arguments. This is a narrow reclamation measurement, not a general
memory-model result.

## Environment

- Machine: Intel Core i7-8565U @ 1.80 GHz, 4 logical CPUs, Linux
  7.2.15+kali-amd64, x86_64.
- Rust: `rustc 1.100.0 (0ef1bc386 2026-11-09)`, release profile.
- C compiler: Debian clang 21.1.8, invoked by the native backend with
  `-std=gnu11 -O2`.
- Measurement helper: `benchmarks/measure.c`, built with `cc -O2`. It forks,
  redirects the child's stdout/stderr to `/dev/null`, and reports the child's
  wall time and `ru_maxrss` from `wait4(2)`.

## Workload

`/tmp/opencode/t121/print_heavy.vtx`:

```vtx
struct Point:
    x: Int
    y: Int

fn main():
    var i = 0
    while i < 50000:
        print(i)
        print(float(i) * 1.5)
        print([i, i + 1])
        print(Point(x=i, y=i))
        i = i + 1
```

50,000 iterations produce 200,000 printed lines. Before the change, each of
the four `print` calls allocated a fresh rendering (`vantyx_str_from_int`,
`vantyx_str_from_float`, `vantyx_list_display`, and the generated struct
display) and never freed it.

- Baseline: commit `d6d7b77`, built with `cargo build --release`.
- Patched: T-121 worktree, same flags.

Both compilers built the fixture with the same system C compiler; the
resulting executables were measured with the helper.

## Method

Nine alternating baseline/patched runs in one process-free sequence:

```sh
for i in 1 2 3 4 5 6 7 8 9; do
  benchmarks/measure baseline
  benchmarks/measure patched
done
```

No run was discarded. Values are reported as observed; the median is the
middle of nine values.

## Raw results

| Run | Baseline ms | Baseline maxrss KiB | Patched ms | Patched maxrss KiB |
| --- | ---: | ---: | ---: | ---: |
| 1 | 612.969 | 19,336 | 610.221 | 8,348 |
| 2 | 611.380 | 19,252 | 601.643 | 8,376 |
| 3 | 602.969 | 19,268 | 612.170 | 8,460 |
| 4 | 622.717 | 19,252 | 624.433 | 8,296 |
| 5 | 610.426 | 19,096 | 638.272 | 8,252 |
| 6 | 645.556 | 19,468 | 607.120 | 8,320 |
| 7 | 616.163 | 19,268 | 594.432 | 8,244 |
| 8 | 609.031 | 19,416 | 592.571 | 8,224 |
| 9 | 603.638 | 19,252 | 586.297 | 8,292 |
| **Median** | **611.380** | **19,268** | **607.120** | **8,296** |

Peak RSS range: baseline 19,096–19,468 KiB; patched 8,224–8,460 KiB.

## Result

- Median peak RSS fell from **19,268 KiB to 8,296 KiB**, a reduction of
  10,972 KiB (about 10.7 MiB, 56.9%).
- Median wall time was 611.4 ms before and 607.1 ms after. The difference is
  inside run-to-run noise; this change is not a speed claim.
- The workload output was byte-identical:

```text
e6174790b75cdf0decca87f33b59392343db76567e3116dbc978a9c28f16558b  interpreter
e6174790b75cdf0decca87f33b59392343db76567e3116dbc978a9c28f16558b  baseline native
e6174790b75cdf0decca87f33b59392343db76567e3116dbc978a9c28f16558b  patched native
```

## Focused LeakSanitizer probe

A separate 1,000-iteration probe printed only statically owned renderings:

```vtx
fn main():
    var i = 0
    while i < 1000:
        print(i)
        print(float(i) * 1.5)
        print([i, i + 1])
        i = i + 1
```

It was built through `tools/clang-sanitize.sh` and run with
`ASAN_OPTIONS=detect_leaks=1:abort_on_error=1`. LeakSanitizer reported only the
999 list values created by the `[i, i + 1]` argument (71,928 bytes: list header
plus element buffer). It reported no `vantyx_str_from_int`,
`vantyx_str_from_float`, or list-display rendering leaks. The list value itself
is a fresh temporary that Stage 2 does not yet reclaim; this probe is evidence
for the print-rendering change, not a claim that the program is leak-free.

## Limits

- One machine, one compiler, one fixture. The absolute numbers are not
  portable.
- Only top-level renderings of `print` arguments are reclaimed. `str(...)`,
  interpolation, string `+`, and values bound or stored in containers remain
  caller-owned and unreclaimed, as do lists, maps, structs, enums, and all
  other native allocations.
- The measurement redirects output to `/dev/null`; per-line flushing still
  runs, so the wall time includes 200,000 `fflush` calls.
- This does not replace the staged plan in ADR-0056 and does not claim general
  memory reclamation, leak freedom, or a garbage collector.
