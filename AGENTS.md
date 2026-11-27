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
python3 tools/test_differential_fuzz.py
python3 tools/differential_fuzz.py --iterations 100 --seed 1
for f in examples/*.vtx; do ./target/debug/vantyx run "$f"; done
```

Native tests require `clang` (or `VANTYX_CC`) and skip when unavailable;
the differential fuzzer skips the same way.
Optional, when the change touches parsing, diagnostics, or performance:

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
| `src/checker.rs` | Scopes, gradual typing, enums, exhaustiveness, reachability-aware return checking |
| `src/interp.rs` | Reference tree-walking runtime |
| `src/lint.rs` | Read-only static lints over the checked AST (`L0001`–`L0004`, ADR-0037) |
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

## Current verified baseline (2026-11-26)

- Public repository: https://github.com/zeiad6/vantyx (MIT; renamed from
  `zeiad6/auren`; CI run 45365926013 green on `e3d8bdd` with 457 tests; a clean
  clone of the rewritten history passed 457 tests; CI run 45488725812 passed
  on `4982ab3` with 581 tests after run 45488152955 failed only a nonportable
  hard-link test that `4982ab3` fixed; the status documentation commit
  `0491c63` also passed run 45489360135; run 45493833079 passed on `a4fe86b`
  with 584 tests, run 45496041422 passed on `5e1092f` with 613 tests, and run
  45499019615 passed on `53a8b82` with the expanded fuzzer, and run
  45503321172 passed on `66b4efc` with T-075, run 45504177763 passed on
  `de0bac7` with the rustfmt gate, run 45507195701 passed on `1aba7ba`
  with T-077, and run 45920832199 passed on `33c966e` with T-078–T-100 and the
  skill validator, run 45923211883 passed on `c54c61c` with T-101, run
  45923860569 passed on `5d5655b`, T-102 label-stratified splits are pushed
  in `a331f2d` with the 718-test gate and CI run 45925745650 green, T-103
  deterministic scalar-list sorting is pushed in `46b7d54` with the 723-test
  gate and CI run 45928466694 green, and T-104 call-site depth-limit
  diagnostics are pushed in `4269010` with the 723-test gate and CI run
  45933776137 green). The
  2026-11-24 history correction replaced the placeholder contributor identity
  and moved the `v0.1.0` tag.
- `cargo test`: 723 passed (402 library unit + 15 CLI unit + 47 AI integration +
  117 e2e + 142 native); the full native suite also passes with `VANTYX_CC=gcc`.
- Clippy clean; `cargo fmt --check --all` clean; debug and release builds succeed.
- All five examples build and run natively with identical output.
- Native condition and short-circuit lowering match the interpreter:
  statement-expression conditions are parenthesized and `and`/`or` skip the
  right operand when the left decides the result (T-070/T-071, ADR-0036).
  `tools/differential_fuzz.py` found both defects and is part of the gate
  (T-072). It now generates ten program shapes (scalars/control flow,
  collections, pattern matching, nested aliasing, typed functions, strings,
  numerics, result-enum `?` propagation, runtime-error agreement, and
  loop-heavy control flow with `break`/`continue`, match statements, map
  iteration, early returns, and guard side effects added by T-077); 180/180
  cases agreed on each of seeds 1, 500, and 1000 with nine shapes, the tenth
  shape matched 20/20 generated programs on seeds 1–20, and the full ten-shape
  run agreed 200/200 on seeds 1, 2, 3, 500, and 1000. `--shrink` performs
  bounded line-oriented minimization, and `tools/test_differential_fuzz.py`
  covers the comparison and shrink logic. The numeric and runtime-error
  shapes include accepted and rejected numeric-string conversions after T-078.
  Native `print` now evaluates every argument into a temporary before rendering
  and writing, and float `min`/`max` use one explicit tie/NaN rule in both
  engines (T-079, ADR-0039). Native `type` now evaluates its argument before
  yielding the type name, so calls, output, mutation, and runtime errors inside
  the argument match the interpreter (T-080); the fuzzer covers both the
  side-effecting success path and a failing argument. Native string `contains`
  binds both operands in source order so GCC and clang agree (T-081), and
  `Unit` equality and `Unit` interpolation still evaluate their expressions
  (T-082/T-083); the fuzzer covers all three. CI runs the self-tests
  and 80 iterations on seed 1.
- Tooling: `vantyx lint` reports `L0001` unused binding, `L0002` `var` never
  reassigned, `L0003` unreachable statement, and `L0004` unused parameter, with
  compile errors still exiting `1` and lint-only findings exiting `0` (T-073,
  T-085, ADR-0037). The checker no longer reports a false `E0309` for dead code
  after a reachable `return` (T-074, ADR-0037).
- Native support: scalars, strings, `List[T]`, `Map[K,V]` with a scalar-key
  hash index, structs, enums, `match` with nested payload patterns, top-level
  or-patterns, integer range patterns and guards, calls and short-circuit
  expressions in `if`/`elif`/`while` conditions, statement-position result-enum
  `?` propagation, `range(...)` as a `List[Int]` value outside `for` loops
  (T-099), strict numeric-string conversion matching the interpreter
  (T-078), deterministic two-phase `print` evaluation and float `min`/`max`
  tie/NaN semantics (T-079, ADR-0039), argument evaluation in `type` (T-080),
  source-order evaluation of named constructor arguments (T-088), lexical
  function frames where a callee resolves names in its own frame and
  the top-level bindings, never caller locals (T-087, ADR-0040), matching
  first-line runtime diagnostics for float-to-int conversion, missing map keys,
  `range` limits, and file/directory failures, including Rust's `(os error N)`
  filesystem suffix and the shared glob parser wording (T-096, T-097),
  call-site `R0012` depth-limit locations that match the interpreter across
  modules (T-104), imported
  modules, basic
  UTF-8 file I/O, file metadata probes, single-level directory operations,
  recursive directory creation and symlink-safe removal, lexical glob
  expansion, rename and byte-preserving file copy, recursive directory copy,
  UTF-8 text builtins, lexical path builtins whose trailing-dot stem and
  extension behavior matches the interpreter (T-086), deterministic
  scalar-list sorting over a stable merge sort for `Int`/`Float`/`Str`/`Bool`
  (T-103, ADR-0050), checked arithmetic,
  bounds checks, structural equality.
- Native rejects: `Any`, `List[Any]`, `Map[Any,*]`, `Any` enum payloads,
  `match` on `Any`, dynamically typed file/directory/glob/recursive-copy/
  rename/copy/metadata/text/path arguments, tuple/float patterns, unannotated
  value-returning functions. Same-file `copy_file` targets are refused with
  `R0015` in both engines before the target is opened, including symlink and
  hard-link aliases (T-089).
- AI host support: offline mock/echo chat, OpenAI-compatible chat and model
  listing, streamed text chat with SSE and incremental HTTP/curl transport,
  strict JSON and JSON Schema subset, validated structured extraction with
  bounded repair, token estimates, budgets, embedding provider interface with
  batch limits, deterministic lexical hash embeddings and a bounded FIFO cache,
  OpenAI-compatible `/v1/embeddings`, in-memory vector index, BM25, local RAG
  ingestion/chunking/citations, lexical/semantic/hybrid retrieval, retrieval
  evaluation, typed tools with capabilities, bounded agents, redacted metrics,
  offline GGUF v2/v3 and Safetensors inspection with tensor-range validation,
  Safetensors shard-index aggregation with index/header cross-checks and
  per-shard SHA-256 provenance (T-091, ADR-0042), bounded JSONL dataset
  validation with sorted field/type summaries and a streamed SHA-256 (T-092,
  ADR-0043), optional dataset validation against the supported JSON Schema
  subset with numbered violations (T-094, ADR-0044), deterministic train/
  held-out splits with seeded content-hash assignment, validation gates,
  overwrite refusal, and output digests (T-098, ADR-0046), exact byte-based
  duplicate-record removal with validation gates, overwrite refusal, a
  1,000,000-record digest cap, and input/output provenance (T-100, ADR-0047),
  deterministic seeded shuffling that orders records by a domain-separated
  SHA-256 of the seed and record bytes under 64 MiB/1,000,000-record in-memory
  caps, with validation gates, overwrite refusal, and input/output provenance
  (T-101, ADR-0048), label-stratified splits that group by a top-level scalar
  label and assign exact per-label counts under 64 MiB/1,000,000-record/
  1,000-label/1,024-byte caps with a deterministic non-empty fallback (T-102,
  ADR-0049), and `vantyx ai`. A live cloud OpenAI-compatible run on 2026-11-24 covered
  model discovery, chat, streaming, validated extraction, 768-dimension
  embeddings, semantic RAG/search, and retrieval evaluation; the 401/429/
  timeout paths are covered by a loopback harness. A later retry was rejected
  with HTTP 400 `User location is not supported`, and a re-verification at
  20:59Z using `MY_API_KEY` passed all 21 live checks after one transient 503,
  so cloud access can change and is not guaranteed. Evidence:
  `docs/ai/LIVE_VERIFICATION.md`.
- Known gaps: no reclamation, O(n) structural-key native maps and a fixed hash
  seed for scalar keys, no generics/closures/async/FFI, no formatter or LSP,
  the linter has only four conservative rules with no suppression or fixes,
  basic whole-file I/O,
  single-level and recursive directory operations, lexical glob expansion,
  rename/copy, recursive directory copy, metadata probes, text builtins,
  deterministic scalar-list sorting (scalar element types only; no comparator,
  key, descending flag, in-place variant, deduplication, or structural
  ordering), and
  lexical POSIX-style path helpers only (no escaping/braces/case folding in
  globs and no metadata objects or preservation, recursive move, streaming,
  atomic copy/write, error conversion or expression-position `?`, or sandbox; glob materializes sorted
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
  generation, dataset inspection is JSONL-only and does not tokenize or
  implement full JSON Schema; label-stratified splits use one top-level scalar
  field, one label per record, and in-memory grouping under 64 MiB/1,000,000
  records/1,000 labels (no weights, token-aware balancing, or external-memory
  grouping); deduplication is exact-byte only (no
  JSON canonicalization or near-duplicate detection), splits are deterministic
  but not shuffled, and shuffling holds the dataset in memory under 64 MiB and
  1,000,000 records (no external-memory sort), no language-level AI
  syntax, no tensors or GPU support.
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

Specialized agents live in `.opencode/agents/`. Reusable workflows live in
`.opencode/skills/` and are listed in `docs/AGENT_SKILLS.md`; the harness
advertises them by ID. Use `spec-sync-auditor` after large changes to detect
documentation drift; use `qa-verifier` for independent verification; use
`compiler-architect` for read-only design review. The interpreter remains the
oracle regardless of which agent writes code.
