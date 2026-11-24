# Auren — repository rules for autonomous agents

Auren is a gradual-typing language implemented in Rust with a tree-walking
interpreter and a native GNU C11 backend. Repository codename: RustZ.

## Non-negotiable rules

1. **The interpreter is the semantic oracle.** Native behavior must match it.
   Prove changes with differential tests in `tests/native.rs` (exit code and
   stdout).
2. **Never silently miscompile.** Unsupported native constructs must be
   rejected with `E0401` and a concrete help message.
3. **Zero third-party Rust dependencies.** Adding one requires an ADR. The
   native backend links only libc/libm and invokes the system C compiler.
4. **Update status in the same change.** Any task that changes behavior must
   update `TASKS.md` and `PROJECT_STATUS.md`, and the language specification or
   ADR if semantics changed. `VERIFIED` requires an observed command and date.
5. **Do not claim performance without measurements.** Record methodology and
   raw results under `benchmarks/results/`.
6. **Preserve working systems.** Inspect first, make the smallest effective
   change, run the full suite, and do not delete user work.
7. **No destructive Git operations.** Do not force push, rewrite history, or
   commit unless explicitly authorized.
8. **Document limits honestly.** Native allocations are intentionally leaked in
   v0.1; there is no cycle collector and no sandbox.

## Verification commands

Run before reporting completion:

```sh
cargo build
cargo build --release
cargo clippy --all-targets -- -D warnings
cargo test
for f in examples/*.auren; do ./target/debug/auren run "$f"; done
```

Native tests require `clang` (or `AUREN_CC`) and skip when unavailable.
Optional, when the change touches parsing or performance:

```sh
python3 tools/fuzz_smoke.py --binary target/debug/auren
python3 benchmarks/run.py --reps 5
```

## Architecture map

| Path | Responsibility |
| --- | --- |
| `src/lexer.rs` | Indentation-aware lexer; spans; non-panicking recovery |
| `src/parser.rs` | Recursive-descent/Pratt parser; depth guards |
| `src/checker.rs` | Scopes, gradual typing, enums, exhaustiveness |
| `src/interp.rs` | Reference tree-walking runtime |
| `src/ir.rs` | Typed, backend-neutral IR; `E0401` boundary |
| `src/codegen_c.rs` | GNU C11 backend and runtime helpers |
| `src/repl.rs` | Zero-dependency REPL editor, history, and completion (ADR-0013) |
| `src/types.rs` | Semantic types and type-expression resolution |
| `src/value.rs` | Interpreter values, display, equality |
| `tests/e2e.rs` | Language-level end-to-end tests |
| `tests/native.rs` | Differential interpreter vs native tests |

## Current verified baseline (2026-11-23)

- Public repository: https://github.com/zeiad6/auren (MIT; CI green on `main`).
- `cargo test`: 241 passed (126 library unit + 7 CLI unit + 41 e2e + 67 native).
- Clippy clean; debug and release builds succeed.
- All five examples build and run natively with identical output.
- Native support: scalars, strings, `List[T]`, `Map[K,V]`, structs, enums,
  `match` with guards, checked arithmetic, bounds checks, structural equality.
- Native rejects: `Any`, `List[Any]`, `Map[Any,*]`, `Any` enum payloads,
  `match` on `Any`, unannotated value-returning functions.
- Known gaps: no reclamation, O(n) native maps, no modules/generics/closures/
  async/FFI, block-shaped `match` cannot appear inside parentheses, REPL history
  recall is single-line and completion is top-level-only.

## Definition of done

A task is done only when it has a specification, a real implementation, tests
that exercise it, a clean build and clippy run, updated documentation, and an
accurate status entry. A feature that only compiles or only has docs is not
done.

## Subagents

Specialized agents live in `.opencode/agents/`. Use `spec-sync-auditor` after
large changes to detect documentation drift; use `qa-verifier` for independent
verification; use `compiler-architect` for read-only design review. The
interpreter remains the oracle regardless of which agent writes code.
