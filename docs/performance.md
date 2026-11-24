# Performance

## Status

Vantyx makes **no competitive performance claims**. A single-machine baseline
was measured on 2026-11-22 and re-measured on 2026-11-24 after the native
scalar-key map hash index (T-044); methodology, environment, and raw numbers are
in [benchmarks.md](benchmarks.md) and `benchmarks/results/`. The interpreter is
a reference implementation; the native backend emits C compiled with `-O2`.

## Current cost model

### Interpreter

- Tree-walking evaluation over the checked AST.
- Environment lookup uses hash maps per scope.
- Values use `Rc<RefCell<...>>` for strings, collections, and structs.
- Reference semantics mean mutations are visible through aliases.
- Call depth is capped at 256; evaluation depth at 1024.
- Recursion and loops are interpreted, not compiled.
- Maps are insertion-ordered vectors with linear key search.

Expected consequence: convenient for scripts and semantics testing, not
competitive with optimized native code. The measured baseline confirms this:
the interpreter was slower than CPython on most of the six microbenchmarks, and
9×–240× slower than the native backend on these workloads.

### Native backend

- Typed IR lowers to unboxed C values for `Int`, `Float`, `Bool`, `Str`, plus
  pointers for structs, lists, and maps.
- Generated C is compiled by clang with `-O2` and linked with libc/libm.
- Integer arithmetic uses checked builtins; list/string bounds and map key
  lookups are checked.
- Strings, structs, list buffers, map buffers, map indexes, and enum values are
  allocated and never freed (documented leak).
- `Any` and `List[Any]` are rejected, so there is no dynamic dispatch or boxing.
  Lists and maps use typed contiguous storage with generated descriptors.
- Native scalar map keys (`Unit`, `Int`, `Float`, `Bool`, `Str`) use an
  insertion-ordered open-addressing index with a 50% load factor (ADR-0016).
  Struct, enum, list, and map keys keep the O(n) structural-equality path.

Expected consequence: native code should be substantially faster than the
interpreter for supported numeric/struct/list workloads. The 2026-11-24 baseline
shows the native backend in the same performance band as C and Rust on all six
workloads. Immutable string concatenation is the clearest remaining gap at
~3.0× slower than C. These are workload-specific observations, not a general
performance claim.

## Measured baseline (2026-11-24)

Medians over 5 measured runs after one discarded warm-up, on an Intel
i7-8565U (4 cores, 1.98 GB RAM), Linux 7.2.15, clang 21.1.8, rustc 1.100.0,
Python 3.14.10. Full methodology and raw data: [benchmarks.md](benchmarks.md).

| Workload | Interpreter | Native | Python | C | Rust |
| --- | ---: | ---: | ---: | ---: | ---: |
| `arith_loop` | 1422.6 ms | 5.91 ms | 456.0 ms | 4.94 ms | 6.43 ms |
| `fib` | 181.9 ms | 3.38 ms | 53.6 ms | 3.48 ms | 3.52 ms |
| `list_ops` | 480.7 ms | 5.54 ms | 186.7 ms | 6.01 ms | 5.76 ms |
| `map_ops` | 121.7 ms | 3.60 ms | 42.3 ms | 2.88 ms | 4.50 ms |
| `struct_ops` | 489.9 ms | 9.73 ms | 189.0 ms | 11.66 ms | 13.34 ms |
| `string_ops` | 74.8 ms | 8.32 ms | 54.3 ms | 2.76 ms | 3.61 ms |

For comparison, before the hash index on 2026-11-22, native `map_ops` was
64.2 ms versus C at 8.36 ms and Rust at 9.04 ms. The C, Rust, and Python map
workloads now use hash maps; the interpreter remains linear by design.

## Interpretation and next optimizations

The baseline identifies where optimization effort is justified:

1. **String concatenation** is now the largest native gap relative to C. Each
   operation allocates a fresh exact-size string. A builder or rope API, or
   geometric native buffers, would help allocation-heavy string code without
   changing value semantics.
2. **Interpreter startup and dispatch** remain the main script-mode cost.
   Variable slot resolution and the bytecode VM (T-011) are the candidates.
3. **Structural map keys** still take the O(n) path. Recursive hashing is a
   possible follow-up, but it must preserve cycle-safe equality and adversarial
   behavior.
4. **Native numeric loops** are already close to C/Rust; constant folding and
   DCE are lower-value than the items above for these workloads.
5. **Native memory reclamation** is not a speed optimization, but it bounds
   long-running processes and is the largest documented runtime limitation.

Optimization order after the baseline:

1. Interpreter variable slot resolution instead of hash lookups.
2. Bytecode VM for startup and dispatch (T-011).
3. Native string builder or geometric concatenation buffers.
4. Native constant folding and dead-code elimination in the IR.
5. Escape analysis and stack allocation.
6. Specialized collection representations and structural-key hashing.
7. SIMD/vectorization for numeric loops.
8. Cranelift/LLVM evaluation if C-backend optimization is insufficient.

Correctness and differential tests gate every optimization.
