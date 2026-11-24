# Prior Art and Research Notes

Date: 2026-11-21. Sources were fetched directly because the web-search API was unavailable. These notes record what informed Auren's v0.1 design; they are not benchmark claims.

## Mojo — ownership and argument conventions

Source: <https://docs.modular.com/mojo/manual/values/ownership/> (v1.2.0).

Observed:
- Single owner per value; destruction at owner lifetime end; references extend lifetime.
- Default argument convention is an immutable reference; `mut` is a mutable reference; `var` transfers ownership; `^` is the caller-side transfer sigil.
- Argument exclusivity for mutable references prevents aliasing surprises and enables optimization.
- Small scalars are passed in registers; ownership is checked by a lifetime checker.

Borrowed for Auren:
- Ownership should be **opt-in and ergonomic**, not mandatory for scripts.
- Mutability should be explicit at API boundaries.
- The long-term plan includes borrow/lifetime checking as an advanced layer.

Rejected for v0.1:
- Caller-side move sigils and strict ownership from day one; they raise the learning curve and delay a working language.

## Roc — friendly, fast, functional

Source: <https://www.roc-lang.org/>.

Observed:
- Prioritizes fast builds and fast execution, compiles to machine code/Wasm.
- Friendly syntax and tooling are first-class goals.
- Small set of primitives, single paradigm.

Borrowed:
- "Friendly" is an engineering requirement: diagnostics and tooling matter as much as semantics.
- Native compilation is a design target from the start, not an afterthought.

Rejected:
- Pure functional paradigm; Auren is multi-paradigm and imperative-friendly.

## Hylo (formerly Val) — mutable value semantics

Source: <https://hylo-lang.org/>.

Observed:
- Value semantics with `inout`/`sink`/`let` conventions; subscripts project values; borrow checking research.
- Claims safe systems programming without a tracing GC; strong focus on generics and C++ interop.

Borrowed:
- Parameter conventions (`let`/`inout`/`sink`) are a promising future direction for explicit, low-cost mutation.
- The idea that value semantics can be both safe and fast.

Rejected for now:
- Full value semantics with projections is a large semantic commitment; Auren v0.1 uses reference semantics for collections and documents the tradeoff.

## Vale — generational references and gradual memory safety

Source: <https://vale.dev/> (archived; successor Valen).

Observed:
- Single ownership without a borrow checker, generational references for safety, regions planned.
- Explicit goal of safety without GC and with easy FFI.
- Emphasizes memory-safety approach tradeoffs.

Borrowed:
- Memory-safety strategy is a spectrum; Auren should offer a simple default and an advanced deterministic mode.
- FFI safety is part of language design, not an afterthought.

Rejected:
- Generational references add runtime metadata and complexity; revisit only with measurements.

## Cranelift vs LLVM vs MLIR

Source: <https://cranelift.dev/>.

Observed:
- Cranelift is fast to compile, written in Rust, avoids UB in its IR, targets x86-64/aarch64/s390x/riscv64, used by Wasmtime; codegen ~10x faster than LLVM-based systems in cited work, with runtime often close but not equal to heavily optimizing compilers.
- LLVM/MLIR offer stronger optimization and broader targets at much greater complexity and build cost.

Decision for Auren: start with a small C backend for native executables (uses system clang, fast to build on a 1 GiB machine), keep IR backend-agnostic, and evaluate Cranelift for JIT/fast debug builds and LLVM for release optimization when benchmarks justify the dependency. No backend is claimed until implemented.

## Other influences

- **Python**: indentation blocks, dynamic ergonomics, batteries-included feel. Rejected: truthiness, implicit coercions, unchecked runtime mutation.
- **Rust**: memory safety, `Result`-style explicit errors, ownership as an advanced tool. Rejected: mandatory lifetimes/borrow syntax for ordinary code.
- **Zig**: explicit allocators, comptime, simple language core. Candidate ideas for later: allocator parameters, compile-time evaluation.
- **Gleam**: small typed language with excellent errors and a Rust implementation. Candidate ideas: friendly diagnostics, no null, exhaustive matching.
- **Julia**: multiple dispatch and numerical performance. Candidate ideas: array/SIMD libraries, not core dispatch.
- **Koka**: effect types. Candidate idea: explicit effects for async/IO in a later phase.

## Naming

"Kestrel" and "Vesper" were rejected after finding active programming languages with those names. "Auren" has no known programming-language collision (only unrelated brands), so it was selected for this project. This is an engineering uniqueness check, not legal trademark advice.
