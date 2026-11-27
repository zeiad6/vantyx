# Historical Requirements Audit

Date: 2026-11-23; reconciled 2026-11-24 after T-064, T-063, T-062, T-061,
T-060, T-059, T-058, T-057, T-054, T-013, and T-046
Repository: RustZ (public name `vantyx`; renamed from `auren`)
Auditor: autonomous engineering session
Scope: every requirement recoverable from `TASKS.md`, `PROJECT_STATUS.md`,
`docs/roadmap.md`, `AGENTS.md`, the ADR set, the public launch material, Git
history, the source tree, tests, benchmarks, and the two Vantyx AI directives.

## Method

1. Read the task inventory and every ADR, roadmap, status, and launch document.
2. Inspected `src/`, `tests/`, `examples/`, `benchmarks/`, `tools/`, CI, and
   `.opencode/agents/`.
3. Re-ran the verification gate instead of trusting prior reports.
4. Assigned one status per requirement: `COMPLETE`, `PARTIAL`, `MISSING`,
   `BROKEN`, `BLOCKED`, or `SUPERSEDED`.
5. Recorded the actual evidence, the remaining work, and a priority.

Limitations: the original conversation prompts are not stored verbatim in the
repository. Requirements were reconstructed from the written task records and
the two AI directives supplied in this session. A claim is not marked complete
because a document says so; the table cites code or an observed command.

## Observed baseline at audit start (2026-11-23)

| Check | Observed result |
| --- | --- |
| `cargo test` | 241 passed at first audit: 126 library unit, 7 CLI unit, 41 e2e, 67 native (historical; see current re-verification below) |
| `cargo build` / `--release` | succeeded earlier in the session; re-checked before release claims |
| `cargo clippy --all-targets -- -D warnings` | clean at the last recorded gate |
| Public CI | runs 45308499010, 45308796021, 45309035756 passed (pre-rename); run 45317367818 passed after rename and AI foundation; run 45318733338 passed with structured extraction (357 tests); run 45321718008 passed with the map hash index (358 tests); runs 45330680289 and 45330898922 passed with embeddings (383 tests) |
| Source AI references | none (`grep` over `src/`, `tests/`, `Cargo.toml`) |
| Modules / imports | no AST or parser support |
| Async / await | no language support; only a host thread for the interpreter stack |
| Tensor / GPU code | none at initial inspection; still none |
| Embedding code | vector math and in-memory index existed at initial inspection; T-045 adds provider interface, hash baseline, bounded cache, and `/v1/embeddings` |

## Current re-verification (2026-11-24)

| Check | Observed result |
| --- | --- |
| `cargo test` | **685 passed**: 381 library unit, 15 CLI unit, 38 AI integration, 113 e2e, 138 native (2026-11-26, after T-094/T-092/T-091/T-090/T-089); the full native suite also passed with `VANTYX_CC=gcc`; earlier 676-, 669-, 664-, 647-, 645-, 643-, 636-, 630-, 629-, 627-, 623-, 621-, 613-, 584-, 581-, 573-, 571-, 550-, 526-, 517-, 516-, 507-, 501-, 491-, 485-, 478-, 469-, 463-, 457-, 437-, 430-, 429-, 422-, 413-, 383-, 358-, 357-, and 340-test runs also passed |
| Map benchmark | `map_ops` native 3.597 ms, interpreter 121.728 ms, C 2.884 ms, Rust 4.495 ms (2026-11-24; `benchmarks/results/2026-11-24.*`) |
| `cargo build` / `--release` | both succeeded after the AI foundation and rename |
| `cargo clippy --all-targets -- -D warnings` | clean, zero warnings |
| Examples | all five interpreter/native outputs identical |
| Rename | code, CLI, extension, env vars, tests, docs, CI, GitHub repository, and clean-clone CI complete |
| AI host | `src/ai/` foundation and `vantyx ai` CLI implemented; offline and loopback tests pass; streamed text chat is covered by loopback HTTP and HTTPS smoke tests; offline bounded GGUF/Safetensors inspection with streamed SHA-256 is implemented (T-090), including Safetensors shard-index aggregation with index/header cross-checks and per-shard digests (T-091), plus bounded JSONL dataset validation with field/type summaries and streamed SHA-256 (T-092) and optional schema validation with numbered violations (T-094) |
| Embeddings | deterministic hash provider, batch limits, bounded cache, OpenAI-compatible `/v1/embeddings`, and `vantyx ai embed` implemented; offline/loopback tests pass |
| Modules (T-013) | file imports, path confinement, cycles, declaration-only modules, interpreter/native differential test, and `E0501`–`E0512` |
| RAG (T-046, T-054, T-057) | bounded ingestion, stable cited chunks, strict versioned JSON index, BM25/cosine/hybrid RRF search, source-prefix filtering, recall/precision/MRR evaluation, incremental vector reuse with deletion pruning, and deterministic MMR diversity reranking with duplicate suppression; 21 unit + 4 CLI tests and manual 3-document runs |
| File, directory, metadata, and path builtins (T-058–T-064) | `read_file`, `write_file`, `append_file`, `file_exists`, `file_size`, `is_file`, `is_dir`, `is_symlink`, `list_dir`, `make_dir`, `remove_file`, `remove_dir`, `make_dir_all`, `remove_dir_all`, `rename`, `copy_file`, `path_join`, `path_parent`, `path_name`, `path_stem`, `path_extension`, and `path_is_absolute` in both engines; 64 MiB UTF-8 read cap, NUL rejection, sorted listings, fatal `R0015`, recursive creation/removal with symlink-safe unlinking, trailing-separator trimming, and destructive-root refusal (ADR-0035), byte-preserving copies, read-only follow-symlink metadata probes with `lstat` symlink detection, lexical POSIX-style no-I/O paths, left-to-right evaluation, and `E0401` for dynamically typed arguments |
| Or-patterns (T-047/T-050) | top-level `|` alternatives, shared binding-set checking (`E0317`), exhaustiveness integration, ordered interpreter matching, and native short-circuit lowering with conditional binding initialization; parser/checker/e2e/native tests; ADR-0021 |
| Result propagation (T-075) | postfix `?` unwraps the first variant's single payload of a nominal two-variant enum or returns the second variant unchanged; only whole `let` initializers, plain assignment values, and expression statements; explicit matching return type required; `E0319` for invalid placement/shape/return type; five e2e tests, three differential native tests, and a ninth fuzzer shape; specification §5.10, ADR-0038 |
| Print and float `min`/`max` (T-079) | both engines evaluate all `print` arguments, then render them, then write the line; a failing later argument leaves no partial stdout and later mutations are visible; float `min`/`max` use one explicit rule (single NaN yields the other operand; ties return the second operand); two e2e and two differential native tests, plus fuzzer cases; specification §6/§11, ADR-0039 |
| `type` argument evaluation (T-080) | the native backend binds and discards the argument before yielding the static type name, so calls, mutation, nested output, and runtime errors agree with the interpreter; one e2e and one differential native test, plus fuzzer cases; specification §6/§11 |
| String `contains` argument order (T-081) | native string `contains` binds both operands in source order instead of relying on unspecified C argument evaluation, which GCC used right to left; extended left-to-right text test plus a clang/GCC cross-compiler differential test; specification §6/§11 |
| `Unit` equality operands (T-082) | `Unit ==`/`!=` still evaluate both operands left to right even though the result is constant; native `binary_expr` hoists both operands whenever either has effects; one e2e and one differential native test, plus fuzzer cases; specification §5.3/§11 |
| `Unit` interpolation evaluation (T-083) | an interpolated `Unit` expression is evaluated once before rendering `()`; native `display_value` keeps the expression via `((void)(value), vantyx_str_from_unit())`; one e2e and one differential native test, plus fuzzer cases; specification §1/§11 |
| Nested patterns (T-048/T-049) | recursive payload patterns, multi-level bindings, matrix exhaustiveness, guard exclusion, and native short-circuit lowering with path-based bindings; parser/checker/e2e/native tests; ADR-0019/ADR-0020 |
| AI live evidence | Cloud path verified on 2026-11-24 against Gemini (discovery, chat, streaming, extraction, 768-dimension embeddings, semantic RAG/search, retrieval evaluation) and re-verified at 20:59Z with 21/21 checks using `MY_API_KEY`; an earlier retry was geo-blocked and one re-run hit a transient 503, so access is not guaranteed; no local server or model is installed |
| Language-level AI | still blocked on async (T-017); the modules prerequisite (T-013) is complete |
| Tensor / GPU / embedding model code | none; vector math, provider embeddings, and a persisted flat retrieval index exist, but no tensor runtime or trained local model ships |

## Status summary

| Status | Core tasks | Directive requirements |
| --- | ---: | ---: |
| COMPLETE (including documented subsets) | 54 | 16 |
| IN_PROGRESS | 0 | 0 |
| READY | 0 | 0 |
| MISSING | 8 | 10 |
| BLOCKED | 0 | 3 |
| PARTIAL | 2 | 16 |
| BROKEN | 0 | 0 |
| SUPERSEDED | 0 | 0 |
| **Total tracked** | **64** | **45** |

Counts are derived from the rows below, not from prior reports. Core rows use
the `TASKS.md` vocabulary; directive rows use
`COMPLETE`/`PARTIAL`/`MISSING`/`BLOCKED`. `COMPLETE (subset)` and
`COMPLETE (offline/loopback)` are counted as complete because the documented
subset has tests and evidence; the missing scope is listed in the same row. The
directive matrix includes 45 rows covering the AI extension plus the recovered
cross-cutting requirements. Core rows T-036–T-043, T-067–T-068, and T-075–T-085
are tracked in `TASKS.md` and the AI matrix rather than duplicated here;
T-044–T-066 and T-069–T-074 are included above. T-016 moved from
`MISSING` to `PARTIAL` when T-075 shipped statement-position `?`; a built-in
generic `Result`, error conversion, and expression-position propagation remain
open.

## Core task matrix (T-001 … T-064)

| ID | Requirement | Implementation / evidence | Status | Missing work | Priority | Action |
| --- | --- | --- | --- | --- | --- | --- |
| T-001 | Bootstrap Cargo project | `Cargo.toml`, `src/`, Git; `cargo build` | COMPLETE | — | P0 | Keep |
| T-002 | Indentation-aware lexer | `src/lexer.rs`, 9 unit tests, fuzz smoke | COMPLETE | — | P0 | Keep |
| T-003 | Parser and AST | `src/parser.rs`, `src/ast.rs`, 18 unit tests | COMPLETE | — | P0 | Keep |
| T-004 | Diagnostic renderer | `src/diagnostic.rs`, 3 unit tests | COMPLETE | — | P0 | Keep |
| T-005 | Gradual checker | `src/checker.rs`, 47 unit tests | COMPLETE | Flow narrowing/generics are T-018 | P0 | Keep |
| T-006 | Tree-walking runtime | `src/interp.rs`, 23 unit tests + e2e | COMPLETE | — | P0 | Keep |
| T-007 | CLI | `src/main.rs`; `run/check/build/repl/version/help` plus `vantyx ai` host commands | COMPLETE | Keep command help and errors synchronized | P0 | Extend |
| T-008 | E2E suite | `tests/e2e.rs`, 105 tests including modules, or-patterns, integer ranges, file/directory I/O, metadata probes, glob expansion, recursive copy, deterministic print, float `min`/`max`, and `type` argument side effects | COMPLETE | — | P0 | Keep |
| T-009 | REPL | `src/repl.rs`; persistence tests | COMPLETE | Multi-line recall, field completion (T-034) | P1 | Extend |
| T-010 | IR + native backend | `src/ir.rs`, `src/codegen_c.rs`, 130 differential tests including imported modules, nested-pattern support, native or-patterns, integer ranges, file/directory I/O, recursive directories with destructive-root refusal, metadata probes, lexical globbing, recursive directory copy, text builtins, lexical path builtins, deterministic two-phase print, float `min`/`max`, `type` argument evaluation, and ordered string `contains` operands | COMPLETE (subset) | Reclamation, debug info, optimization, more types | P0 | Extend |
| T-011 | Bytecode VM | None | MISSING | Entire VM and benchmark | P2 | Defer |
| T-012 | Ownership / escape analysis | None | MISSING | Design, analysis, tests | P2 | Defer |
| T-013 | Modules and imports | `src/module.rs`, import AST/parser, `E0501`–`E0512`, 12 unit tests, e2e and native tests, manual interpreter/native smoke | COMPLETE (subset) | Namespaces, aliases, re-exports, package-relative lookup, incremental builds | P1 | Keep |
| T-014 | Enums and matching | AST/checker/interp/IR/codegen; 12 e2e + native tests | COMPLETE (subset) | Range/float patterns, generic enums | P1 | Extend |
| T-015 | Closures and iterators | None | MISSING | Syntax, capture semantics, native lowering | P2 | Defer |
| T-016 | `Result` / `?` | Statement-position postfix `?` for nominal two-variant enums; checker `E0319`, interpreter `Flow::Return`, native `IrStmtKind::Try`; five e2e and three differential native tests; ADR-0038 (T-075) | PARTIAL | Built-in generic `Result[T, E]`, error conversion, `try` block, and expression-position propagation | P2 | Extend |
| T-017 | Async and concurrency | None | MISSING | Design, runtime, cancellation, tests | P3 | Defer |
| T-018 | Traits and generics | None | MISSING | Type-system design and implementation | P3 | Defer |
| T-019 | Formatter and linter | `vantyx lint` with `L0001`–`L0004`, 24 unit tests, five e2e tests, `docs/linting.md`, ADR-0037 (T-073/T-085) | PARTIAL | Comment-preserving formatter; lint suppression/configuration and broader reachability | P2 | Formatter next |
| T-020 | Package manager | None | MISSING | Manifest, resolver, registry, lockfile | P3 | Defer |
| T-021 | C ABI FFI | None | MISSING | ABI, unsafe boundary, native linking | P3 | Defer |
| T-022 | Benchmarks vs Python/Rust/C | `benchmarks/`, 6 workloads, raw JSON/MD | COMPLETE | Enum and AI benchmarks | P2 | Extend |
| T-023 | Security audit | `docs/security-audit-2026-11-22.md`, 12 findings | COMPLETE | Re-audit after AI subsystem | P1 | Extend |
| T-024 | Native `List[T]` | IR + C runtime; differential tests | COMPLETE (subset) | Reclamation | P1 | Keep |
| T-025 | Native string indexing/iteration | UTF-8 helpers; differential tests | COMPLETE | — | P1 | Keep |
| T-026 | Documentation reconciliation | Docs updated; this audit adds a new pass | COMPLETE | Must be repeated after rename/AI | P1 | Repeat |
| T-027 | Native `Map[K,V]` | Insertion-ordered arrays + scalar-key hash index; differential tests | COMPLETE (subset) | Structural keys remain O(n); reclamation | P1 | Extend |
| T-028 | Prior-art review | `docs/research/prior-art-lessons-2026.md` | COMPLETE | AI prior art is new work | P2 | Extend |
| T-029 | REPL editing/history/echo | `src/repl.rs`, 7 CLI unit + e2e tests | COMPLETE | — | P3 | Keep |
| T-030 | Native enums/match | Tagged C structs; differential tests | COMPLETE (subset) | Allocation benchmark | P2 | Extend |
| T-031 | Bracketed-match diagnostic | Parser test + e2e test | COMPLETE | — | P3 | Keep |
| T-032 | Scalar literal patterns | Parser/checker/interp/IR/C; tests | COMPLETE | Float patterns | P2 | Extend |
| T-033 | Match guards | Parser/checker/interp/IR/C; tests | COMPLETE | Guards cannot bind or mutate | P2 | Keep |
| T-034 | REPL multi-line history/field completion | None | MISSING | Entire task | P4 | Defer |
| T-035 | Public repository and launch kit | Public repo, CI, release, issue, `llms.txt`; Vantyx rename verified by GitHub and clean-clone CI | COMPLETE | Keep discovery material current | P1 | Maintain |
| T-044 | Native scalar-key map hash index | `src/codegen_c.rs`; `large_scalar_maps_match`; 2026-11-24 benchmark | COMPLETE | Structural-key hashing; adversarial seed | P1 | Keep |
| T-045 | Embedding provider interface, batch embeddings, cache, and CLI | `src/ai/provider.rs`, `src/ai/embedding_provider.rs`, `src/ai/openai.rs`, `src/ai_cli.rs`; 413-test run; loopback `/v1/embeddings`; ADR-0017 | COMPLETE (offline/loopback) | Live embedding-model evidence; TTL/persistent cache | P1 | Extend |
| T-046 | Local RAG ingestion, hybrid retrieval, citations, persistence, source filtering, and evaluation | `src/ai/rag.rs` (14 unit tests), `src/ai_cli.rs` (`index`/`search`/`eval`), `tests/ai.rs` (2 integration tests), `examples/ai/retrieval-eval.json`; ADR-0018 | COMPLETE (offline/loopback) | PDF/HTML/Office ingestion, arbitrary metadata fields, cross-source dedup, measured reranking quality, grounded generation, live corpus evaluation | P1 | Extend |
| T-047 | Top-level or-patterns with shared bindings and exhaustiveness | `src/ast.rs` (`Pattern::Or`), `src/parser.rs`, `src/checker.rs` (`E0317`), `src/interp.rs`; parser + 5 checker + e2e tests | COMPLETE (interpreter; native lowering added by T-050) | Or-patterns over `Any` natively | P1 | Extend |
| T-048 | Nested payload patterns with multi-level bindings and matrix exhaustiveness | `src/ast.rs` (`PatternArg::Nested`), `src/parser.rs`, `src/checker.rs` (`CoverPattern`, `matrix_exhaustive`), `src/interp.rs`; parser + 4 checker + e2e tests; ADR-0019 | COMPLETE (interpreter; native lowering added by T-049) | Tuple patterns; nested unreachable analysis | P1 | Extend |
| T-049 | Native nested-pattern lowering with path-based bindings and single-evaluation guards | `src/ir.rs` (recursive `IrPattern`, `IrFieldPath`), `src/codegen_c.rs` (recursive conditions, binding declarations), `tests/native.rs::nested_patterns_match`, extended e2e; ADR-0020 | COMPLETE (interpreter/native differential) | Tuple patterns; nested unreachable analysis | P1 | Extend |
| T-050 | Native top-level or-pattern lowering with one shared binding set | `src/ir.rs` (`IrPattern::Or`, `collect_bindings`), `src/codegen_c.rs` (`or_binding_declarations`, alternative conditions), `tests/native.rs` or-pattern differential tests; ADR-0021 | COMPLETE (interpreter/native differential) | Nested or-patterns; tuple patterns; usefulness analysis | P1 | Extend |
| T-051 | Integer range patterns with half-open and inclusive bounds | `src/token.rs`/`src/lexer.rs` (`DotDot`/`DotDotEq`), `src/ast.rs` (`Pattern::Range`), `src/parser.rs` (`finish_range_pattern`), `src/checker.rs` (`E0318`), `src/interp.rs`, `src/ir.rs` (`IrPattern::Range`), `src/codegen_c.rs`; parser + 3 checker + e2e + native differential tests; ADR-0022 | COMPLETE (interpreter/native differential) | Range expressions; float/character/computed bounds; interval usefulness analysis | P1 | Extend |
| T-052 | Streamed OpenAI-compatible text chat through the provider, HTTP transport, and CLI | `src/ai/provider.rs` (`chat_stream`, `streaming` capability), `src/ai/sse.rs` (`push_bytes` UTF-8 carry), `src/ai/http.rs` (`Transport::stream`, content-length/chunked/close-delimited, `curl --include --no-buffer`), `src/ai/openai.rs` (`ChatStreamState`, streaming request JSON), `src/ai_cli.rs` (`--stream`), `tools/streaming_smoke.py`; unit, loopback CLI, and HTTP/HTTPS smoke tests; ADR-0023 | COMPLETE (offline/loopback; live cloud path verified later under T-067) | Streaming tool calls, interactive cancellation, throughput evidence (first-token latency observed once) | P1 | Extend |
| T-053 | Remove the unrelated `auren` account from contributor attribution | `git filter-branch` over branches and tags, force-with-lease push of `main` and `v0.1.0`, `.mailmap`; contributor API and commit-identity checks | COMPLETE (history rewritten with explicit authorization) | None; old hashes are historical | P1 | Keep |
| T-054 | Incremental RAG re-indexing with stable-chunk vector reuse, deletion pruning, and empty-index semantics | `src/ai/rag.rs` (`IncrementalPlan`, `ReuseStats`, `plan_incremental`, `validate_chunk_set`, 4 unit tests), `src/ai_cli.rs` (`--incremental`, compatibility checks, lazy provider, reuse reporting, parser test), `tests/ai.rs` (1 CLI integration test), `benchmarks/rag.py` incremental phases; ADR-0024 | COMPLETE (offline/loopback) | File-manifest shortcut, partial-prefix merge, sub-chunk diffing, trained-model evidence | P1 | Extend |
| T-055 | Shared immutable ASCII character buffers for native string iteration and indexing | `src/codegen_c.rs` static single-byte character table; `string_ops` benchmark recorded 8.318 ms to the 1.6–3.2 ms range; differential native tests | COMPLETE | Multibyte iteration still allocates; concatenation still copies | P2 | Extend |
| T-056 | Native enum construction and match benchmark | `benchmarks/run.py` `enum_ops` workload; seven-workload suite; measured native 3.646 ms vs C 3.766 ms and Rust 4.529 ms in the same run; ADR-level decision not to rewrite the representation | COMPLETE | No representation change justified by current evidence | P2 | Keep |
| T-057 | Deterministic MMR diversity reranking for local RAG search | `src/ai/rag.rs` (`DEFAULT_MMR_LAMBDA`, `search_with_source_diverse`, `diversify_hits`, `deduplicate_hits`, `jaccard_similarity`, `mmr_select`, 3 unit tests), `src/ai_cli.rs` (`--diverse`, `rerank` reporting, parser test), `tests/ai.rs` (1 CLI integration test), `benchmarks/rag.py` diverse phase; ADR-0025 | COMPLETE (offline/loopback) | Measured retrieval-quality improvement; semantic paraphrase dedup; configurable CLI lambda | P1 | Extend |
| T-058 | Basic UTF-8 file I/O builtins | `read_file`, `write_file`, `append_file`, and `file_exists` in `src/checker.rs`, `src/interp.rs`, `src/ir.rs`, and `src/codegen_c.rs`; 64 MiB read cap, UTF-8/NUL validation, fatal `R0015`, left-to-right evaluation; e2e and differential native tests; ADR-0026 | COMPLETE (interpreter/native differential) | Streaming/append validation, atomic writes, recoverable errors, sandbox | P1 | Extend |
| T-059 | Single-level directory operations | `list_dir`, `make_dir`, `remove_file`, and `remove_dir` in both engines; deterministic byte-sorted UTF-8 listings, single-level semantics, `R0015`, left-to-right evaluation; directory-specific e2e and differential native tests; ADR-0027 | COMPLETE (interpreter/native differential) | Metadata objects/preservation, directory-entry cap (recursive creation/removal is T-063; probes are T-064; glob is T-065) | P1 | Extend |
| T-060 | UTF-8 text builtins | `split`, `join`, `trim`, `starts_with`, `ends_with`, and `replace` in both engines; literal lossless splitting, Unicode scalar splitting for an empty separator, explicit `White_Space` trim, non-overlapping replacement, `R0016`; e2e and differential native tests; ADR-0028 | COMPLETE (interpreter/native differential) | Regex, case conversion, normalization, locale handling, grapheme-cluster APIs | P1 | Extend |
| T-061 | Rename and byte-preserving regular-file copy | `rename` and `copy_file` in both engines; host-defined rename overwrite, regular-file source check, exact byte copy including NUL/invalid UTF-8, no metadata preservation, `R0015`, left-to-right evaluation, `E0401` for dynamically typed paths; e2e and differential native tests; ADR-0029 | COMPLETE (interpreter/native differential) | Recursive move (recursive copy is T-066), metadata preservation, atomic copy, fsync, recoverable errors | P1 | Extend |
| T-062 | Lexical POSIX-style path builtins | `path_join`, `path_parent`, `path_name`, `path_stem`, `path_extension`, and `path_is_absolute` in both engines; no filesystem access or normalization, UTF-8-preserving slices, final-dot extension rule, ordered `path_join`, `E0401` for dynamically typed paths; e2e and differential native tests; ADR-0030 | COMPLETE (interpreter/native differential) | `Path` type, normalization/containment, Windows drive/UNC support, multi-argument join | P1 | Extend |
| T-063 | Recursive directory creation and removal | `make_dir_all` and `remove_dir_all` in both engines; component-by-component creation accepting existing directories and symlinks to directories, empty-path no-op creation, symlink-unlinking recursive removal with trailing-separator trimming before the root check, destructive-root refusal added by T-069/ADR-0035, `R0015` failures, `R0005` dynamic interpreter type errors, `E0401` for dynamically typed native paths; e2e, differential native, and Unix symlink-safety tests; ADR-0031 | COMPLETE (interpreter/native differential) | Metadata preservation, traversal callbacks, dry run/trash, recursion depth and entry caps, intermediate `link/sub` confinement, Windows evidence (probes are T-064; glob is T-065) | P1 | Extend |
| T-064 | File metadata probes | `file_size`, `is_file`, `is_dir`, and `is_symlink` in both engines; follow-symlink size and type predicates, `lstat` symlink detection including dangling links, regular-file requirement and logical `st_size` with `R0015`, false-on-error predicates, `E0301`/`R0005`/`E0401` boundaries; e2e, differential native, and Unix symlink tests; ADR-0032 | COMPLETE (interpreter/native differential) | Metadata objects, mode/owner/timestamp/ACL/inode/device queries, recursive directory size, metadata preservation, atomic probes, Windows evidence | P1 | Extend |
| T-065 | Lexical glob expansion | `glob` in both engines; `/` components, Unicode `*`/`?`/classes, whole-component `**` without symlink traversal, explicit-dot hidden rules, byte-sorted deduplicated lexical results, `lstat` literal probes, `R0016` malformed patterns/component cap, `R0015` I/O/depth/result caps/non-UTF-8 names, `E0301`/`R0005`/`E0401` boundaries; library, e2e, differential native, and Unix symlink tests; ADR-0033 | COMPLETE (interpreter/native differential) | Escaping/braces/extglobs, case-insensitive mode, normalization or `..` confinement, streaming/cancellation, result-count tuning, Windows evidence | P1 | Extend |
| T-066 | Recursive directory copy | `copy_dir_all` in both engines; existing source directory and new target requirement, parent resolution, resolved-self containment, raw-name and byte copying, root-link following with inner-link and special-file rejection, no metadata preservation or rollback, 100,000-entry and 128-depth caps, `E0301`/`R0005`/`E0401` boundaries; library, e2e, differential native, and Unix link/non-UTF-8 tests; ADR-0034 | COMPLETE (interpreter/native differential) | Recursive move, metadata preservation, overwrite/merge modes, rollback or atomic staging, byte/entry-count tuning, sandbox, Windows evidence | P1 | Extend |
| T-069 | Refuse destructive recursive-removal roots | `remove_dir_all` refuses `.`, `./`, `..`, `../`, every root whose final component is `.` or `..` (including `link/.` and `link/..`), and `/`/`//`/`///` with `R0015` before any filesystem access in both engines; ordinary roots such as `tree`, `tree/`, `./tree`, `/tmp/tree`, and `foo/.hidden` remain accepted; empty paths keep the ordinary missing-path failure; five `src/fs_safety.rs` unit tests, two e2e tests, and two differential native tests; the prior test that locked the destructive `.` behavior was replaced; ADR-0035 amends ADR-0031 | COMPLETE (interpreter/native differential) | Lexical only: no link resolution or intermediate normalization, `link/sub` still traverses, check-then-act races remain, and there is no confirmation, trash, dry run, rollback, depth/entry cap, `Result` recovery, or sandbox; Windows native evidence missing | P1 | Keep |
| T-070 | Native statement-expression conditions | `if`/`elif`/`while` conditions that lower to GNU statement expressions are wrapped in one extra pair of parentheses; comparisons keep their existing form; `tests/native.rs::conditions_with_calls_match`; ADR-0036 | COMPLETE (P0 correctness fix) | No statement-prelude rewrite; GNU C11 remains required | P0 | Keep |
| T-071 | Native short-circuit evaluation | Effectful `and`/`or` right operands lower through a lazy conditional expression; effect-free operands use `&&`/`||`; call-count and left-to-right tests; ADR-0036 | COMPLETE (P0 correctness fix) | Effect analysis remains conservative and treats every binary/unary node as effectful | P0 | Keep |
| T-072 | Deterministic differential fuzzer | `tools/differential_fuzz.py` with ten program shapes (scalars/control flow, collections, pattern matching, nested aliasing, typed functions, strings, numerics, result-enum `?`, runtime-error agreement, and loop-heavy control flow added by T-077), deterministic seeds, failure saving, an optional bounded line-oriented shrinker, and CI at 80 iterations plus fuzzer self-tests; 180/180 on seeds 1, 500, and 1000 with nine shapes, the tenth shape passed 20/20 generated programs on seeds 1–20, and the full ten-shape run agreed 200/200 on seeds 1, 2, 3, 500, and 1000; found T-070/T-071, a collection-generator coverage bug, and (with targeted probing) the T-078 numeric-string grammar divergence, the T-079 signed-zero `min`/`max` and partial-`print` divergences, and the T-080 skipped `type` argument | COMPLETE (tooling) | No sanitizer integration or coverage measurement; the line shrinker cannot split a line or guarantee a global minimum; error paths compare exit code and stdout only | P2 | Extend |
| T-073 | Static linter | Read-only AST pass with `L0001` unused binding, `L0002` `var` never reassigned, `L0003` unreachable statement after a direct `return`/`break`/`continue`, and `L0004` unused parameter (T-085); underscore names, loop variables, and match bindings ignored; lint-only findings exit `0`, compile errors exit `1`; 24 unit tests and five e2e tests; `docs/linting.md`, specification §14, ADR-0037 | COMPLETE (tooling, conservative) | Suppression/configuration, loop-variable/match-binding coverage, all-terminating `if`/`match` reachability, deny mode, machine-readable output, fixes | P2 | Extend |
| T-074 | Reachability-aware return checking | `block_returns` walks reachable statements and stops at the first `return`/`break`/`continue`; a fully returning `if` with `else` or `match` terminates the block; dead code after `return` no longer causes a false `E0309`; three checker unit tests, two e2e tests, and one differential native test; ADR-0037 | COMPLETE (checker correctness) | No full control-flow graph or loop-termination proof; `if` without `else` does not terminate | P1 | Keep |

### Core partials that must not be described as finished

| Area | What exists | What is still missing |
| --- | --- | --- |
| Native backend | Typed subset with lists, maps (scalar-key hash index), structs, enums, strings, guards | Reclamation, cycle handling, structural-key hashing, debug info, optimization, `Any` |
| Pattern matching | Enum, wildcard, scalar literal, nested payloads, top-level or-patterns, integer ranges, guards | Tuple patterns, floats, interpolated strings, generic enums |
| REPL | Line editing, single-line history, top-level completion | Multi-line recall, field completion, local completion |
| Memory | Interpreter `Rc` with cycles; native intentional leaks | Cycle collector or ownership/escape analysis; sandbox |
| Tooling | `run/check/lint/build/repl/ai` | Formatter, test runner, package manager, LSP, debugger |
| Platforms | Linux x86_64 CI only | macOS/Windows CI and native-backend checks |

## Roadmap inventory beyond `TASKS.md`

| Roadmap item | Status | Evidence / gap | Priority | Action |
| --- | --- | --- | --- | --- |
| Debug info and optimization pipeline | MISSING | No DWARF, constant folding, DCE, inlining, or unboxing | P2 | Plan after reclamation |
| Cranelift/LLVM evaluation | MISSING | ADR-0008 defers it; no benchmark | P3 | Defer |
| Escape analysis / ownership | MISSING | T-012 | P2 | Defer |
| Cycle handling | MISSING | Documented leak in interpreter and native | P2 | Design |
| Documented safety case | PARTIAL | `docs/security.md`, `docs/memory_model.md`; no formal case | P2 | Extend |
| Profiling and optimization | MISSING | None | P2 | Measure first |
| SIMD/vectorization | MISSING | None | P3 | Research |
| Async/concurrency design | MISSING | T-017 | P3 | Defer |
| Standard library | MISSING | Builtins only; file/directory I/O, recursive directories, rename/copy, lexical paths, and basic text exist | P1 | Plan |
| Formatter/linter/test runner | PARTIAL | Linter delivered (T-073, ADR-0037); formatter blocked on comment preservation; no test runner | P2 | Formatter design |
| Package manager/registry | MISSING | T-020 | P3 | Defer |
| LSP/editor support | MISSING | None | P3 | Defer |
| Tensors/numerical | MISSING | Roadmap Phase 11 only | P2 | AI track |
| GPU backend | MISSING | No hardware verification | P3 | Research only |
| C ABI FFI / Python/Rust interop | MISSING | T-021 | P3 | Defer |
| Independent external audit | MISSING | None | P3 | Before 1.0 |
| Stability policy / release process | PARTIAL | SemVer implied; v0.1.0 pre-release | P2 | Document |
| Cross-platform CI | MISSING | Ubuntu-only workflow | P2 | Add matrix |
| Regression/stress suites | PARTIAL | 636 tests; ten-shape deterministic differential fuzzer runs in CI at 80 iterations (T-072/T-077); fuzz smoke is manual | P2 | Add sanitizer integration and coverage measurement |
| Native map hash index | COMPLETE for scalar keys (T-044, ADR-0016) | ADR-0010 revisit criterion | P1 | Keep; consider structural keys |
| Enum allocation benchmark | MISSING | T-030 scope limit | P3 | Benchmark |
| Nested patterns | COMPLETE for interpreter and native payload nesting (T-048/T-049) | Matrix exhaustiveness; path-based native lowering | P1 | Keep |
| Or-patterns | COMPLETE for top-level alternatives in the interpreter and native backend (T-047/T-050) | Shared bindings; native conditional initialization | P1 | Keep |
| `ROADMAP.md` at repository root | COMPLETE | Canonical high-level roadmap; `docs/roadmap.md` retains phase detail | P3 | Keep synchronized |

## Vantyx AI directive matrix

The directive is broad. Requirements are grouped so that each row is testable.
"Language-level" means usable from `.vtx` source; "host-level" means a Rust
library or CLI capability. The distinction matters because file-based modules
exist but async does not.

| ID | Requirement | Current state | Status | Missing work | Priority | Action |
| --- | --- | --- | --- | --- | --- | --- |
| AI-001 | Historical requirement recovery | This document | COMPLETE | Keep synchronized | P0 | Maintain |
| AI-002 | Rename audit and execution | `RESEARCH/NAMING_AUDIT.md`; code, CLI, extension, docs, tests, CI, GitHub repository, and clean-clone CI verified | COMPLETE | Keep old-name redirects documented | P0 | Maintain |
| AI-003 | AI architecture document | `docs/ai/AI_ARCHITECTURE.md` | COMPLETE | Keep synchronized | P0 | Maintain |
| AI-004 | Current AI ecosystem research | `docs/ai/AI_RESEARCH.md` with verified repository metadata | COMPLETE | Refresh before dependency decisions | P0 | Maintain |
| AI-005 | AI security threat model | `docs/ai/AI_SECURITY.md` | COMPLETE | Revisit when tools gain write/network capability | P0 | Maintain |
| AI-006 | Provider-independent interface | `src/ai/provider.rs`, mock/echo/OpenAI adapters, stable errors and capabilities | COMPLETE | Add new adapters behind the same trait | P0 | Maintain |
| AI-007 | Working cloud provider | OpenAI-compatible adapter, opt-in remote HTTPS, loopback/stub tests, and a live Gemini run on 2026-11-24 covering discovery, chat, streaming, extraction, embeddings, RAG, and retrieval evaluation, re-verified at 20:59Z with 21/21 checks | COMPLETE | Provider access is not guaranteed: an earlier retry was rejected with HTTP 400 `User location is not supported` and one re-run hit a transient 503; no bypass attempted | P1 | Maintain |
| AI-008 | Working local inference path | Compatible-endpoint path supports Ollama/llama.cpp/LM Studio; no installed server evidence | PARTIAL | Run against a real local server; add native Ollama discovery | P1 | Verify live |
| AI-009 | Model manager CLI | `vantyx ai models` and `doctor` implemented; no info/pull/remove or integrity checks | PARTIAL | Add info/pull/remove with checksums, size, license, provenance | P2 | Implement |
| AI-010 | Tokenization | Heuristic estimator explicitly labelled "not a model tokenizer" | PARTIAL | Model-specific tokenizer boundary when an engine is embedded | P2 | Research |
| AI-011 | Structured output / JSON Schema | Strict JSON parser, depth/size limits, JSON Schema subset validator, `ai::extract`, `vantyx ai extract`, bounded repair with usage accounting, and offline/loopback tests | COMPLETE | Server-side constrained decoding remains research | P1 | Maintain |
| AI-012 | Tool calling with capabilities | Typed tool registry, capability sets, path roots, approval enum, timeouts, redacted logging | COMPLETE | Only read-only demo tools ship; add dangerous tools with approvals | P1 | Maintain |
| AI-013 | Agent loop with hard limits | Bounded loop with step/tool/token/time limits and cancellation flag | PARTIAL | Persistent state and resume | P1 | Extend |
| AI-014 | Multi-agent orchestration research | No evidence gathered; deliberately unbuilt | MISSING | Research only if a measurable use case appears | P3 | Defer |
| AI-015 | RAG ingestion/chunking | `src/ai/rag.rs` bounded recursive text/Markdown/code ingestion, stable line-cited chunks, incremental re-index, and deletion pruning; filtering and limit tests | COMPLETE (subset) | PDF/HTML/Office, arbitrary metadata fields, cross-source dedup | P1 | Extend |
| AI-016 | Embeddings and vector store | Provider interface, batch limits, deterministic hash provider, bounded cache, OpenAI-compatible `/v1/embeddings`, vector math, in-memory index, and strict-JSON persisted RAG index | PARTIAL | Live trained embedding-model evidence; approximate/large-scale index; vector-database integration | P1 | Extend |
| AI-017 | Hybrid search / BM25 | BM25 with Unicode tokenization, cosine semantic search, hybrid reciprocal-rank fusion, deterministic ties, source-prefix filtering, cited hits, and opt-in MMR diversity reranking | COMPLETE (subset) | Measured reranking quality and arbitrary metadata filters | P2 | Extend |
| AI-018 | Retrieval evaluation harness | `vantyx ai eval` reports recall@k, precision@k, MRR, and per-query hits from a JSON fixture; offline integration test | COMPLETE (retrieval-only) | Grounding/hallucination/latency metrics; licensed multi-document corpus | P2 | Extend |
| AI-019 | Tensor primitives | None | MISSING | Shape/dtype/strides/matmul or documented non-goal | P3 | Research |
| AI-020 | Automatic differentiation | None | MISSING | Evaluate Burn instead of custom autodiff | P4 | Research |
| AI-021 | Numerical reliability tests | None | MISSING | NaN/inf/overflow/precision/determinism | P3 | Implement with tensors |
| AI-022 | GPU backends | None | BLOCKED | No verified GPU hardware; CUDA/Metal/Vulkan/WebGPU | P4 | Research only |
| AI-023 | SIMD/CPU dispatch | None | MISSING | Runtime feature detection and portable fallback | P3 | Research |
| AI-024 | AI performance benchmarks | None | MISSING | Load, prefill, decode, tokens/s, memory, retrieval | P2 | Benchmark after foundation |
| AI-025 | Observability / tracing | Opt-in JSONL metrics with provider, model, latency, usage, and error code; no prompts or secrets | PARTIAL | Distributed tracing and per-request spans | P2 | Extend |
| AI-026 | Budgets and limits | Step, tool, token, and wall-clock budgets with tests | PARTIAL | Cost and concurrency limits | P1 | Extend |
| AI-027 | Multimodal text/image/audio/video | Typed text and image content parts; no audio/video or size validation | PARTIAL | Audio/video parts, limits, capability checks, real example | P3 | Research/implement incrementally |
| AI-028 | MCP client/server integration | None | MISSING | Protocol, transports, permissions, lifecycle | P3 | Research |
| AI-029 | ONNX/GGUF/Safetensors/Python interop | Offline bounded GGUF v2/v3 and Safetensors header inspection with tensor-range validation and streamed SHA-256 (`vantyx ai inspect`, T-090, ADR-0041), plus Safetensors shard-index aggregation with weight-map validation, index/header tensor-name cross-checks, path confinement, and per-shard digests (T-091, ADR-0042); no ONNX or Python interop and no weight loading | PARTIAL | Format support or documented non-goal | P3 | Research |
| AI-030 | AI test architecture | Mock/echo providers, stub transport, loopback HTTP, SSE, timeout, cancellation, permission tests | COMPLETE | Keep offline by default | P0 | Maintain |
| AI-031 | Working AI prototypes | Offline CLI chat, JSON output, metrics, bounded tool-agent loop, loopback OpenAI-compatible chat | PARTIAL | One real local-model example and one real cloud example | P2 | Build after live verification |
| AI-032 | Language-level `from ai import Client` | None | BLOCKED | Depends on async (T-017); modules (T-013) are complete | P1 | Unblock via host CLI first |
| AI-033 | `await` / async runtime | None | BLOCKED | T-017 | P3 | Defer |
| AI-034 | Modules/imports | `src/module.rs`, import AST/parser, module tests, e2e/native differential coverage | COMPLETE (subset) | Namespaces, aliases, re-exports, package-relative imports | P1 | Keep |
| AI-035 | Standard library / package manager | File/directory I/O, rename/copy, lexical paths, and basic text builtins exist in both engines; no package manager | PARTIAL | T-019/T-020; broader stdlib needed for AI ecosystem | P2 | Core track |
| AI-036 | AI execution sandbox | None | MISSING | Process/WASM/OS sandbox; documented limits | P2 | Design |
| AI-037 | Secrets and credentials | Generic policy only | PARTIAL | No AI credential store; env/keyring policy | P0 | Implement |
| AI-038 | Prompt-injection defense | Model output treated as untrusted; tool arguments schema-validated; capabilities deny by default; denied-tool tests | PARTIAL | Dedicated prompt-injection fixture corpus and retrieval trust labels | P0 | Extend |
| AI-039 | Model download integrity | None | MISSING | Checksums, size, license, provenance, resume | P2 | Implement |
| AI-040 | Dataset validation and provenance | `vantyx ai dataset <file>` streams UTF-8 JSONL, validates one top-level object per non-blank line, accepts BOM/CRLF/blank lines, reports size, line/blank/record/invalid counts, sorted field/type histograms, and up to 20 numbered invalid lines, and computes a SHA-256 under 256 MiB file / 16 MiB line / 10,000-field caps (T-092, ADR-0043); optional `--schema <file>` validates every structurally valid record against the supported JSON Schema subset under a 16 MiB schema cap and a shape preflight, reporting up to 20 numbered violations and exiting `1` on schema-invalid records (T-094, ADR-0044); no tokenization, splits, deduplication, full JSON Schema, or nested-field summaries | PARTIAL | Deterministic splits, dedup, tokenizer-aware counts | P3 | Extend |
| AI-040 | AI discovery material | `README.md`, `llms.txt`, launch kit, status, root `ROADMAP.md`, and AI docs updated with verified modules/RAG claims | COMPLETE | Keep synchronized after live verification | P1 | Maintain |
| AI-041 | Cross-platform CI | Ubuntu only | PARTIAL | macOS/Windows matrix | P2 | Add |
| AI-042 | Honest performance claims | Benchmark rules exist | PARTIAL | AI benchmarks do not exist | P1 | Enforce |
| AI-043 | Versioning/release process | v0.1.0 pre-release | PARTIAL | Stability policy, changelog, tags | P2 | Document |
| AI-044 | Dependency policy for AI | ADR-0015: zero-dependency host, HTTP/subprocess boundary first | COMPLETE | New ADR required before embedding any engine | P0 | Maintain |
| AI-045 | `ROADMAP.md` synchronization | Root `ROADMAP.md` summarizes Track A/Track B; `docs/roadmap.md` retains phase detail; AI phases synchronized | COMPLETE | Keep both synchronized at each gate | P3 | Maintain |

### Directive conflicts resolved in this audit

1. **"Vantyx" vs the published "Auren" name.** The directive names Vantyx
   throughout and asks for a rename audit. The repository is public as `auren`.
   Resolution: treat Vantyx as the requested name, audit it, and execute the
   rename only with a complete reference map. See `RESEARCH/NAMING_AUDIT.md`.
2. **Zero third-party Rust dependencies vs Candle/Burn/ort/tokenizers.**
   Resolution: keep the compiler and host AI foundation dependency-free; use
   explicit HTTP/subprocess boundaries for local servers and cloud APIs. Any
   embedded inference engine requires a new ADR that documents binary size,
   build complexity, licenses, and platform support. Do not add one silently.
3. **AI as first-class language syntax vs missing async/stdlib.** Resolution:
   build the host-level AI toolkit now (`vantyx ai` + Rust library); modules
   (T-013) are complete, while the language surface waits for T-017 and a
   standard library. Document the gap explicitly.
4. **"15 prototypes" vs "do not create 15 broken demos."** Resolution: ship a
   small number of real, tested prototypes first.
5. **GPU claims vs no verified GPU.** Resolution: mark GPU `BLOCKED` until a
   real device and backend execute tests; no simulated "support".

## Naming audit summary

Full detail: `RESEARCH/NAMING_AUDIT.md`.

- **Vantyx** is six letters, two syllables, pronounceable, not a person's name,
  and has no programming-language collision in the checked sources. It is a
  brand name, not a descriptive one; the content link must come from the
  tagline and documentation.
- Unrelated uses exist: `Vantyx Group`, `VantyxBot`, `VantyxMedia`, and 85 GitHub
  repositories match the string. `crates.io`, npm, and PyPI returned no package
  named `vantyx` at audit time (crates.io 404).
- The name is **not trademark-cleared**. A proper search is required before 1.0.
- Decision: proceed with Vantyx because the directive names it explicitly, and
  record the tradeoff. Revisit before 1.0 if a descriptive name is preferred.

## Prioritized recovery plan

### P0 — Truth and identity ✅ (2026-11-23)

1. [x] Publish this audit and the naming audit.
2. [x] Rename code, CLI, extension, environment variables, tests, examples,
   benchmarks, docs, and CI to Vantyx. The GitHub repository rename is the only
   remaining step.
3. [x] Create the AI architecture, research, security, and roadmap documents.
4. [x] Add AI tasks to `TASKS.md` and reconcile `PROJECT_STATUS.md` and
   `docs/roadmap.md`.

### P1 — AI foundation that can be verified offline ✅ (2026-11-23)

1. [x] Zero-dependency JSON value/parser/writer with tests.
2. [x] Provider abstraction: request, response, usage, errors, capabilities,
   timeout, cancellation, mock provider.
3. [x] OpenAI-compatible adapter behind an explicit network opt-in; SSE parser.
4. [x] Token estimation and deterministic budgets.
5. [x] Embeddings math, in-memory vector index, cosine/dot, BM25.
6. [x] Typed tool registry with capabilities and allowlists; bounded agent loop.
7. [x] CLI: `vantyx ai providers`, `vantyx ai chat --provider mock`,
   `vantyx ai doctor`, and `vantyx ai models`/`chat` against a compatible
   endpoint when configured.
8. [x] Tests for every item, including offline loopback-server tests.
9. [x] Embedding provider interface, batch limits, deterministic hash baseline,
   bounded cache, OpenAI-compatible `/v1/embeddings`, and `vantyx ai embed`
   (T-045, ADR-0017).

Evidence: 2026-11-23 `cargo test` — 357 passed (218 library unit + 11 CLI unit +
20 AI integration + 41 end-to-end + 67 native); `cargo clippy --all-targets --
-D warnings` clean; `cargo build --release` succeeded; all five examples matched
interpreter/native output. Live local-model and cloud evidence is still missing
because no server or credentials were available.

Re-verification after T-044 on 2026-11-24: `cargo test` — 358 passed (218
library unit + 11 CLI unit + 20 AI integration + 41 end-to-end + 68 native);
`cargo clippy --all-targets -- -D warnings` clean; debug and release builds
succeeded; all five examples matched. `python3 benchmarks/run.py --reps 5`
verified identical output and recorded `map_ops` native 3.597 ms versus
interpreter 121.728 ms, C 2.884 ms, and Rust 4.495 ms. GitHub Actions run
45321718008 passed build, all 358 tests, clippy, release build, and differential
examples on commit `f55c91c`.

Re-verification after T-052 on 2026-11-24: `cargo test` — 457 passed
(297 library unit + 13 CLI unit + 30 AI integration + 44 end-to-end + 73 native);
`cargo clippy --all-targets -- -D warnings` clean; debug and release builds
succeeded; all five examples matched; fuzz smoke produced controlled
diagnostics. Streaming tests cover text deltas, split UTF-8, `[DONE]`,
post-`[DONE]` frames, error-frame redaction, 429 mapping, tools/tool-call
rejection, usage fallback, content-length/chunked/close-delimited bodies,
request JSON, CLI option rejection, and a loopback CLI run.
`python3 tools/streaming_smoke.py` observed incremental HTTP and HTTPS deltas
with `stream: true` and `Accept: text/event-stream`; HTTPS used a self-signed
certificate and the system `curl`. No live model was available, so this is
protocol evidence, not model-behavior evidence.

Re-verification after T-054 on 2026-11-24: `cargo test` — 463 passed
(301 library unit + 14 CLI unit + 31 AI integration + 44 end-to-end + 73 native);
`cargo clippy --all-targets -- -D warnings` clean; debug and release builds
succeeded; all five examples matched. Incremental tests cover reuse/prune
planning, provider/model/dimension mismatch rejection, delete-all round-trip,
one-vector-per-chunk validation, `--incremental` parsing, and an offline CLI run
that edits one document, deletes another, and checks post-update search. A
manual 3-chunk reproduction reported `reused_chunks:1`, `embedded_chunks:2`,
`removed_chunks:2`, and `removed_sources:1`. The extended RAG plumbing benchmark
recorded a no-op update at 47.377 ms (600 reused, 0 embedded) and a
one-changed-chunk update at 48.049 ms (599 reused, 1 embedded), versus a
42.746 ms full build; because the hash provider is nearly free and every update
still scans and chunks the corpus, the gain is avoided embedding calls rather
than wall time.

Re-verification after T-074 on 2026-11-24: `cargo test` — 613 passed
(348 library unit + 15 CLI unit + 32 AI integration + 96 end-to-end + 122 native);
`cargo clippy --all-targets -- -D warnings` clean; debug and release builds
succeeded. T-073 added `vantyx lint` with `L0001` unused binding, `L0002` `var`
never reassigned, and `L0003` unreachable statement, plus 19 unit tests and
four e2e tests. T-074 fixed `block_returns` so dead code after a reachable
`return` no longer causes a false `E0309`, with three checker unit tests, two
e2e tests, and one differential native test. Earlier, T-069 added five
`src/fs_safety.rs` unit tests, two e2e tests, and
two differential native tests covering `.`, `..`, final `.`/`..` components,
and the filesystem root, and replaced the prior differential test that locked
the destructive `remove_dir_all(".")` behavior. The HTTPS curl timeout mapping
is covered by two unit tests (exit 28 → retryable `AI_E_TIMEOUT`; exit 7 →
`AI_E_CONNECT`). Live cloud verification
against the Gemini OpenAI-compatible endpoint covered model discovery (65
models), chat (`VANTYX LIVE`), streaming, validated extraction (one repair),
768-dimension embeddings, 3-document semantic RAG, semantic search
(`vantyx.md` first at 0.606483), and retrieval evaluation (recall@2 = 1.0,
precision@2 = 0.5, MRR = 1.0). The loopback failure harness passed 16 checks
across 7 scenarios, including 401, 429, and timeout. A retry returned HTTP 400
`User location is not supported` for both available credentials; a later
re-verification at 20:59Z using `MY_API_KEY` passed all 21 live checks after a
first re-run hit a transient HTTP 503 during extraction. No local model is
installed. Evidence: `docs/ai/LIVE_VERIFICATION.md`,
`docs/ai/live-evidence-2026-11-24.json`, and
`docs/ai/live-evidence-2026-11-24-retry2.json`; no key value is stored.

Re-verification after T-066 on 2026-11-24: `cargo test` — 571 passed
(319 library unit + 15 CLI unit + 32 AI integration + 88 end-to-end + 117 native);
`cargo clippy --all-targets -- -D warnings` clean; debug and release builds
succeeded; all five examples matched; fuzz smoke produced controlled
diagnostics. Recursive-copy tests cover round-trip trees with hidden and empty
directories, binary files, source preservation, existing and missing-parent
targets, file and missing sources, empty/`.`/`..` and NUL target names,
resolved self-containment, depth and entry caps, static `E0301`, dynamic
`R0005`, native `E0401`, root-link following, inner-link and dangling-link
rejection, Unix socket rejection, and non-UTF-8 entry names. The native
recursive-copy tests passed under `-fsanitize=address,undefined` with leak
detection disabled for intentional v0.1 leaks.

Re-verification after T-065 on 2026-11-24: `cargo test` — 550 passed
(312 library unit + 15 CLI unit + 32 AI integration + 81 end-to-end + 110 native);
`cargo clippy --all-targets -- -D warnings` clean; debug and release builds
succeeded; all five examples matched; fuzz smoke produced controlled
diagnostics. Glob tests cover wildcard, class, recursive, hidden, literal,
absolute, repeated-separator, dangling-link, base-link, and symlink cases;
byte-sorted deduplicated output; malformed patterns and component-cap `R0016`;
depth-, result-, and component-cap limits; non-UTF-8 names and NUL `R0015`;
static `E0301`, dynamic `R0005`, and native `E0401` boundaries; and the native
tests ran under ASan/UBSan with leak detection disabled. Metadata-probe tests cover a written regular file and created
directory, missing paths, an empty path, a directory-valued `file_size` target,
false-on-error predicates, static `E0301` errors, dynamic interpreter `R0005`
errors, native `E0401` rejection, and Unix file, directory, and dangling
symlink rules; `file_size` follows file links and reports logical `st_size`.
The four native metadata tests passed under `-fsanitize=address,undefined` with
leak detection disabled for intentional v0.1 leaks.

Re-verification after T-063 on 2026-11-24: `cargo test` — 517 passed
(304 library unit + 15 CLI unit + 32 AI integration + 67 end-to-end + 99 native);
`cargo clippy --all-targets -- -D warnings` clean; debug and release builds
succeeded; all five examples matched; fuzz smoke produced controlled
diagnostics. Recursive-directory tests cover nested creation, existing
directories, empty-path creation, sorted listings, trailing-separator removal,
and cleanup in both engines; regular files blocking creation and regular-file,
missing, and empty removal targets fail with `R0015`; dynamic interpreter `Any`
paths raise `R0005`; native `Any` paths are rejected with `E0401`; inner and
root symbolic links are unlinked without traversing or altering their targets,
including a root link with a trailing separator. T-069 later replaced the
destructive `remove_dir_all(".")` behavior with a pre-access `R0015` refusal
for `.`, `..`, final `.`/`..` components, and the filesystem root, and both
engines preserve the tree in those cases. The
five native recursive tests passed under `-fsanitize=address,undefined` with leak detection disabled
for intentional v0.1 leaks.

Re-verification after T-062 on 2026-11-24: `cargo test` — 507 passed
(304 library unit + 15 CLI unit + 32 AI integration + 62 end-to-end + 94 native);
`cargo clippy --all-targets -- -D warnings` clean; debug and release builds
succeeded; all five examples matched; fuzz smoke produced controlled
diagnostics; the link checker resolved 75 relative links in 100 Markdown files.
Path tests cover empty paths, absolute children, repeated and trailing
separators, root and dot parents, empty names, dotfiles, trailing dots,
multiple dots, missing extensions, Unicode names, and relative/absolute probes,
plus static `E0301` errors, dynamic `R0005` errors, and `E0401` rejection of
dynamically typed arguments for all six builtins. The native path tests passed
under `-fsanitize=address,undefined` with leak detection disabled for
intentional v0.1 leaks.

Re-verification after T-061 on 2026-11-24: `cargo test` — 501 passed
(304 library unit + 15 CLI unit + 32 AI integration + 59 end-to-end + 91 native);
`cargo clippy --all-targets -- -D warnings` clean; debug and release builds
succeeded; all five examples matched; fuzz smoke produced controlled
diagnostics; the link checker resolved 74 relative links in 97 Markdown files.
Rename/copy tests cover file and directory renames, target truncation, cleanup,
byte-preserving binary copies with NUL/invalid-UTF-8/high-bit bytes, static
argument-type errors, fatal `R0015` cases (missing sources, directory copy
sources, missing target parents, rename-over-directory), left-to-right argument
evaluation, and `E0401` rejection of dynamically typed paths. The new native
tests passed under `-fsanitize=address,undefined` with leak detection disabled
for intentional v0.1 leaks. This gate also includes T-060's text builtins
(491 tests: 304 + 15 + 32 + 54 + 86), T-059's directory operations (485-test
gate), and T-058's file I/O (478-test gate).

Re-verification after T-057 on 2026-11-24: `cargo test` — 469 passed
(304 library unit + 15 CLI unit + 32 AI integration + 44 end-to-end + 74 native);
`cargo clippy --all-targets -- -D warnings` clean; debug and release builds
succeeded; all five examples matched. Diversity tests cover trimmed-duplicate
suppression, Jaccard bounds, MMR ordering versus relevance-only ordering,
deterministic repeated runs, invalid-lambda rejection, `--diverse` parsing and
mode rejection, and an offline CLI run with two identical and two distinct
documents. The RAG plumbing benchmark recorded standard hybrid search at
27.175 ms and MMR diverse search at 28.292 ms over 600 chunks, with 10 hits,
0 duplicate texts, and the same top-1 citation. Reranking is a
context-efficiency control; retrieval-quality improvement is not claimed.

Re-verification after T-051 on 2026-11-24: `cargo test` — 437 passed
(281 library unit + 12 CLI unit + 27 AI integration + 44 end-to-end + 73 native);
`cargo clippy --all-targets -- -D warnings` clean; debug and release builds
succeeded; all five examples matched; fuzz smoke produced controlled
diagnostics. Range-pattern tests cover half-open and inclusive bounds,
negative bounds, nested payload positions, or-pattern alternatives, `Int`
exhaustiveness still requiring `_`, `E0301` on non-integer scrutinees, `E0318`
for empty ranges, and interpreter/native differential output.

Re-verification after T-050 on 2026-11-24: `cargo test` — 430 passed
(276 library unit + 12 CLI unit + 27 AI integration + 43 end-to-end + 72 native);
`cargo clippy --all-targets -- -D warnings` clean; debug and release builds
succeeded; all five examples matched; fuzz smoke produced controlled diagnostics
for 9 limit cases and 400 inputs. Native or-pattern tests cover enum
alternatives with multiple payloads, guards, scalar literals, `Bool` coverage,
zero-payload variants, match expressions, and alternatives that bind nested
payloads at different depths. Nested-pattern tests cover recursive complete
and incomplete coverage, payload type mismatch, duplicate bindings, runtime
payload extraction, zero-payload nested variants, enum-typed nested bindings,
nested guards, and differential interpreter/native output. Or-pattern
interpreter tests cover enum and `Bool` coverage, binding-name/type mismatch
rejection, guarded exhaustiveness, and payload binding.

Re-verification after T-047 on 2026-11-24: `cargo test` — 422 passed
(270 library unit + 12 CLI unit + 27 AI integration + 43 end-to-end + 70 native);
`cargo clippy --all-targets -- -D warnings` clean; debug and release builds
succeeded; all five examples matched.

Re-verification after T-013/T-046 on 2026-11-24: `cargo test` — 413 passed
(263 library unit + 12 CLI unit + 27 AI integration + 42 end-to-end + 69 native);
`cargo clippy --all-targets -- -D warnings` clean; debug and release builds
succeeded; all five examples matched; fuzz smoke produced controlled diagnostics
for 9 limit cases and 400 inputs. Module tests cover both import forms, selection,
cycles, path escape, duplicate/unknown names, non-entry statements and `main`,
and resource limits. RAG tests cover deterministic ingestion, stable cited
chunks, pathological lines, strict index validation, semantic/hybrid ranking,
0600 persistence, evaluation metrics, and CLI mode/limit rejection. Live
local/cloud AI and a trained embedding model remain absent.

Earlier re-verification after T-045 on 2026-11-24: `cargo test` — 383 passed (237
library unit + 12 CLI unit + 25 AI integration + 41 end-to-end + 68 native);
`cargo clippy --all-targets -- -D warnings` clean; debug and release builds
succeeded; all five examples matched. Embedding tests cover offline determinism,
normalization, batch limits, cache behavior, loopback `/v1/embeddings`, index
and dimension validation, and `echo` rejection. Live embedding-model evidence
remains blocked by the unavailable local/cloud inference environment; the hash
provider is explicitly lexical, not semantic. GitHub Actions run 45330680289
passed build, all 383 tests, clippy, release build, and differential examples on
commit `52dd1ae`.

### P1 — Core language recovery

1. Tuple patterns once a tuple type exists, and usefulness-based unreachable analysis (range patterns shipped in T-051).
2. Streaming tool calls and interactive cancellation (T-052 follow-ups); live first-token and throughput evidence remains part of T-040.
3. Native memory reclamation design.
4. Async design for language-level AI (T-017).
5. Retrieval arbitrary metadata fields, cross-source deduplication, reranking,
   and a licensed corpus evaluation (T-046/T-054 follow-ups; source-prefix
   filtering and incremental vector reuse ship).

### P2 — Ecosystem

1. Retrieval arbitrary metadata fields, cross-source deduplication, reranking,
   and grounded-generation design without overclaiming (source-prefix filtering
   and incremental vector reuse ship).
2. Model manager with integrity checks.
3. Observability and budgets.
4. Cross-platform CI.
5. Standard library and formatter (the linter half of T-019 is delivered).

### P3+ — Research-only until evidence exists

GPU, SIMD, autodiff, tensors, multi-agent orchestration, MCP, ONNX/GGUF
embedding, and language-level async AI APIs.

## Historical completion gate

The previous engineering directives are **not** complete. The core language
reaches a verified v0.1 subset with file-based modules; the Vantyx rename is
complete in code, documentation, GitHub, and clean-clone CI; and the offline AI
host foundation now includes embeddings, local RAG retrieval, citations, and
retrieval evaluation with tests, plus incremental vector reuse, deletion
pruning, and deterministic MMR diversity reranking; nested payload patterns and
top-level or-patterns are implemented in the interpreter and native backend;
and file/directory I/O, recursive directory creation/removal, rename/copy,
lexical paths, and basic UTF-8 text builtins work in both engines with
differential tests. A trailing-dot native divergence in `path_stem` and
`path_extension` was found and fixed in T-086, including a 2,500-random-path
probe and a focused differential regression. The
following previously requested items remain open: generics, traits, closures, iterators, built-in generic `Result`/error conversion and expression-position `?` (statement-position propagation shipped in T-075), async, concurrency,
FFI, formatter, package manager, standard library, native reclamation,
cycle handling, structural-key map hashing, REPL multi-line history, REPL field
completion, enum benchmarks, cross-platform CI, debug info, compiler
optimization, language-level AI syntax, live local-model evidence (the cloud
path was verified and re-verified on 2026-11-24, but access is not guaranteed),
PDF/HTML/Office ingestion, reranking, and
grounded generation. This audit is the baseline for closing them honestly.
