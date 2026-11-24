# ADR-0008: Typed IR with a C11 backend for native builds

- Status: Accepted (provisional)
- Date: 2026-11-21

## Context

The interpreter is the semantic oracle, but Vantyx's mission requires native executables. The project runs on a 1 GiB Linux machine, has a zero-dependency compiler core (ADR-0007), and must keep the front end reusable by future backends.

## Problem

How should Vantyx lower checked programs to native code without (a) committing to a heavyweight dependency, (b) creating a second semantic implementation, or (c) silently miscompiling constructs the backend cannot represent?

## Options considered

1. **C emission via the system C compiler.** Lower checked AST to a typed IR, emit GNU C11, invoke `clang` (or `VANTYX_CC`), link with libc/libm.
2. **Cranelift.** Pure-Rust codegen, fast compile times, no C toolchain, but a large dependency and an immature ecosystem for some targets.
3. **LLVM.** Best optimization and target coverage, but heavy build/link requirements and a large dependency footprint.
4. **Bytecode VM first.** Portable and fast to start, but it does not produce native executables and delays the native path.

## Decision

Use option 1 now: a deliberately small, typed IR (`src/ir.rs`) and a C11 emitter (`src/codegen_c.rs`). The interpreter remains the semantic oracle. The IR is backend-neutral; C is the first backend, not the final architecture.

The backend covers a statically typed subset:

- types: `Unit`, `Int`, `Float`, `Bool`, `Str`, nominal structs;
- declarations: globals, functions, structs;
- control flow: `if`/`elif`/`else`, `while`, `for ... in range(...)`, `return`, `break`, `continue`;
- built-ins: `print`, `len(Str)`, `str`, `int`, `float`, `bool`, `type`, `abs`, `min`, `max`, `sqrt`, `floor`, `ceil`, `assert`.

Constructs outside the subset are rejected with `E0401` and a help message. They are never silently lowered to different semantics.

## Rationale

- No new dependency; builds quickly on the target machine.
- The system C compiler provides optimization, target support, and linking.
- The IR boundary is a real compiler stage and can later feed Cranelift or LLVM.
- Differential tests (`tests/native.rs`) compare interpreter and native exit codes/stdout, making semantic drift visible.
- The subset boundary is explicit and testable, which is safer than a broad but incorrect backend.

## Consequences

- `vantyx build` requires a C compiler (`clang` by default; override with `VANTYX_CC`).
- Native builds reject `Any`, `List`, `Map`, indexing, and non-`range` loops for now. *(Superseded by the 2026-11-22 update below.)*
- Generated programs leak strings and structs on purpose; native processes are short-lived and this matches the interpreter's documented `Rc` cycle limitation. There is no native cycle collector.
- Native code has no call-depth guard yet; runaway recursion can exhaust the C stack. *(Superseded: a 256-frame guard was added.)*
- No debug information or optimization pipeline exists beyond the C compiler's `-O2`.
- The C backend is a bridge. It must not accrete language-specific semantics that belong in the IR.

## Reversibility

High. The IR is small and backend-neutral. A Cranelift or LLVM backend can be added behind the same IR; the C backend can remain as a bootstrap/debug path.

## Reconsideration criteria

Revisit when:

1. Native coverage of collections requires a dynamic runtime that is awkward to express in C.
2. Benchmark data shows C-backend compile time or runtime is a blocker.
3. A JIT or debug-build path is needed.
4. Cranelift/LLVM can be adopted without violating the dependency policy in ADR-0007.

## Update (2026-11-22)

The implementation has moved beyond the original subset description:

- `List[T]`, `Map[K, V]`, list/string indexing, map key iteration, and
  `for c in str` are now supported for statically known types (ADR-0009,
  ADR-0010). The rejection list is now `Any`, `List[Any]`, and `Map[Any, *]`.
- A native call-depth guard (`R0012`, limit 256) exists and is tested.
- Structural equality of lists, maps, and structs is cycle-safe.
- The backend remains intentionally leak-only and has no debug information.

The original decision, rationale, and C-backend boundary are unchanged.
