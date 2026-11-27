---
name: Runtime and Memory Safety
description: Audit and change ownership, borrowing, escape analysis, allocation, reference counting, garbage collection, cycles, unsafe Rust, and resource bounds with sanitizers, stress tests, and measurable evidence.
---

# Runtime and Memory Safety

## When to use

- Changing ownership, borrowing, lifetimes, aliasing, drop order, allocation, or reclamation.
- Investigating leaks, use-after-free, double-free, buffer overruns, cycles, or unbounded memory growth.
- Adding or reviewing `unsafe` Rust, FFI, raw pointers, arenas, reference counting, or a garbage collector.
- Evaluating concurrency safety, thread-local storage, or resource limits.

Do not use this skill for a pure performance change that does not affect memory ownership; use `performance-benchmarking` instead.

## Required tools

- Rust: `cargo`, `rustc`, `clippy`, `miri`, `cargo-fuzz`, `loom` for concurrency models.
- C/C++: AddressSanitizer, UndefinedBehaviorSanitizer, ThreadSanitizer, `valgrind`, `gdb`/`lldb`.
- Memory measurement: `/usr/bin/time -v`, `heaptrack`, `massif`, `tracemalloc`, `dhat`, RSS sampling.
- Optional: `rr`, `perf`, `strace`, allocation counters, custom stress harnesses.
- The project's semantic oracle and test suite.

## Safety constraints

- Never add `unsafe` without a written safety invariant covering aliasing, initialization, bounds, lifetime, and thread safety.
- Do not change observable semantics to fix a memory bug. Preserve evaluation order, drop order, error behavior, and output.
- Bound every new allocation, recursion, cache, queue, and traversal. Document why a bound is unnecessary if you omit it.
- Do not run destructive stress tests against production data or shared systems.
- Distinguish intentional leaks or caches from defects; record the project's documented memory policy.
- Do not suppress sanitizer findings without a minimized explanation and a regression test.

## Workflow

1. **State the memory model.** Write the invariants:
   - who owns each value and when it is freed;
   - aliasing and mutation rules;
   - lifetime/escape rules;
   - reclamation strategy (RAII, reference counting, tracing GC, arena, manual);
   - cycle policy;
   - thread-safety and synchronization;
   - resource limits.
   Verify the model against the source, not the docs.

2. **Inventory every representation.** Values may exist as Rust types, FFI structs, generated code, serialized forms, and runtime handles. Map each representation and its ownership transfer points: assignment, argument passing, collection insertion, return, drop, and error unwinding.

3. **Establish a baseline.**
   ```sh
   cargo test --workspace
   cargo clippy --workspace --all-targets -- -D warnings
   cargo +nightly miri test
   /usr/bin/time -v ./target/debug/binary --small-workload
   ```
   Record RSS, peak allocation, test results, and tool versions. If a tool is unavailable, say so.

4. **Reproduce the suspected issue minimally.** Create a focused test or program. Capture:
   - exact command, input, and environment;
   - exit code, stderr, backtrace (`RUST_BACKTRACE=full`);
   - sanitizer or Miri output;
   - the allocation/free site and responsible `file:line`.
   Classify: leak, use-after-free, double-free, out-of-bounds, uninitialized read, overflow, race, cycle, or unbounded growth.

5. **Fix at the correct layer.** Prefer safe abstractions and RAII. Options include:
   - change ownership or borrow instead of cloning;
   - use `Option`/`NonNull` invariants with a safety comment;
   - add a guard/drop implementation;
   - use an arena for bulk lifetime;
   - add reference counting or a cycle collector;
   - cap recursion, allocation, or cache size.
   Do not paper over a lifetime bug with `mem::forget` or a global.

6. **Add regression and stress tests.**
   - aliasing: two names for one collection, mutation through an alias;
   - cycles: self-reference, mutual reference, display/equality/hash;
   - drop order and panic during drop;
   - empty, single-element, and maximum-size collections;
   - out-of-bounds and overflow;
   - repeated allocation/free loops;
   - deep nesting and recursion limits;
   - concurrency races if applicable.

7. **Validate with independent tools.**
   ```sh
   cargo +nightly miri test
   RUSTFLAGS="-Zsanitizer=address" cargo +nightly test -Zbuild-std --target x86_64-unknown-linux-gnu
   valgrind --leak-check=full --error-exitcode=1 ./target/debug/binary
   ```
   Use the tools that fit the language and platform; record exclusions and false positives.

8. **Measure resource behavior.** Compare RSS and allocation counts before/after under the same workload. Distinguish steady-state from per-iteration growth. For GC/RC, measure pause time, throughput, and memory overhead.

9. **Review unsafe and FFI boundaries.** For every `unsafe` block verify pointer validity, alignment, initialization, aliasing, lifetime, panic safety, and thread safety. For FFI, verify ownership transfer and error handling on both sides.

10. **Document limits and failure modes.** Update the memory-model documentation, known gaps, and any ADR. State what is intentionally leaked, cached, or unsupported.

## Validation and acceptance criteria

- The memory model and invariants are written and match the implementation.
- A minimal reproduction exists for every fixed defect, and it is now a regression test.
- Aliasing, mutation, drop, cycle, bounds, and overflow behavior match the semantic oracle.
- No new `unsafe` without a safety comment and reviewer approval.
- Sanitizers/Miri/Valgrind are clean for the tested scope, or exceptions are documented with evidence.
- New traversals and allocations have a bound or a written justification.
- Memory measurements use a stated method and show no unintended per-iteration growth.
- The full test suite and relevant stress tests pass.

## Expected output

1. Memory-model note: ownership, aliasing, reclamation, cycles, limits, threads.
2. Failing reproduction and classification.
3. Fix with a clear invariant and minimal diff.
4. Regression and stress tests.
5. Validation matrix: tool, command, scope, result, exclusions.
6. Memory measurements before/after, with methodology.
7. Updated documentation and residual risks.

## Troubleshooting

- **Miri cannot run FFI or unsupported intrinsics:** isolate the safe portion, document the gap, and use sanitizers or a native harness for the rest.
- **Leak is intentional:** verify the documented policy and measure per-iteration growth; do not report a bounded cache as a defect.
- **Reference-count cycle:** add a cycle-breaking policy, weak references, or a collector; test display, equality, and drop.
- **Drop order differs between engines:** trace assignment, argument passing, and collection insertion; fix the representation, not the test.
- **Sanitizer reports a false positive:** minimize, verify the allocation lifetime, and document the suppression with a regression test.
- **Stress test OOMs the machine:** cap the workload, measure growth per iteration, and run in a memory-limited cgroup.
- **Race only under load:** use `loom`, ThreadSanitizer, or a deterministic scheduler; record the interleaving.

## Examples

```sh
# Reproduce and classify
RUST_BACKTRACE=full cargo test memory_regression -- --nocapture
cargo +nightly miri test memory_regression

# Native leak check
cargo build
valgrind --leak-check=full --show-leak-kinds=all ./target/debug/app --tiny

# Measure growth
/usr/bin/time -v ./target/release/app --iterations 100000 2>&1 | rg 'Maximum resident|Elapsed'

# Unsafe inventory
rg -n 'unsafe' src
```

## Related skills and review

- Pair with `security-audit-engineering` for attacker-controlled memory corruption and FFI risk.
- Pair with `performance-benchmarking` for allocator, GC, or cache changes that claim speed or memory gains.
- Pair with `compiler-engineering` when ownership or drop semantics cross a compiler stage.
- Reviewer checks: invariant documented, reproduction stable, unsafe justified, sanitizer scope honest, and no semantic drift.
