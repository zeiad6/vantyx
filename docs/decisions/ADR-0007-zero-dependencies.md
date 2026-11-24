# ADR-0007: Zero third-party dependencies in the compiler core

- Status: Accepted
- Date: 2026-11-21

## Context
The project runs on a 1 GiB machine and treats the compiler as security-sensitive infrastructure.

## Options
1. Use crates for CLI parsing, diagnostics, etc.
2. Zero dependencies for the core; add crates only with an ADR.
3. Vendor all dependencies.

## Decision
Option 2. The lexer, parser, checker, runtime, diagnostics, and CLI argument parsing are hand-written. Any future dependency requires an ADR documenting necessity, maintenance, license, and supply-chain risk.

## Rationale
- Fast, reproducible builds.
- Minimal supply-chain attack surface.
- No version churn.
- Forces clear, purpose-built implementations.

## Consequences
- More code to maintain (CLI parsing, rendering).
- Missing ecosystem conveniences.
- Backend crates (Cranelift/LLVM bindings) will require exceptions via ADR.

## Reversibility
High. Dependencies can be added per-component with tests.

## Reconsideration criteria
When a dependency clearly reduces security risk or enables a required capability (e.g., Cranelift), accept it with an ADR.
