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
| `enum_ops` | 200,000 enum constructions with payloads and exhaustive `match` dispatch |
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
| `arith_loop` | 684.518 | 2.726 | 213.220 | 2.838 | 3.055 |
| `enum_ops` | 687.079 | 3.646 | 164.903 | 3.766 | 4.529 |
| `fib` | 93.506 | 1.795 | 27.288 | 1.544 | 1.794 |
| `list_ops` | 226.563 | 2.738 | 89.147 | 2.223 | 2.526 |
| `map_ops` | 65.779 | 1.781 | 22.596 | 1.355 | 2.119 |
| `struct_ops` | 237.671 | 5.348 | 100.108 | 6.078 | 6.548 |
| `string_ops` | 36.897 | 1.565 | 24.949 | 1.242 | 1.799 |

For comparison, the 2026-11-24 pre-T-055 `string_ops` native median was
8.318 ms; it fell to the 1.6–3.2 ms range across later runs after single-byte
ASCII iteration and indexing stopped allocating a heap string per character.
The 2026-11-22 `map_ops` medians before the hash index were: Vantyx interpreter
106.218 ms, Vantyx native 64.212 ms, Python 3208.396 ms, C 8.360 ms, Rust
9.044 ms. The Python number changed because the workload now uses `dict`
instead of the deliberately naive linear implementation.

Peak RSS and min/max spread are in the raw result files. Representative values:

| Workload | Vantyx native RSS | Vantyx interp RSS | C RSS | Rust RSS |
| --- | ---: | ---: | ---: | ---: |
| `arith_loop` | 2.02 MiB | 3.78 MiB | 1.58 MiB | 2.08 MiB |
| `enum_ops` | 2.01 MiB | 3.72 MiB | 1.60 MiB | 2.09 MiB |
| `list_ops` | 3.46 MiB | 13.81 MiB | 2.77 MiB | 3.37 MiB |
| `struct_ops` | 5.02 MiB | 43.06 MiB | 4.65 MiB | 5.06 MiB |

Build time and binary size:

| Workload | Vantyx build s | C build s | Rust build s | Vantyx binary B | C binary B | Rust binary B |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `arith_loop` | 0.195 | 0.097 | 0.155 | 16,744 | 15,992 | 4,493,248 |
| `enum_ops` | 0.143 | 0.077 | 0.104 | 16,784 | 15,992 | 4,493,272 |
| `fib` | 0.122 | 0.074 | 0.087 | 16,784 | 16,016 | 4,493,104 |
| `list_ops` | 0.134 | 0.085 | 0.192 | 16,840 | 16,144 | 4,494,328 |
| `map_ops` | 0.258 | 0.084 | 0.313 | 21,824 | 16,240 | 4,516,256 |
| `struct_ops` | 0.142 | 0.082 | 0.124 | 16,832 | 16,144 | 4,494,736 |
| `string_ops` | 0.136 | 0.081 | 0.128 | 16,880 | 16,192 | 4,494,072 |

## Interpretation

- **Native vs interpreter:** native is roughly 20×–250× faster on these
  workloads. `map_ops` is ~37× faster than the interpreter because the native
  backend uses a hash index while the interpreter remains linear.
- **Native vs C/Rust:** on these workloads the native backend is in the same
  performance band. Do not read small differences (including cases where
  Vantyx is marginally ahead) as a language-level win; they are within
  measurement noise and reflect workload-specific code generation.
  `string_ops` fell from 8.318 ms to the 1.6–3.2 ms range across runs after
  single-byte ASCII iteration and indexing stopped allocating a heap string per
  character (T-055). `enum_ops` (T-056) puts heap-allocated native enums in the
  same band as stack-value C and Rust enums, so a representation rewrite is not
  justified by this evidence. Immutable concatenation still allocates a fresh
  exact-size string.
- **Maps:** the scalar-key hash index (ADR-0016) cut `map_ops` from 64.212 ms
  to the 1–4 ms band. Native is about 1.3× slower than C's hash table and
  faster than the Rust `HashMap` result on this run. Structural keys still take
  the linear path, and the fixed seed is not collision-resistant against
  adversarial input.
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
| Index (ingest + chunk + embed + save) | 43.482 | 42.662 | 45.614 |
| Search (load + query embed + rank) | 27.175 | 26.505 | 29.117 |
| Search with MMR diversity reranking | 28.292 | 27.354 | 30.767 |

One warm-up pair was discarded; five measured pairs were taken; every baseline
search returned 10 cited hits and the first was `doc-0007.md:1`. MMR reranking
also returned 10 hits with the same top-1 citation and no duplicate texts; the
generated corpus has no identical documents, so this run measures reranking
cost, not duplicate suppression. Index and search are separate processes, so
search includes JSON index loading. This measures local plumbing and flat
linear search, **not** semantic quality, trained-model latency, retrieval
quality, or vector-database performance. Raw results:
[`benchmarks/results/2026-11-24-rag.json`](../benchmarks/results/2026-11-24-rag.json)
and
[`benchmarks/results/2026-11-24-rag.md`](../benchmarks/results/2026-11-24-rag.md).

Command:

```sh
cargo build --release
python3 benchmarks/rag.py --reps 5 --docs 200 --dimensions 64 --top-k 10
```
