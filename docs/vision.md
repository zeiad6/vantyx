# Vision

Vantyx exists to remove a false choice: languages are either easy to start or fast and safe in production. Vantyx aims to be both without hiding what the machine does.

## Who it serves

- **Beginners and prototypers** who want a script that runs immediately, with clear errors.
- **Application engineers** who want static guarantees without fighting a borrow checker for ordinary code.
- **Performance and systems engineers** who need explicit control, native code, and predictable memory behavior.
- **Scientific/AI engineers** who need arrays, SIMD, and GPU paths without abandoning the language.

## Principles in practice

| Principle | Concrete commitment |
| --- | --- |
| Simple by default | No required type annotations, no required `main`, indentation blocks |
| Safe by default | Checked arithmetic, bounds checks, no null, no uninitialized values, no user `unsafe` |
| Fast by design | Typed AST, explicit IR, native backend, measured optimizations |
| Complexity on demand | Optional annotations, ownership modes, FFI, low-level APIs |
| Progressive optimization | Add types and annotations later; compiler specializes what it knows |
| No hidden unpredictability | Documented evaluation order, entry rules, aliasing, and failure modes |
| Production-oriented | Tests, diagnostics, regression suites, security review, honest status |

## Ten-year direction

1. A trustworthy core language with excellent diagnostics.
2. A native compiler with predictable performance.
3. A standard library for data, text, files, networking, and numerics.
4. Safe concurrency and structured async.
5. Optional ownership/borrowing for zero-cost memory control.
6. Tensor/SIMD/GPU extensions as libraries, not core syntax.
7. Stable C ABI interop and careful Python interop.
8. Tooling: formatter, linter, package manager, LSP, debugger integration.

## What Vantyx will not do

- Claim memory safety that has not been verified.
- Hide allocations or copies behind magic syntax.
- Require advanced ownership syntax for simple programs.
- Become a Python or Rust clone; borrow ideas, keep an independent design.
- Sacrifice correctness for benchmark wins.
