# Benchmarks

## Status

A reproducible baseline was measured on **2026-11-22** and re-measured on
**2026-11-24** after the native scalar-key map hash index (T-044), on one
machine. Raw results are stored as generated, not hand-edited:

- [`benchmarks/results/2026-11-22.json`](../benchmarks/results/2026-11-22.json) — initial full data
- [`benchmarks/results/2026-11-22.md`](../benchmarks/results/2026-11-22.md) — initial generated summary
- [`benchmarks/results/2026-11-24.json`](../benchmarks/results/2026-11-24.json) — current full data
- [`benchmarks/results/2026-11-24.md`](../benchmarks/results/2026-11-24.md) — current generated summary

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
| `map_ops` | 3,000 map inserts and 3,000 lookups (native/C/Rust/Python hash; interpreter linear) |
| `struct_ops` | 80,000 heap-allocated structs, field mutation, and sum |
| `string_ops` | 15 string doublings (65,536 characters) and a character scan |

Each workload exists as `.vtx`, `.py`, `.c`, and `.rs` under
`benchmarks/`. The Vantyx source is run twice: once through `vantyx run`
(interpreter, including parsing and type checking) and once as a prebuilt
native executable from `vantyx build`.

## Methodology

- **Repetitions:** 1 warm-up run discarded, 5 measured runs per
  implementation; the table reports the median, minimum, and maximum.
- **Timing:** wall time including process startup. The timer starts
  immediately before `fork` in a small C helper and stops after `wait4`.
  For `vantyx-interp`, this includes lexing, parsing, and type checking.
- **Memory:** peak RSS from `wait4(2)` `ru_maxrss`, reported by the same C
  helper. The helper's own resident set (roughly 1.5–2 MiB) is the floor for
  very small binaries.
- **Builds:** `vantyx build` (including its `clang -O2` invocation), `clang -O2
  -std=c11`, and `rustc -O` are timed separately and excluded from run time.
- **Output verification:** the runner aborts if any implementation's stdout
  differs from the interpreter's.
- **CPU:** not pinned; frequency scaling and background load can move results
  by a few percent. Treat small differences as noise.

## Equivalence notes

- Vantyx `Int` arithmetic is **checked**; C `int64_t` arithmetic and Rust
  release-mode arithmetic are unchecked. Vantyx pays for overflow checks.
- Native Vantyx maps use an insertion-ordered scalar-key hash index (ADR-0016);
  struct/enum/list keys keep structural linear search. The C, Rust, and Python
  map workloads use their standard hash maps, so the 2026-11-24 native
  comparison is between algorithmically equivalent implementations. The
  interpreter remains a linear reference and is intentionally the slow case.
  The 2026-11-22 run used linear arrays in every language and is retained for
  comparison, not as the current methodology.
- Vantyx native stores each struct behind its own allocation, so the C and Rust
  struct workloads use one allocation per struct (`malloc` / `Box`) rather than
  a contiguous value array.
- Vantyx strings are immutable; concatenation allocates a new string. Python
  and Rust follow the same immutable-string model. The C workload uses a
  geometric-growth buffer; total copied bytes are similar for this doubling
  pattern, but the allocation strategy is not identical.
- Binary sizes are not directly comparable: the Rust binaries statically link
  the standard library, while the Vantyx and C binaries rely on libc/libm.

## Environment

| Item | Value |
| --- | --- |
| Date | 2026-11-24 |
| CPU | Intel Core i7-8565U @ 1.80 GHz, 4 cores |
| RAM | 1,978,576 kB |
| OS | Linux 7.2.15+kali-amd64 (glibc 2.43) |
| Vantyx | 0.1.0, release build |
| clang | 21.1.8 (Debian) |
| rustc | 1.100.0 |
| Python | 3.14.10 |

## Results (median ms)

| Workload | Vantyx interp | Vantyx native | Python | C | Rust |
| --- | ---: | ---: | ---: | ---: | ---: |
| `arith_loop` | 1422.623 | 5.910 | 455.966 | 4.944 | 6.430 |
| `fib` | 181.901 | 3.376 | 53.608 | 3.483 | 3.518 |
| `list_ops` | 480.670 | 5.540 | 186.734 | 6.013 | 5.764 |
| `map_ops` | 121.728 | 3.597 | 42.253 | 2.884 | 4.495 |
| `struct_ops` | 489.947 | 9.730 | 189.024 | 11.663 | 13.344 |
| `string_ops` | 74.813 | 8.318 | 54.285 | 2.764 | 3.611 |

For comparison, the 2026-11-22 `map_ops` medians before the hash index were:
Vantyx interpreter 106.218 ms, Vantyx native 64.212 ms, Python 3208.396 ms,
C 8.360 ms, Rust 9.044 ms. The Python number changed because the workload now
uses `dict` instead of the deliberately naive linear implementation.

Peak RSS and min/max spread are in the raw result files. Representative values:

| Workload | Vantyx native RSS | Vantyx interp RSS | C RSS | Rust RSS |
| --- | ---: | ---: | ---: | ---: |
| `arith_loop` | 2.00 MiB | 3.52 MiB | 1.60 MiB | 2.11 MiB |
| `list_ops` | 3.39 MiB | 14.29 MiB | 2.77 MiB | 3.31 MiB |
| `struct_ops` | 5.14 MiB | 44.52 MiB | 4.65 MiB | 5.11 MiB |

Build time and binary size:

| Workload | Vantyx build s | C build s | Rust build s | Vantyx binary B | C binary B | Rust binary B |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `arith_loop` | 1.086 | 0.181 | 1.956 | 16,744 | 15,992 | 4,493,248 |
| `fib` | 0.988 | 0.178 | 1.085 | 16,784 | 16,016 | 4,493,104 |
| `list_ops` | 1.065 | 0.462 | 1.473 | 16,840 | 16,144 | 4,494,328 |
| `map_ops` | 1.032 | 0.213 | 1.312 | 21,824 | 16,240 | 4,516,256 |
| `struct_ops` | 0.765 | 0.263 | 1.106 | 16,832 | 16,144 | 4,494,736 |
| `string_ops` | 0.620 | 0.179 | 1.339 | 16,784 | 16,192 | 4,494,072 |

## Interpretation

- **Native vs interpreter:** native is roughly 9×–240× faster on these
  workloads. `map_ops` is now ~34× faster than the interpreter because the
  native backend uses a hash index while the interpreter remains linear.
- **Native vs C/Rust:** on these workloads the native backend is in the same
  performance band. Do not read small differences (including cases where
  Vantyx is marginally ahead) as a language-level win; they are within
  measurement noise and reflect workload-specific code generation.
  `string_ops` remains the clearest gap at ~3.0× slower than C because
  immutable concatenation allocates a fresh exact-size string.
- **Maps:** the scalar-key hash index (ADR-0016) cut `map_ops` from 64.212 ms
  to 3.597 ms. Native is now 1.25× slower than C's hash table and faster than
  the Rust `HashMap` result on this run. Structural keys still take the linear
  path, and the fixed seed is not collision-resistant against adversarial
  input.
- **Interpreter:** it is a semantic oracle, not a performance target. It is
  slower than Python on these microbenchmarks, and its map workload is
  intentionally linear rather than a `dict`.
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

## Local RAG plumbing benchmark (2026-11-24)

`benchmarks/rag.py` measures the offline RAG path separately from the language
workloads. It generates 200 deterministic Markdown documents (468,324 source
bytes), indexes them with the lexical hash provider at 64 dimensions, and runs
hybrid search for `compiler memory safety native map retrieval` with top-k 10.
The index contains 600 chunks and 600 vectors (1,247,300 bytes).

| Phase | Median ms | Min ms | Max ms |
| --- | ---: | ---: | ---: |
| Index (ingest + chunk + embed + save) | 41.639 | 40.347 | 42.529 |
| Search (load + query embed + rank) | 26.720 | 25.133 | 27.154 |

One warm-up pair was discarded; five measured pairs were taken; every search
returned 10 cited hits. Index and search are separate processes, so search
includes JSON index loading. This measures local plumbing and flat linear
search, **not** semantic quality, trained-model latency, or vector-database
performance. Raw results:
[`benchmarks/results/2026-11-24-rag.json`](../benchmarks/results/2026-11-24-rag.json)
and
[`benchmarks/results/2026-11-24-rag.md`](../benchmarks/results/2026-11-24-rag.md).

Command:

```sh
cargo build --release
python3 benchmarks/rag.py --reps 5 --docs 200 --dimensions 64 --top-k 10
```
