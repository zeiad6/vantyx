# ADR-0005: Reference semantics for collections in v0.1

- Status: Accepted (provisional)
- Date: 2026-11-21

## Context
Vantyx needs collections now. Rust-style ownership is too heavy for the first release; copying every value is expensive; reference semantics is easy but has aliasing surprises and cycles.

## Options
1. Rust-style ownership/borrowing for all values.
2. Reference counting with reference semantics (`Rc<RefCell>`).
3. Mutable value semantics / copy-on-write.
4. Tracing garbage collection.

## Decision
Use `Rc<RefCell>` reference semantics for `List`, `Map`, and structs in v0.1. Scalars and `Str` are immutable values. Document aliasing and leak limitations. Plan escape analysis and optional ownership as an advanced layer.

## Rationale
- Enables a complete language quickly without a GC runtime.
- Memory safety is guaranteed by Rust even with aliasing.
- No mandatory lifetime syntax for beginners.
- `Rc` is deterministic and easy to reason about for acyclic data.

## Consequences
- Cycles leak.
- Aliasing can surprise users; docs and tests must be explicit.
- Thread safety is absent (single-threaded); concurrency requires a new model.
- Future value-semantics or ownership modes must interoperate without breaking existing code.

## Reversibility
Medium-high. Semantics are visible to users, but an opt-in value/ownership mode can be added alongside reference types.

## Reconsideration criteria
Before 1.0, decide the long-term default via user studies and benchmarks; cycles and aliasing are the main triggers.
