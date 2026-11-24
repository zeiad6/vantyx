# Benchmarks

## Status

A reproducible baseline was measured on **2026-11-22** on one machine. Raw
results are stored as generated, not hand-edited:

- [`benchmarks/results/2026-11-22.json`](../benchmarks/results/2026-11-22.json) — full data
- [`benchmarks/results/2026-11-22.md`](../benchmarks/results/2026-11-22.md) — generated summary

These are single-machine baseline numbers, not a competitive claim. They exist
to guide optimization and to make regressions visible.

## Rules

- Compare equivalent programs only: same input, same output, same algorithmic work.
- The runner verifies that every implementation prints identical stdout **before** timing.
- Report the exact commands, tool versions, hardware, and date.
- One warm-up run is discarded; report medians and min/max over repeated runs.
- Store raw results under `benchmarks/results/`; do not edit them by hand.
- Never claim "Rust-level performance" or a speedup without stored measurements.
- Do not generalize a microbenchmark result into a language-wide claim.

## Workloads

| Workload | What it exercises |
| --- | --- |
| `arith_loop` | 1,000,000 checked integer loop iterations with multiply/modulo/add |
| `fib` | Recursive `fib(24)` (46,368), function calls and recursion |
| `list_ops` | 150,000 list appends, indexed mutation, and sum |
| `map_ops` | 3,000 linear-map inserts and 3,000 lookups |
| `struct_ops` | 80,000 heap-allocated structs, field mutation, and sum |
| `string_ops` | 15 string doublings (65,536 characters) and a character scan |

Each workload exists as `.auren`, `.py`, `.c`, and `.rs` under
`benchmarks/`. The Auren source is run twice: once through `auren run`
(interpreter, including parsing and type checking) and once as a prebuilt
native executable from `auren build`.

## Methodology

- **Repetitions:** 1 warm-up run discarded, 5 measured runs per
  implementation; the table reports the median, minimum, and maximum.
- **Timing:** wall time including process startup. The timer starts
  immediately before `fork` in a small C helper and stops after `wait4`.
  For `auren-interp`, this includes lexing, parsing, and type checking.
- **Memory:** peak RSS from `wait4(2)` `ru_maxrss`, reported by the same C
  helper. The helper's own resident set (roughly 1.5–2 MiB) is the floor for
  very small binaries.
- **Builds:** `auren build` (including its `clang -O2` invocation), `clang -O2
  -std=c11`, and `rustc -O` are timed separately and excluded from run time.
- **Output verification:** the runner aborts if any implementation's stdout
  differs from the interpreter's.
- **CPU:** not pinned; frequency scaling and background load can move results
  by a few percent. Treat small differences as noise.

## Equivalence notes

- Auren `Int` arithmetic is **checked**; C `int64_t` arithmetic and Rust
  release-mode arithmetic are unchecked. Auren pays for overflow checks.
- Auren maps are insertion-ordered linear scans (ADR-0010). The Python, C, and
  Rust map workloads deliberately use linear key arrays too, so this is not a
  comparison against hash tables. The Python implementation is **not**
  `dict` and is not representative of Python's normal map performance.
- Auren native stores each struct behind its own allocation, so the C and Rust
  struct workloads use one allocation per struct (`malloc` / `Box`) rather than
  a contiguous value array.
- Auren strings are immutable; concatenation allocates a new string. Python
  and Rust follow the same immutable-string model. The C workload uses a
  geometric-growth buffer; total copied bytes are similar for this doubling
  pattern, but the allocation strategy is not identical.
- Binary sizes are not directly comparable: the Rust binaries statically link
  the standard library, while the Auren and C binaries rely on libc/libm.

## Environment

| Item | Value |
| --- | --- |
| Date | 2026-11-22 |
| CPU | Intel Core i7-8565U @ 1.80 GHz, 4 cores |
| RAM | 1,978,576 kB |
| OS | Linux 7.2.15+kali-amd64 (glibc 2.43) |
| Auren | 0.1.0, release build |
| clang | 21.1.8 (Debian) |
| rustc | 1.100.0 |
| Python | 3.14.10 |

## Results (median ms)

| Workload | Auren interp | Auren native | Python | C | Rust |
| --- | ---: | ---: | ---: | ---: | ---: |
| `arith_loop` | 1300.888 | 5.210 | 413.489 | 5.097 | 5.683 |
| `fib` | 167.512 | 3.311 | 49.790 | 3.148 | 3.612 |
| `list_ops` | 383.585 | 4.661 | 169.181 | 4.399 | 5.114 |
| `map_ops` | 106.218 | 64.212 | 3208.396 | 8.360 | 9.044 |
| `struct_ops` | 405.054 | 9.110 | 178.566 | 11.753 | 12.376 |
| `string_ops` | 59.757 | 7.716 | 44.846 | 2.639 | 3.595 |

Peak RSS and min/max spread are in the raw result files. Representative values:

| Workload | Auren native RSS | Auren interp RSS | C RSS | Rust RSS |
| --- | ---: | ---: | ---: | ---: |
| `arith_loop` | 2.09 MiB | 3.12 MiB | 1.60 MiB | 2.00 MiB |
| `list_ops` | 3.36 MiB | 6.45 MiB | 2.77 MiB | 3.34 MiB |
| `struct_ops` | 5.21 MiB | 30.39 MiB | 4.60 MiB | 4.96 MiB |

Build time and binary size:

| Workload | Auren build s | C build s | Rust build s | Auren binary B | C binary B | Rust binary B |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `arith_loop` | 0.236 | 0.165 | 0.206 | 16,736 | 15,992 | 4,493,248 |
| `fib` | 0.238 | 0.140 | 0.162 | 16,776 | 16,016 | 4,493,104 |
| `list_ops` | 0.259 | 0.159 | 0.205 | 16,832 | 16,144 | 4,494,328 |
| `map_ops` | 0.448 | 0.161 | 0.234 | 21,672 | 16,136 | 4,494,432 |
| `struct_ops` | 0.295 | 0.150 | 0.235 | 16,824 | 16,144 | 4,494,736 |
| `string_ops` | 0.267 | 0.171 | 0.221 | 16,784 | 16,192 | 4,494,072 |

## Interpretation

- **Native vs interpreter:** native is roughly 8×–250× faster on arithmetic,
  recursion, list, and struct workloads. `map_ops` is only ~1.65× faster
  because both engines use linear maps and native map equality goes through
  generated descriptors.
- **Native vs C/Rust:** on these workloads the native backend is in the same
  performance band for arithmetic, recursion, list, and struct code. Do not
  read small differences (including cases where Auren is marginally ahead) as
  a language-level win; they are within measurement noise and reflect
  workload-specific code generation. `string_ops` is ~2.9× slower than C
  because immutable concatenation allocates a fresh exact-size string.
- **Maps:** native map operations are ~7.7× slower than hand-written C linear
  arrays. This is the measured cost of descriptor-based structural equality,
  checked growth, and the general-purpose runtime. A hash index should be
  considered only if map-heavy workloads become a priority (ADR-0010).
- **Interpreter:** it is a semantic oracle, not a performance target. It is
  slower than Python on most of these microbenchmarks; the `map_ops` Python
  number reflects the deliberately naive linear implementation, not `dict`.
- **Memory:** native RSS is low (2–5 MiB) because native programs leak but do
  not maintain interpreter metadata. The interpreter's `struct_ops` peak is
  much higher due to `Rc<RefCell<...>>` per struct and per field.

## Reproduce

```sh
cargo build --release
python3 benchmarks/run.py --reps 5
```

To run one workload:

```sh
python3 benchmarks/run.py --workload map_ops --reps 5
```

The runner writes `benchmarks/results/<date>.json` and `.md`. Re-running on the
same date overwrites that date's files; preserve older results by copying them
or changing the date handling before a new official baseline.
