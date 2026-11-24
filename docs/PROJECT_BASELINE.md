# Project Baseline — Vantyx (repository: RustZ)

Date: 2026-11-24
Session: re-baselined after the static linter (T-073), reachability-aware return checking (T-074), the deterministic differential fuzzer (T-072), native condition parenthesization (T-070), native short-circuit evaluation (T-071), recursive directory copy (T-066), lexical glob expansion (T-065), file metadata probes (T-064), recursive directory creation/removal (T-063), lexical path builtins (T-062), rename and byte-preserving copy (T-061), UTF-8 text builtins (T-060), single-level directory operations (T-059), basic UTF-8 file I/O builtins (T-058), deterministic MMR diversity reranking for local RAG (T-057), the native enum benchmark (T-056), shared ASCII string buffers (T-055), incremental RAG re-indexing with vector reuse (T-054), streamed OpenAI-compatible chat (T-052), integer range patterns (T-051), native or-pattern lowering (T-050), native nested-pattern lowering (T-049), nested payload patterns (T-048), top-level or-patterns (T-047), modules/imports (T-013), local RAG indexing/retrieval/evaluation (T-046), the embedding provider interface and `vantyx ai embed` (T-045), the native scalar-key map hash index (T-044), the Vantyx rename, the AI host foundation and `vantyx ai` CLI (T-037–T-039), validated structured extraction (T-043), the REPL upgrade (T-029), match guards (T-033), scalar literal patterns (T-032), native enums and `match` (T-030), the bracketed-match diagnostic (T-031), the sustainability research addendum (T-028), launch-kit preparation (T-035), and documentation reconciliation

## Current re-baseline (2026-11-24)

The sections below record the original empty-repository inspection. The repository is no longer empty.

| Area | Current state |
| --- | --- |
| Implementation | Rust 2024, zero third-party dependencies |
| Front end | Indentation-aware lexer, Pratt parser, spans, diagnostics, file-based imports |
| Type system | Gradual typing, `Any` runtime boundaries, nominal structs and enums, recursive matrix exhaustiveness, scalar literal patterns, nested payload patterns, top-level or-patterns with shared binding rules, integer range patterns, and `Bool`-checked match guards |
| Interpreter | Full v0.1 language, reference semantics, checked operations, call-depth guard, enum values, match, scalar literal dispatch, guarded arm selection, ordered or-pattern alternatives, recursive nested-payload bindings, integer range matching, UTF-8 file I/O with a 64 MiB read cap, file metadata probes with follow-symlink type predicates and `lstat` symlink detection, single-level directory listing, creation, and removal, lexical glob expansion with hidden-aware matching and symlink-safe `**`, recursive directory creation and symlink-safe removal with destructive-root refusal (ADR-0035), rename and byte-preserving regular-file copy, new-target recursive directory copy with inner-link and special-file rejection, UTF-8 text splitting, joining, trimming, affix tests, and replacement, and lexical POSIX-style path join/parent/name/stem/extension/absolute helpers |
| Native path | Typed IR + GNU C11 backend for a statically typed subset (`Unit`/`Int`/`Float`/`Bool`/`Str`/`List[T]`/`Map[K,V]`/structs/enums) with UTF-8 string indexing/iteration, an insertion-ordered scalar-key map hash index, imported declarations, `match` with nested payload patterns, top-level or-patterns, integer range comparisons, guards over enums, scalar literals, and ranges, calls and short-circuit expressions in `if`/`elif`/`while` conditions, cycle-safe structural equality, matching UTF-8 file I/O, POSIX `stat`/`lstat` metadata probes, POSIX `DIR`/`readdir`/`lstat` glob expansion with the same hidden and symlink rules, sorted single-level directory operations, symlink-safe recursive directory creation/removal with the same destructive-root refusal, rename and byte-preserving regular-file copy, `realpath`-checked recursive directory copy with raw-name copying, inner-link and special-file rejection, and the same entry/depth caps, UTF-8 text builtins with Unicode `White_Space` trimming, and lexical POSIX-style path helpers |
| Native exclusions | `Any`, `List[Any]`, `Map[Any, *]`, `Any` enum payloads, `match` on `Any`, unsupported loop forms, and dynamically typed file, directory, glob, recursive-copy, rename/copy, metadata, text, or path arguments → `E0401` |
| Tests | 348 library unit + 15 CLI unit + 32 AI integration + 96 end-to-end + 122 differential native = **613 passing** locally on 2026-11-24 (T-073–T-074); CI run 45496041422 green on `5e1092f` with 613 tests; CI run 45493833079 green on `a4fe86b` with 584 tests for T-070–T-072; CI run 45488725812 green on `4982ab3` with 581 tests after run 45488152955 failed only a nonportable hard-link test; the status documentation commit `0491c63` also passed run 45489360135; earlier runs 45376386743, 45378711057, and 45380623770 green for T-054, T-055, and T-056 |
| Benchmarks | Seven-workload baseline vs Python/C/Rust; 2026-11-24 results in `benchmarks/results/` after the map hash index (T-044), ASCII string buffers (T-055), and enum benchmark (T-056); `string_ops` native 1.565 ms and `enum_ops` 3.646 ms vs C 1.242/3.766 ms and Rust 1.799/4.529 ms |
| AI host | Dependency-free `src/ai/` foundation: strict JSON, providers, mock/echo, budgets, embedding provider interface with batch limits, deterministic lexical hash embeddings and bounded cache, OpenAI-compatible `/v1/embeddings`, vector index, BM25, JSON Schema subset, validated structured extraction with bounded repair, streamed OpenAI-compatible text chat over SSE with incremental loopback HTTP and `curl` HTTPS transport, local RAG with incremental vector reuse, deletion pruning, and deterministic MMR diversity reranking, typed tools, bounded agents, redacted metrics, OpenAI-compatible HTTP adapter; `vantyx ai providers/doctor/models/chat/extract/embed/index/search/eval/tokens`; live cloud verification on 2026-11-24 covered discovery, chat, streaming, extraction, 768-dimension embeddings, semantic RAG/search, and retrieval evaluation, with 401/429/timeout covered by a loopback harness; a 20:59Z re-verification passed all 21 checks after an earlier geo-block and a transient 503, so access is not guaranteed |
| Build/tooling | `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, `cargo test` clean; `vantyx run/check/lint/build/repl/ai`; `vantyx lint` with `L0001` unused binding, `L0002` `var` never reassigned, and `L0003` unreachable statement (T-073, ADR-0037); reachability-aware `E0309` (T-074); REPL line editing, history, completion, and expression echo (T-029, ADR-0013); deterministic differential fuzzer `tools/differential_fuzz.py` wired into CI (T-072); manual fuzz harness |
| Launch assets | Public at https://github.com/zeiad6/vantyx (renamed from `zeiad6/auren`); MIT license, green CI on `e3d8bdd` (run 45365926013, 457 tests), v0.1.0 pre-release, pinned feedback issue, contribution/security/conduct policies, `CITATION.cff`, `llms.txt`, and an honest AI-recommendation/promotion kit under `docs/launch/`; creator metadata points to Zeidex (`zeiad6`) and the 2026-11-24 history correction replaced the placeholder commit identity and moved the `v0.1.0` tag |
| Maturity | **Functional prototype**, not production-ready |
| Main risks | Intentional native leaks; O(n) structural-key maps and a fixed scalar-key hash seed; single-machine benchmark baseline only; docs drift; gradual `Any` erodes static guarantees; the linter is advisory only (three conservative rules, no suppression or fixes) and a lint-only run still exits `0`; host-privileged file and directory I/O with no sandbox, fatal errors, non-atomic writes and copies, read-only metadata probes that hide inspection errors, no metadata object or preservation, whole-directory listing, and glob expansion that materializes sorted results and can revisit overlapping `**` subtrees; recursive removal refuses final `.`/`..` components and the filesystem root but still has no confirmation, rollback, trash, depth limit, or entry-count limit, and intermediate components such as `link/sub` can still traverse a symbolic link; recursive copy requires a new target outside the resolved source, rejects inner links and special files, leaves a partial target on failure, preserves no metadata, has only 100,000-entry and 128-depth caps rather than a byte cap, and its containment check is not a sandbox; glob lacks escaping, braces, case folding, and `..` confinement, and explicit links can escape the apparent base; lexical path helpers do not normalize or confine paths; text builtins without regex/case/normalization/grapheme support and with memory-proportional results; no live local-model evidence, a cloud trained embedding model was exercised on 2026-11-24 and re-verified at 20:59Z, but access is not guaranteed and an earlier retry was geo-blocked; flat RAG search with full-corpus scans on every incremental update; multibyte string iteration still allocates per character and concatenation copies exact-size buffers; streaming text only (no tool-call streaming or cancellation); no model/tokenizer/GPU support |
| Next task | Formatter and linter follow-ups (comment-preserving syntax, lint suppression/configuration, broader reachability); live local-model and trained embedding-model verification (T-040) when available, including streaming latency (the cloud path was verified and re-verified on 2026-11-24, but access is not guaranteed); streaming tool calls and cancellation; RAG arbitrary metadata fields, cross-source deduplication, and measured reranking quality; standard-library expansion beyond file I/O, single-level and recursive directories, lexical globbing, rename/copy, recursive copy, metadata probes, lexical paths, and basic text (metadata objects/preservation, recursive move, regex/case/normalization, grapheme-aware text, recoverable `Result` errors, atomic/streaming I/O); immutable concatenation/multibyte iteration optimization; native memory reclamation design (T-013, T-029–T-033, T-035–T-039, T-042–T-069 complete or verified, with cloud live evidence and local-model evidence still blocked) |

The source tree is tracked on branch `main`. Publication and CI status are recorded in `PROJECT_STATUS.md`; do not infer a public URL from this baseline.

## Repository state at initial inspection

- Directory: repository root (the original machine-specific absolute path is intentionally omitted from the public record)
- Contents: **empty** (no files, no Git repository)
- No prior implementation, documentation, tests, or build scripts existed.
- Git was initialized during this session.

## Environment

| Tool | Version | Notes |
| --- | --- | --- |
| Rust | 1.100.0 | installed via rustup during this session |
| Cargo | 1.100.0 | |
| clang | 21.1.8 | available for future native linking |
| LLVM | 21 / 22 shared libraries | available; not yet used |
| GCC | 16.2.0 | available |
| Python | 3.14.10 | available for benchmark comparisons |
| OS | Linux x86_64 | 4 cores, ~1 GiB RAM |
| Network | crates.io reachable | web search API unavailable; primary docs fetched directly |

## Project maturity

**Empty → early prototype.** The baseline is a greenfield language implementation.

## Architecture chosen

- Implementation language: Rust (memory safety, performance, ecosystem).
- Front end: indentation-aware lexer → AST → name resolution → gradual type checker.
- Execution: reference tree-walking interpreter, sharing the checked AST with future backends.
- Future native path: lower checked AST to an explicit IR, then a C or Cranelift/LLVM backend. No backend is claimed until it exists.
- No third-party runtime dependencies in the compiler.

## Initial risks

| Risk | Severity | Mitigation |
| --- | --- | --- |
| Scope explosion across 12 phases | High | Vertical slices; only verified features enter status |
| Interpreter performance mistaken for native performance | High | Separate claims; benchmark later with equivalent workloads |
| Gradual `Any` erodes static safety | Medium | Runtime checks on all dynamic operations; diagnostics encourage annotations |
| Reference semantics for collections can surprise users | Medium | Documented; ownership/borrow analysis planned as opt-in advanced mode |
| `Rc` cycles leak memory | Medium | Documented limitation; cycle collector or ownership analysis planned |
| Indentation lexer edge cases (tabs, blank lines, continuations) | Medium | Dedicated lexer tests |
| Single 1 GiB machine limits heavy backend builds | Medium | Prefer lightweight backend path (C emission) before LLVM integration |
| Compiler panics on malformed input | High | Error-returning pipeline, depth guards, negative tests, and `tools/fuzz_smoke.py` (T-023) |

## Working / broken / missing (historical, initial baseline)

- Working: nothing yet at baseline.
- Broken: not applicable.
- Missing: all compiler stages, runtime, CLI, tooling, standard library, docs.

See the current re-baseline table at the top for the actual state.

## Baseline decision

Proceed with Phase 1 (architecture/specification) and Phase 2 (foundation) in one vertical slice, then verify with real builds and tests. No feature is marked VERIFIED without execution evidence.
