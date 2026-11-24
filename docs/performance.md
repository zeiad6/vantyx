# Performance

## Status

Auren makes **no competitive performance claims**. A single-machine baseline was measured on 2026-11-22 (T-022); methodology, environment, and raw numbers are in [benchmarks.md](benchmarks.md) and `benchmarks/results/`. The interpreter is a reference implementation; the native backend emits C compiled with `-O2`.

## Current cost model

### Interpreter

- Tree-walking evaluation over the checked AST.
- Environment lookup uses hash maps per scope.
- Values use `Rc<RefCell<...>>` for strings, collections, and structs.
- Reference semantics mean mutations are visible through aliases.
- Call depth is capped at 256; evaluation depth at 1024.
- Recursion and loops are interpreted, not compiled.

Expected consequence: convenient for scripts and semantics testing, not competitive with optimized native code. The measured baseline confirms this: the interpreter was slower than CPython on most of the six microbenchmarks, and 50×–250× slower than the native backend on arithmetic, recursion, list, and struct workloads.

### Native backend

- Typed IR lowers to unboxed C values for `Int`, `Float`, `Bool`, `Str`, plus pointers for structs, lists, and maps.
- Generated C is compiled by clang with `-O2` and linked with libc/libm.
- Integer arithmetic uses checked builtins; list/string bounds and map key lookups are checked.
- Strings, structs, list buffers, and map buffers are allocated and never freed (documented leak).
- `Any` and `List[Any]` are rejected, so there is no dynamic dispatch or boxing. Lists and maps use typed contiguous storage with generated descriptors.
- Native map lookup/insertion is O(n) by design (ADR-0010), so map-heavy workloads should not be expected to match hash-table implementations.

Expected consequence: native code should be substantially faster than the interpreter for supported numeric/struct/list workloads. The 2026-11-22 baseline shows the native backend in the same performance band as C and Rust on arithmetic, recursion, list, and struct workloads; it is ~2.9× slower than C on immutable string concatenation and ~7.7× slower than hand-written C linear arrays on map operations. These are workload-specific observations, not a general performance claim.

## Measured baseline (2026-11-22)

Medians over 5 measured runs after one discarded warm-up, on an Intel
i7-8565U (4 cores, 1.98 GB RAM), Linux 7.2.15, clang 21.1.8, rustc 1.100.0,
Python 3.14.10. Full methodology and raw data: [benchmarks.md](benchmarks.md).

| Workload | Interpreter | Native | Python | C | Rust |
| --- | ---: | ---: | ---: | ---: | ---: |
| `arith_loop` | 1300.9 ms | 5.21 ms | 413.5 ms | 5.10 ms | 5.68 ms |
| `fib` | 167.5 ms | 3.31 ms | 49.8 ms | 3.15 ms | 3.61 ms |
| `list_ops` | 383.6 ms | 4.66 ms | 169.2 ms | 4.40 ms | 5.11 ms |
| `map_ops` | 106.2 ms | 64.2 ms | 3208.4 ms* | 8.36 ms | 9.04 ms |
| `struct_ops` | 405.1 ms | 9.11 ms | 178.6 ms | 11.75 ms | 12.38 ms |
| `string_ops` | 59.8 ms | 7.72 ms | 44.8 ms | 2.64 ms | 3.60 ms |

\* The Python map workload uses linear arrays to match Auren's map design; it
is not `dict` and is not representative of normal Python map performance.

## Interpretation and next optimizations

The baseline identifies where optimization effort is justified:

1. **Map operations** are the largest native gap relative to C/Rust. ADR-0010
   defers a hash index until map-heavy workloads justify the complexity. The
   measured 7.7× gap is evidence for considering it, but not yet a mandate.
2. **String concatenation** allocates a fresh exact-size string per operation.
   A builder or rope API, or geometric native buffers, would help allocation-
   heavy string code without changing value semantics.
3. **Interpreter startup and dispatch** remain the main script-mode cost.
   Variable slot resolution and the bytecode VM (T-011) are the candidates.
4. **Native numeric loops** are already close to C/Rust; constant folding and
   DCE are lower-value than the items above for these workloads.

Optimization order after the baseline:

1. Interpreter variable slot resolution instead of hash lookups.
2. Bytecode VM for startup and dispatch (T-011).
3. Native constant folding and dead-code elimination in the IR.
4. Escape analysis and stack allocation.
5. Specialized collection representations.
6. SIMD/vectorization for numeric loops.
7. Cranelift/LLVM evaluation if C-backend optimization is insufficient.

Correctness and differential tests gate every optimization.
