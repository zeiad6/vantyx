# Active Task

## T-056 — Native enum construction and match benchmark

- Status: VERIFIED locally; commit pending
- Priority: P1
- Dependencies: T-022, T-030

### Implemented

- `benchmarks/enum_ops.vtx`: 200,000 iterations of deterministic enum
  construction (`Add`, `Mul`, `Neg`, `Keep`) and exhaustive payload matching.
- Equivalent `enum_ops.py`, `enum_ops.c`, and `enum_ops.rs`.
- `benchmarks/run.py`: `enum_ops` added to `WORKLOADS` and `DESCRIPTIONS`.
- Benchmark tables updated in `README.md`, `docs/benchmarks.md`,
  `PROJECT_STATUS.md`, and `docs/PROJECT_BASELINE.md`.

### Evidence

- All five implementations print `624732 204439`.
- Latest full run: native 3.646 ms (min 3.216, max 4.083), C 3.766 ms, Rust
  4.529 ms, Python 164.903 ms, interpreter 687.079 ms.
- Peak RSS: native 2.01 MiB, interpreter 3.72 MiB, C 1.60 MiB, Rust 2.09 MiB.
- A targeted run measured native 4.208 ms, C 3.557 ms, Rust 3.370 ms, showing
  the differences are within run-to-run noise.
- Conclusion: heap-allocated native enums are competitive with stack-value C
  and Rust enums on this workload; a representation rewrite is not justified
  by this evidence.

### Remaining

- Commit, push, and confirm CI.
- No code work remains for the acceptance criteria.
