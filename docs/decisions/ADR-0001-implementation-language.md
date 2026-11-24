# ADR-0001: Implement the compiler in Rust

- Status: Accepted
- Date: 2026-11-21

## Context
Auren needs a compiler/runtime that is memory-safe, fast, maintainable, and able to grow into native code generation and concurrency.

## Options
1. Rust — memory safety without GC, strong tooling, Cranelift/LLVM bindings, growing compiler ecosystem.
2. C++ — mature compiler ecosystem, but manual memory safety and slower safe iteration.
3. Python — fastest prototyping, unacceptable runtime performance for a compiler long-term.
4. Self-hosted Auren — ideal end state, impossible before the language exists.

## Decision
Implement the compiler and runtime in Rust (2024 edition), with no third-party runtime dependencies for the core.

## Consequences
- Memory safety of the compiler itself; fewer security defects.
- Fast, reliable builds on modest hardware.
- Easy future integration with Cranelift (Rust) and C ABI.
- Contributors need Rust knowledge.
- Bootstrapping (self-hosting) is deferred until the language is mature.

## Reversibility
Medium. Front-end logic is portable; a rewrite would be costly but the specification and tests remain valid.

## Reconsideration criteria
Revisit only if Rust blocks a required platform or performance target, or when self-hosting becomes feasible and beneficial.
