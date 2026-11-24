# Vantyx — repository rules for autonomous agents

Vantyx is a gradual-typing language implemented in Rust with a tree-walking
interpreter, a native GNU C11 backend, and a dependency-free AI host toolkit.
Repository codename: RustZ.

## Non-negotiable rules

1. **The interpreter is the semantic oracle.** Native behavior must match it.
   Prove changes with differential tests in `tests/native.rs` (exit code and
   stdout).
2. **Never silently miscompile.** Unsupported native constructs must be
   rejected with `E0401` and a concrete help message.
3. **Zero third-party Rust dependencies.** Adding one requires an ADR. The
   native backend links only libc/libm and invokes the system C compiler. The
   AI host uses `std` plus an explicit HTTP/subprocess boundary (ADR-0015).
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
for f in examples/*.vtx; do ./target/debug/vantyx run "$f"; done
```

Native tests require `clang` (or `VANTYX_CC`) and skip when unavailable.
Optional, when the change touches parsing or performance:

```sh
python3 tools/fuzz_smoke.py --binary target/debug/vantyx
python3 benchmarks/run.py --reps 5
```

## Architecture map

| Path | Responsibility |
| --- | --- |
| `src/lexer.rs` | Indentation-aware lexer; spans; non-panicking recovery |
| `src/parser.rs` | Recursive-descent/Pratt parser; import syntax; depth guards |
| `src/module.rs` | File-based module loader, path confinement, cycles, declaration flattening |
| `src/checker.rs` | Scopes, gradual typing, enums, exhaustiveness |
| `src/interp.rs` | Reference tree-walking runtime |
| `src/copy_tree.rs` | Recursive directory copy, source containment, entry/depth caps (ADR-0034) |
| `src/glob.rs` | Lexical glob parser, matcher, and bounded walk (ADR-0033) |
| `src/ir.rs` | Typed, backend-neutral IR; `E0401` boundary |
| `src/codegen_c.rs` | GNU C11 backend and runtime helpers |
| `src/repl.rs` | Zero-dependency REPL editor, history, and completion (ADR-0013) |
| `src/ai/` | Dependency-free AI host: JSON, providers, schema, extraction, tools, agents, BM25/vector retrieval, RAG ingestion/search/evaluation, HTTP |
| `src/ai_cli.rs` | `vantyx ai` commands |
| `src/types.rs` | Semantic types and type-expression resolution |
| `src/value.rs` | Interpreter values, display, equality |
| `tests/e2e.rs` | Language-level end-to-end tests |
| `tests/native.rs` | Differential interpreter vs native tests |
| `tests/ai.rs` | Offline AI CLI and loopback HTTP integration tests |

## Current verified baseline (2026-11-24)

- Public repository: https://github.com/zeiad6/vantyx (MIT; renamed from
  `zeiad6/auren`; CI run 45365926013 green on `e3d8bdd` with 457 tests; a clean
  clone of the rewritten history passed 457 tests). The
  2026-11-24 history correction replaced the placeholder contributor identity
  and moved the `v0.1.0` tag.
- `cargo test`: 581 passed (326 library unit + 15 CLI unit + 32 AI integration +
  90 e2e + 118 native).
- Clippy clean; debug and release builds succeed.
- All five examples build and run natively with identical output.
- Native support: scalars, strings, `List[T]`, `Map[K,V]` with a scalar-key
  hash index, structs, enums, `match` with nested payload patterns, top-level
  or-patterns, integer range patterns and guards, imported modules, basic
  UTF-8 file I/O, file metadata probes, single-level directory operations,
  recursive directory creation and symlink-safe removal, lexical glob
  expansion, rename and byte-preserving file copy, recursive directory copy,
  UTF-8 text builtins, lexical path builtins, checked arithmetic, bounds
  checks, structural equality.
- Native rejects: `Any`, `List[Any]`, `Map[Any,*]`, `Any` enum payloads,
  `match` on `Any`, dynamically typed file/directory/glob/recursive-copy/
  rename/copy/metadata/text/path arguments, tuple/float patterns, unannotated
  value-returning functions.
- AI host support: offline mock/echo chat, OpenAI-compatible chat and model
  listing, streamed text chat with SSE and incremental HTTP/curl transport,
  strict JSON and JSON Schema subset, validated structured extraction with
  bounded repair, token estimates, budgets, embedding provider interface with
  batch limits, deterministic lexical hash embeddings and a bounded FIFO cache,
  OpenAI-compatible `/v1/embeddings`, in-memory vector index, BM25, local RAG
  ingestion/chunking/citations, lexical/semantic/hybrid retrieval, retrieval
  evaluation, typed tools with capabilities, bounded agents, redacted metrics,
  and `vantyx ai`. A live cloud OpenAI-compatible run on 2026-11-24 covered
  model discovery, chat, streaming, validated extraction, 768-dimension
  embeddings, semantic RAG/search, and retrieval evaluation; the 401/429/
  timeout paths are covered by a loopback harness. A later retry was rejected
  with HTTP 400 `User location is not supported`, and a re-verification at
  20:59Z using `MY_API_KEY` passed all 21 live checks after one transient 503,
  so cloud access can change and is not guaranteed. Evidence:
  `docs/ai/LIVE_VERIFICATION.md`.
- Known gaps: no reclamation, O(n) structural-key native maps and a fixed hash
  seed for scalar keys, no generics/closures/async/FFI, basic whole-file I/O,
  single-level and recursive directory operations, lexical glob expansion,
  rename/copy, recursive directory copy, metadata probes, text builtins, and
  lexical POSIX-style path helpers only (no escaping/braces/case folding in
  globs and no metadata objects or preservation, recursive move, streaming,
  atomic copy/write, recoverable errors, or sandbox; glob materializes sorted
  results under candidate/depth/component caps; recursive removal refuses
  final `.`/`..` and the filesystem root but still has no confirmation,
  rollback, trash, depth limit, or entry-count limit and intermediate links
  can still traverse;
  `copy_dir_all` requires a new target, rejects inner links and special files,
  leaves partial targets, avoids metadata preservation by design, and caps
  trees at 100,000 entries and depth 128), text builtins
  without regex/case/normalization/grapheme APIs, block-shaped
  `match`
  cannot appear inside parentheses, native or-pattern conditions re-test
  alternatives when initializing bindings,
  REPL history recall is single-line and completion is
  top-level-only, no live local-model evidence (the cloud OpenAI-compatible
  path was verified and re-verified on 2026-11-24, but access is not
  guaranteed), no
  streaming tool calls or interactive cancellation, no constrained
  decoding/model manager/keyring, no PDF/HTML/Office ingestion, or grounded
  generation, no language-level AI syntax,
  no tensors or GPU support.
- Attribution: the developer is **Zeidex** (`zeiad6`,
  https://zeidexlink.zeidexsoft.com/zeidex). The 2026-11-24 history correction
  replaced the historical placeholder identity `Auren Contributors` with
  `Zeidex <48607619+zeiad6@users.noreply.github.com>` for every reachable
  commit and moved the `v0.1.0` tag; `.mailmap` remains for old hashes. Do not
  credit any other contributor without verified work.

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
