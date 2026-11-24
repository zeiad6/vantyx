# Concurrency and Async

## Status (v0.1)

Concurrency is **not implemented**. Auren programs are single-threaded, and both execution engines assume one thread:

- The interpreter runs on one dedicated 16 MiB thread.
- Generated native programs are single-threaded.
- Runtime values (`Rc<RefCell<...>>`) are not `Send`/`Sync` and must not be shared across threads.

No async, tasks, channels, mutexes, atomics, or actors exist yet. The lexer reserves `async`/`await` as future keywords, but the parser and checker do not accept them.

## Why the delay

Concurrency semantics are hard to change later. Adding threads before deciding shared-state and cancellation rules would create architectural debt and unsound APIs. The interpreter's reference-counted values are deliberately non-atomic for single-threaded performance.

## Design constraints for Phase 9

Any future concurrency model must:

1. Prevent data races by construction, not by documentation.
2. Define how reference values (`List`, `Map`, structs) cross task boundaries.
3. Provide structured concurrency: child tasks cannot outlive their scope.
4. Define cancellation, error propagation, and cleanup behavior.
5. Keep ordinary single-threaded code unchanged.
6. Document whether shared state uses ownership transfer, immutable sharing, locks, or message passing.

## Candidate direction (not a decision)

- `Send`-like transfer for values that leave a task; no shared mutable references by default.
- Message passing (`Channel[T]`) as the default cross-task mechanism.
- `Mutex[T]`/`Atomic[T]` as explicit opt-in shared-state types with documented ordering.
- Async tasks scheduled cooperatively; blocking operations isolated behind explicit APIs.

## Required tests before claiming support

- Task cancellation at await points and during cleanup.
- Error propagation from child tasks.
- Shared-state race tests under a sanitizer or deterministic scheduler.
- Deadlock scenarios and timeout behavior.
- Structured-concurrency scope exit (no orphan tasks).
- Cross-platform behavior (Linux first, then macOS/Windows).

No concurrency claim is valid until these tests exist and pass.
