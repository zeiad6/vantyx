# Security Audit — 2026-11-22

Scope: Vantyx v0.1 compiler front end, interpreter, native C backend, and CLI
build path. Method: source review, targeted stress inputs, a reproducible
malformed-input fuzz harness, and the full test suite. No third-party security
tooling was used.

## Summary

| ID | Severity | Finding | Status |
| --- | --- | --- | --- |
| A-001 | High | Deep expressions and blocks crashed the parser with a Rust stack overflow | Fixed, regression-tested |
| A-002 | Medium | `SourceMap::line_col` could slice a UTF-8 code point and panic | Fixed, regression-tested |
| A-003 | Medium | Native structural equality disagreed with the interpreter on cyclic values | Fixed, regression-tested |
| A-004 | Low | Docs claimed the native backend had no call-depth guard | Fixed in docs; guard and test already existed |
| A-005 | Low | Native strings, structs, lists, and maps are intentionally leaked | Accepted limitation, documented |
| A-006 | Low | Interpreter `Rc` cycles are never reclaimed | Accepted limitation, documented |
| A-007 | Low | Native map lookup is O(n); adversarial workloads can be quadratic | Accepted, bounded by process memory/time |
| A-008 | Low | 32-bit allocation-size truncation was possible in generated C | Fixed with `SIZE_MAX` checks |
| A-009 | Info | Native temp C file handling and string escaping | Verified safe |
| A-010 | Info | Native bounds, missing-key, capacity, and range checks | Verified |
| A-011 | Info | `VANTYX_CC`, the C compiler, and libc are trusted boundaries | Documented |
| A-012 | Info | Mutating a map key after insertion makes it unreachable | Documented semantic limitation |

No memory-unsafety, use-after-free, double-free, or code-injection issue was
found in the supported subset. Vantyx is **not** a sandbox; untrusted programs
still require OS-level isolation.

## A-001 — Parser stack exhaustion (High, fixed)

Before the fix, a long binary chain or deeply nested block aborted the process
with `thread 'main' has overflowed its stack` instead of returning a diagnostic.

Reproductions (pre-fix):

```sh
python3 - <<'PY' > /tmp/chain.vtx
print("fn main():")
print("    let x = " + "+".join(["1"] * 10_000))
PY
target/debug/vantyx check /tmp/chain.vtx
```

```sh
python3 - <<'PY' > /tmp/nest.vtx
print("fn main():")
for i in range(2_000):
    print("    " * (i + 1) + "if true:")
print("    " * 2_001 + 'print("x")')
PY
target/debug/vantyx check /tmp/nest.vtx
```

Fix: bounded parser recursion and AST depth in `src/parser.rs`:

- `MAX_EXPR_DEPTH = 256` (line 9) with `E0107`.
- `MAX_AST_DEPTH = 256` (line 12) with `E0108`; checked at the end of
  `parse_expr`.
- `MAX_BLOCK_DEPTH = 256` (line 15) with `E0109`; checked in `parse_block`.
- `MAX_TYPE_DEPTH = 64` (line 17) with `E0110`; checked in `parse_type`.
- `MAX_INTERP_DEPTH = 32` (line 19) with `E0111`; defensive interpolation
  guard.

Tests: `long_left_deep_chains_are_rejected_without_overflow`,
`deeply_nested_blocks_are_rejected_without_overflow`,
`deeply_nested_types_are_rejected_without_overflow`,
`deeply_nested_expressions_do_not_overflow`.

## A-002 — UTF-8 boundary panic (Medium, fixed)

`SourceMap::line_col` previously used a byte offset directly as a string slice
boundary. An offset inside a multi-byte character panicked. It now clamps to the
previous character boundary (`src/span.rs:74-90`).

Test: `offsets_inside_multibyte_characters_are_safe`.

## A-003 — Cyclic structural equality divergence (Medium, fixed)

Native equality returned `false` after its depth limit for two distinct but
structurally equal cycles, while the interpreter returned `true`. Reproduction:

```vantyx
struct Node:
    next: Map[Str, Node]

fn main():
    var m1: Map[Str, Node] = {}
    var m2: Map[Str, Node] = {}
    m1["self"] = Node(next=m1)
    m2["self"] = Node(next=m2)
    print(m1 == m2)   # interpreter: true; native before fix: false
```

Fix: the generated runtime tracks active `(left, right, descriptor)` container
pairs and treats a revisited pair as equal, matching the interpreter's
coinductive `seen` set (`src/codegen_c.rs`, `vantyx_eq_frames`). The depth limit
remains as a stack guard.

Tests: `cyclic_structural_equality_matches`, `struct_equality_matches`.

## A-004 — Stale call-depth claim (Low, fixed)

`docs/security.md` and `PROJECT_STATUS.md` said the native backend had no
call-depth guard. The backend already emits `vantyx_call_depth >= 256` with
`R0012` (`src/codegen_c.rs:489`). Documentation is corrected; the existing
`native_call_depth_limit_exits_two` test verifies the behavior.

## A-005 — Native allocation leaks (Low, accepted)

Native strings, structs, list buffers, and map buffers are allocated with
`malloc`/`realloc` and never freed. This is a resource-exhaustion risk for
long-running native processes, not memory unsafety. It is documented in
`docs/memory_model.md` and ADR-0005/0008/0009/0010. Reclamation requires a
memory-management ADR before implementation.

## A-006 — Interpreter cycle leaks (Low, accepted)

`Rc<RefCell<...>>` reference cycles in lists, maps, or structs are never
reclaimed. Again a resource issue, not unsafety; documented in
`docs/memory_model.md`.

## A-007 — Linear map lookup (Low, accepted)

ADR-0010 chooses insertion-ordered parallel arrays with linear search for
semantic parity. Lookup, insertion, and `contains` are O(n), so adversarial
programs can spend quadratic time. There is no out-of-bounds access or type
confusion; the risk is CPU exhaustion only. The runtime API can later be backed
by an ordered hash index without changing the language.

## A-008 — 32-bit allocation truncation (Low, fixed)

Generated list and map growth checked `cap * elem_size` against `INT64_MAX` but
not against `SIZE_MAX`. On a 32-bit target the product could truncate when cast
to `size_t`. Both reserve functions now also check `SIZE_MAX`
(`src/codegen_c.rs`, `vantyx_list_reserve` and `vantyx_map_reserve`).

## A-009 — Temp files and escaping (Info, verified)

- Generated C is written with `create_new(true)` and mode `0o600` on Unix, so a
  local attacker cannot pre-create or follow a symlink
  (`src/main.rs:246-264`).
- The C compiler is invoked with an argument vector, never a shell.
- String literals and source paths pass through `c_string`, which escapes
  backslash, quote, newline, carriage return, tab, and every non-printable byte
  as an octal escape (`src/codegen_c.rs:1576-1593`).
- The temporary file is removed after a successful build; it is retained with
  its path reported on failure for debugging, still mode `0600`.

Test: `generated_c_temp_file_is_owner_only`.

## A-010 — Native runtime checks (Info, verified)

- List and string indexes are bounds-checked; negative indexes raise `R0003`
  (`vantyx_list_slot`, `vantyx_str_index`).
- Missing map keys raise `R0008`.
- List and map capacity growth checks overflow and `SIZE_MAX`, raising `R0004`.
- `range` rejects zero steps and caps output at 10,000,000 elements.
- Integer arithmetic checks overflow and division by zero.
- Uninitialized global reads raise `R0009`.
- Allocation failure exits with status 2.

Differential tests cover bounds errors, negative indexes, missing keys, division
by zero, overflow, range limits, and call-depth limits.

## A-011 — Trusted boundaries (Info)

`VANTYX_CC` selects an arbitrary compiler executable and is trusted input from
the environment. The selected C compiler, system libc, and libm are trusted.
`vantyx build` writes the output path chosen by the user and does not sandbox
filesystem access. Executing untrusted Vantyx source is unsafe without OS-level
isolation; Vantyx provides no sandbox.

## A-012 — Mutable map keys (Info)

Map keys are compared structurally. Mutating the contents of a list or struct
used as a key after insertion can make the entry unreachable, exactly as in the
interpreter. This is documented in ADR-0010; it is not memory unsafety.

## Verification

Performed on 2026-11-22:

```sh
cargo build
cargo clippy --all-targets -- -D warnings
cargo test
python3 tools/fuzz_smoke.py --binary target/debug/vantyx --iterations 400
```

Results:

- `cargo test`: 90 unit + 22 end-to-end + 54 native differential tests passed,
  0 failed.
- `cargo clippy --all-targets -- -D warnings`: clean.
- `tools/fuzz_smoke.py`: 9 depth/limit cases and 400 random/mutated inputs
  produced controlled diagnostics; no panic, signal, or timeout.
- Native differential tests require `clang` (or `VANTYX_CC`) and skip when no C
  compiler is available.

## Residual risks

- The fuzz harness was manual at audit time. Update 2026-11-27 (T-110): the
  bounded fuzz smoke now runs in CI, and CI runs the native suite with
  generated programs compiled under ASan/UBSan. The harness remains
  deterministic and bounded, not coverage-guided; vary `--seed` for broader
  coverage.
- Native memory is not reclaimed; long-running native programs can exhaust
  memory.
- Native maps are O(n) per operation.
- No sandbox, debugger, stack traces, or source-level unwinding in native
  programs.
- The supported native subset is deliberately smaller than the interpreter;
  unsupported constructs must fail with `E0401`, never be miscompiled. New
  native features require differential tests before documentation claims them.
