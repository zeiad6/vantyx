# Active Task

## T-055 — Shared ASCII character buffers for native string iteration and indexing

- Status: VERIFIED locally; commit pending
- Priority: P1
- Dependencies: T-022, T-044

### Implemented

- `src/codegen_c.rs`: added a lazily initialized static 128-entry ASCII table
  and `vantyx_str_slice`. Single-byte ASCII characters return a pointer into
  the table instead of allocating a 2-byte string. Multibyte UTF-8 characters
  still allocate through `vantyx_strndup`.
- `IrStmtKind::ForStr` and `vantyx_str_index` now use the shared helper.
- `tests/native.rs`: `string_ascii_indexing_iteration_and_concat_match` covers
  printable ASCII iteration, indexing, reversal, and repeated concatenation.

### Evidence

- `string_ops` native median: 3.190 ms (min 3.134, max 3.986) in the full
  six-workload run, down from 8.318 ms; C 2.796 ms and Rust 3.733 ms in the
  same run. An isolated run under lighter load measured 1.616 ms.
- Existing Unicode indexing/iteration and string control-flow differential
  tests still pass.
- Full 464-test gate, clippy, release build, and all five examples pass.

### Remaining

- Commit, push, and confirm CI.
- No code work remains for the acceptance criteria.
