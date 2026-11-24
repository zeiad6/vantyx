# Security Model

Auren treats the compiler, runtime, and generated code as security-sensitive infrastructure. This document states what is actually guaranteed today and what is not.

## Status (v0.1)

- Single-threaded interpreter and native runtime.
- No sandboxing. Do not execute untrusted Auren programs without OS-level isolation.
- No FFI yet, so there is no foreign-code trust boundary in the language.
- The compiler core has zero third-party Rust dependencies (ADR-0007).

## Compiler robustness

Expected properties, exercised by tests:

- The lexer and parser return diagnostics instead of panicking on malformed UTF-8 source.
- Parser recursion is bounded: expression nesting (`E0107`), AST depth (`E0108`), block nesting (`E0109`), type nesting (`E0110`), and interpolation nesting (`E0111`) all produce diagnostics instead of stack overflow.
- `SourceMap::line_col` clamps offsets to UTF-8 boundaries, so invalid spans cannot panic diagnostic rendering.
- The checker reports errors and does not execute user code.
- IR lowering rejects unsupported constructs with `E0401` instead of miscompiling them.
- Runtime failures become diagnostics with exit code 2; compile failures exit 1; usage errors exit 64.

Known limits:

- Fuzzing is reproducible but manual: `python3 tools/fuzz_smoke.py --binary target/debug/auren` is not yet wired into CI.
- Resource exhaustion (very large files, huge collections) is bounded by explicit limits where implemented and otherwise by available memory.
- The native backend invokes a C compiler discovered via `PATH` or `AUREN_CC`; that compiler is trusted.

The full audit record is in [security-audit-2026-11-22.md](security-audit-2026-11-22.md).

## REPL history

The REPL can persist submitted entries in `$AUREN_HISTORY` or `$HOME/.auren_history`. The file is created with mode 0600 on Unix, but it is plain text and may contain secrets typed into the REPL. Set `AUREN_HISTORY=off` to disable persistence. Interactive editing invokes `/bin/stty` or `/usr/bin/stty` from fixed paths; it does not search `PATH`, and failure falls back to canonical input. The editor is safe Rust and contains no `unsafe` block. With `panic = "abort"`, an unexpected panic while raw mode is active could leave the terminal in raw mode; `stty sane` restores it. See [ADR-0013](decisions/ADR-0013-repl-line-editing.md).

## Interpreter memory safety

The interpreter is written in safe Rust and uses `Rc<RefCell<...>>` for reference values. It prevents use-after-free, double-free, dangling references, out-of-bounds access, and uninitialized reads in Auren code. `Rc` cycles leak; this is a resource issue, not memory unsafety. There is no data-race risk because execution is single-threaded.

## Native backend safety

For the supported subset:

- Integer arithmetic, division, and range construction are checked.
- Uninitialized global reads raise `R0009`.
- Allocation failure terminates with a runtime error.
- Generated C uses libc/libm only.
- Native calls are limited to depth 256 (`R0012`).
- List and string indexes are bounds-checked; missing map keys raise `R0008`.
- List and map growth checks overflow and `SIZE_MAX` before allocating (`R0004`).
- Structural equality is cycle-safe and matches the interpreter's coinductive semantics.

Known native limits:

- Strings, structs, list buffers, map buffers, and enum values are intentionally leaked.
- `Any`, `List[Any]`, `Map[Any, *]`, `Any` enum payloads, `match` on `Any`, and unsupported loop forms are rejected with `E0401`.
- Native map lookup and insertion are O(n) (ADR-0010).
- No debug information, stack traces, or source-level unwinding in native programs.

## CLI boundary

- `auren build` writes generated C to a temporary file and invokes the C compiler with argument vectors (no shell interpolation).
- `AUREN_CC` selects the compiler executable; it is trusted input from the environment.
- Output paths are taken from the user; `auren` does not sandbox filesystem access.

## Supply chain

- No crates are used by the compiler or runtime.
- A future dependency (for example Cranelift/LLVM bindings) requires an ADR covering necessity, license, maintenance, and supply-chain risk (ADR-0007).
- The C compiler and system libc are external trusted tools.

## Reporting

Security findings should include:

1. Auren source or CLI invocation that reproduces the issue.
2. Observed behavior and expected behavior.
3. Whether the issue affects the compiler, interpreter, native backend, or tooling.
4. Severity and a minimal fix suggestion.

Do not claim a sandbox, memory-safety guarantee, or FFI safety property that has not been implemented and tested.
