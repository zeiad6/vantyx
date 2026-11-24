# Completed Tasks

Only tasks with an observed verification command and date are listed here.
See `TASKS.md` for full acceptance criteria and evidence.

CI run 45496041422 passed on `5e1092f` with 613 tests and the differential
fuzz step. CI run 45493833079 passed on `a4fe86b` with 584 tests and the
differential fuzz step. CI run 45488725812 passed on `4982ab3` with 581 tests. The preceding
run 45488152955 failed only the glob result-cap test because ext4 caps hard
links per inode; `4982ab3` spreads the links across four anchors. The status
documentation commit `0491c63` also passed run 45489360135.

- T-074 — Reachability-aware return checking (2026-11-24; 613-test gate;
  `block_returns` walks reachable statements and stops at the first
  `return`/`break`/`continue`, so dead code after a reachable `return` no
  longer causes a false `E0309`; a fully returning `if`/`else` or `match`
  terminates the block; three checker unit tests, two e2e tests, and
  `dead_code_after_return_matches`; ADR-0037).
- T-073 — Static linter and `vantyx lint` (2026-11-24; 613-test gate;
  `L0001` unused binding, `L0002` `var` never reassigned, `L0003` unreachable
  statement; underscore names, parameters, loop variables, and match bindings
  ignored; lint-only findings exit `0`, compile errors exit `1`; 19 unit tests
  and four e2e tests; `docs/linting.md`, specification §14, ADR-0037).
- T-072 — Deterministic differential fuzzer, expanded (2026-11-24; 613-test
  gate; `tools/differential_fuzz.py` now generates eight program shapes:
  scalars/control flow, collections, pattern matching, nested aliasing, typed
  functions, strings, numerics, and runtime-error agreement; 160/160 on each of
  seeds 1, 500, and 1000; CI runs 80 iterations on seed 1; the tool found
  T-070 and T-071). The expansion found that the collection generator used
  `xs[-1]`, an invalid negative index, so every collection program stopped at
  its first index; it now uses `xs[len(xs) - 1]`. Empty `replace` patterns were
  removed because they are runtime errors.
- T-071 — Native short-circuit evaluation (2026-11-24; 584-test gate;
  `and`/`or` no longer hoist and eagerly evaluate the right operand; call
  counts match the interpreter across nested expressions, `if` conditions,
  and match guards; `short_circuit_operands_match` and
  `effectful_binary_operands_keep_left_to_right_order`; ADR-0036).
- T-070 — Native statement-expression conditions (2026-11-24; 584-test gate;
  `if`/`elif`/`while` conditions that lower to `({ ... })` are wrapped as
  `(({ ... }))`, so calls in conditions build; comparisons keep their existing
  parenthesization and stay free of `-Wparentheses-equality`;
  `conditions_with_calls_match`; ADR-0036).
- T-069 — Destructive recursive-removal roots refused (2026-11-24; 581-test
  gate; CI run 45488725812; `remove_dir_all` refuses `.`, `./`, `..`, `../`,
  final `.`/`..` components including `link/.` and `link/..`, and `/`/`//`/`///`
  with `R0015` before any filesystem access in both engines; ordinary roots
  such as `tree`, `./tree`, and `/tmp/tree` remain accepted; five unit tests,
  two e2e tests, two differential native tests; the prior destructive-dot test
  was replaced; ADR-0035 amends ADR-0031).
- T-068 — Curl timeout error mapping (2026-11-24; 573-test gate; CI run
  45488725812; curl exit 28 maps to retryable `AI_E_TIMEOUT` on buffered and
  streaming HTTPS paths, while exit 7 remains `AI_E_CONNECT`; two unit tests;
  T-067 evidence in `docs/ai/LIVE_VERIFICATION.md`).
- T-067 — Live cloud OpenAI-compatible verification (2026-11-24; 581-test
  gate; CI run 45488725812; Gemini discovery, chat, streaming, validated
  extraction, 768-dimension embeddings, semantic RAG/search, and retrieval
  evaluation; 16-check loopback failure harness; re-verified at 20:59Z with
  21/21 checks using `MY_API_KEY` after a transient 503; an earlier retry was
  rejected with HTTP 400 `User location is not supported`; no key stored).
- T-066 — Recursive directory copy (2026-11-24; 571-test gate; `copy_dir_all`
  with new-target semantics, resolved self-containment, raw-name/byte
  copying, inner-link and special-file rejection, 100,000-entry / 128-depth
  caps, `E0301`/`R0005`/`E0401` boundaries, ASan/UBSan differential tests;
  ADR-0034).
- T-065 — Lexical glob expansion (2026-11-24; 550-test gate; `glob` with
  Unicode `*`/`?`/classes, whole-component `**` without symlink traversal,
  explicit-dot hidden rules, byte-sorted deduplicated lexical results,
  `R0015`/`R0016` boundaries, `R0005`/`E0301`/`E0401` type boundaries,
  ASan/UBSan differential tests; ADR-0033).
- T-064 — File metadata probes (2026-11-24; 526-test gate;
  `file_size`/`is_file`/`is_dir`/`is_symlink`; follow-symlink type predicates,
  `lstat` symlink detection including dangling links, logical `st_size` with a
  regular-file requirement and `R0015` failures, `R0005`/`E0301`/`E0401`
  boundaries, ASan/UBSan differential tests; ADR-0032).
- T-063 — Recursive directory creation and removal (2026-11-24; 517-test gate;
  `make_dir_all`/`remove_dir_all`; component-by-component creation accepting
  existing directories and symlinks to directories, empty-path no-op creation,
  symlink-unlinking recursive removal with trailing-separator trimming before
  the root check, `R0015` failures, `R0005`/`E0401` dynamic boundaries,
  ASan/UBSan differential tests; ADR-0031).
- T-062 — Lexical POSIX-style path builtins (2026-11-24; 507-test gate;
  `path_join`/`path_parent`/`path_name`/`path_stem`/`path_extension`/
  `path_is_absolute`; no filesystem access or normalization, UTF-8-preserving
  slices, ordered `path_join`, `E0401` for dynamically typed paths, ASan/UBSan
  differential tests; ADR-0030).
- T-061 — Rename and byte-preserving file copy (2026-11-24; 501-test gate;
  `rename`/`copy_file`; host-defined rename overwrite, regular-file source
  check, exact byte copies including NUL/invalid UTF-8, no metadata
  preservation, `R0015` failures, left-to-right evaluation, ASan/UBSan
  differential tests; ADR-0029).
- T-060 — UTF-8 text builtins (2026-11-24; 491-test gate;
  `split`/`join`/`trim`/`starts_with`/`ends_with`/`replace`; Unicode
  `White_Space` trimming, literal non-overlapping replacement, `R0016`,
  left-to-right evaluation, ASan/UBSan differential tests; ADR-0028).
- T-059 — Single-level directory operations (2026-11-24; 485-test gate;
  `list_dir`/`make_dir`/`remove_file`/`remove_dir`; sorted UTF-8 listings,
  single-level semantics, `R0015` failures, ASan/UBSan differential tests;
  ADR-0027).
- T-058 — Basic UTF-8 file I/O builtins (2026-11-24; 478-test gate;
  `read_file`/`write_file`/`append_file`/`file_exists`; 64 MiB read cap,
  UTF-8/NUL validation, `R0015` failures, ASan/UBSan differential tests;
  ADR-0026).
- T-056 — Native enum construction and match benchmark (2026-11-24;
  seven-workload suite; `enum_ops` native 3.646 ms vs C 3.766 ms and Rust
  4.529 ms; representation rewrite not justified).
- T-055 — Shared ASCII character buffers for native string iteration and
  indexing (2026-11-24; 464-test gate; `string_ops` 8.318 ms → 1.565 ms in the
  latest run).
- T-054 — Incremental RAG re-indexing with vector reuse and deletion pruning
  (2026-11-24; 463-test gate; ADR-0024; CI run 45376386743).
- T-053 — Placeholder attribution correction and history rewrite
  (2026-11-24; CI run 45365926013, 457 tests; REST endpoint now lists only
  `zeiad6`).
- T-052 — Streamed OpenAI-compatible text chat (2026-11-24; 457-test gate;
  ADR-0023).
- T-051 — Integer range patterns (2026-11-24; 437-test gate; ADR-0022).
- T-050 — Native top-level or-pattern lowering (2026-11-24; 430-test gate;
  ADR-0021).
- T-049 — Native nested-pattern lowering (2026-11-24; 429-test gate;
  ADR-0020).
- T-048 — Nested payload patterns and matrix exhaustiveness (2026-11-24;
  429-test gate; ADR-0019).
- T-047 — Top-level or-patterns (2026-11-24; 422-test gate).
- T-046 — Local RAG indexing, hybrid retrieval, citations, and evaluation
  (2026-11-24; 413-test gate; ADR-0018).
- T-045 — Embedding provider interface, bounded cache, and `vantyx ai embed`
  (2026-11-24; CI runs 45330680289 and 45330898922; ADR-0017).
- T-044 — Native scalar-key map hash index (2026-11-24; CI run 45321718008;
  ADR-0016).
- T-013 — Modules and imports (2026-11-24; 413-test gate).
- T-037–T-039, T-041–T-043 — AI host foundation, CLI, OpenAI-compatible
  adapter, discovery sync, and validated structured extraction (2026-11-24).
- T-029–T-033 — REPL upgrade, native enums, bracketed-match diagnostic, scalar
  literal patterns, and match guards (2026-11-24).
- T-035 — Public repository, CI, and v0.1.0 pre-release (2026-11-24).
- T-057 — Deterministic MMR diversity reranking for local RAG search
  (2026-11-24; 469-test gate; `--diverse`, duplicate suppression,
  cosine/Jaccard similarity; ADR-0025; RAG diverse search 28.292 ms vs
  27.175 ms standard over 600 chunks).
- T-056 — Native enum construction and match benchmark (2026-11-24; 464-test
  gate; `enum_ops` native 3.646 ms vs C 3.766 ms and Rust 4.529 ms; CI run
  45380623770).
- T-075 — Result-enum propagation with postfix `?` (2026-11-24; 621-test gate;
  statement-position `let`/assignment/expression use, positional
  first-success/second-failure convention, `E0319` rejections, interpreter and
  native `Try` lowering, ninth fuzzer shape, 180/180 on seeds 1, 500, and
  1000; specification §5.10; ADR-0038).
