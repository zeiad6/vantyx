# Completed Tasks

Only tasks with an observed verification command and date are listed here.
See `TASKS.md` for full acceptance criteria and evidence.

- T-113 — Display-focused differential fuzz shape (2026-11-27; verified
  locally; commit pending; CI pending; 734-test gate; `tools/differential_fuzz.py`
  adds a twelfth `display_program` shape for a three-node struct/list cycle, a
  self-referential node, a recursive `Chain` enum of depth 0–60,
  `Map[Key, Int]`, `Map[List[Int], Str]`, `Map[Shape, Int]`, nested empty
  collections, quoted UTF-8 strings, and a list of cyclic nodes;
  `tools/test_differential_fuzz.py` now passes 9 tests; 100/100 agreement on
  seeds 1, 2, 3, and 500; no Rust or native behavior changed).
- T-112 — Single-buffer native collection and nominal display (2026-11-27;
  committed as `8911235`; CI pending; 734-test gate; list, map, struct, and enum
  display append into one growable `VantyxStrBuilder` in source order,
  removing quadratic prefix copying and retained prefixes while preserving
  exact bytes, ordering, quoting, cycle markers, and depth limits; one
  differential native test; ADR-0053; focused measurement median
  754.707 ms → 5.737 ms and peak RSS 568,244 KiB → 2,520 KiB).
- T-111 — Single-buffer native string construction (2026-11-27; committed as
  `45c438b`; CI pending; 733-test gate; interpolation and multi-operand string
  `+` chains append typed parts into one growable `VantyxStrBuilder` in source
  order, preserving effects and runtime-error order while avoiding retained
  intermediate strings; two differential native tests; ADR-0052; focused
  measurement median 59.893 ms → 34.414 ms and peak RSS 33,396 KiB →
  17,564 KiB).
- T-110 — CI runs native sanitizers and fuzz smoke (2026-11-27; committed as
  `02cdf10`; CI pending; 731-test gate; `tools/clang-sanitize.sh` adds
  ASan/UBSan to generated native programs, CI runs the native suite through it
  with leak detection disabled, and the main test job now runs the 400-input
  fuzz smoke).
- T-109 — Module graph accepts exactly 256 files (2026-11-27; committed as
  `bffe38f`; CI pending; 731-test gate; `Loader::unit` now rejects only when
  `self.units.len() >= MAX_MODULES`, so the entry file plus 255 imports loads
  and the 257th file produces one `E0507`; two module unit tests cover the
  exact and over-limit boundaries).
- T-108 — Native standard descriptors match Rust startup behavior (2026-11-27;
  committed as `2104c55`; CI pending; 729-test gate; GCC native suite 147/147;
  release build and five examples; generated `main` replaces closed
  descriptors 0–2 with `/dev/null` before user code, matching Rust's startup
  sanitization so a closed stdout silently discards `print` output and later
  file opens cannot reuse a standard descriptor number; one differential
  native test; ADR-0051).
- T-107 — Native print reports output failures at the print site (2026-11-27;
  committed as `9bb2353`; CI pending; 728-test gate; GCC native suite 146/146;
  release build and five examples; native `print` now flushes each
  completed line, checks `fputc`/`fflush`/`ferror`, and raises `R0013` with
  `strerror` and `(os error N)` at the print, while `main` ignores SIGPIPE;
  Linux `/dev/full` and a closed pipe match the interpreter's first error line
  and exit code `2`; two differential native tests; ADR-0039 amendment).
- T-106 — Empty list literals adopt `contains`/`push` value types (2026-11-26;
  locally verified; 726-test gate; GCC native suite 144/144; release build and
  five examples; committed as `9e42a95`; CI pending; `contains([], value)` and
  `push([], value)` previously checked as `List[Any]` and failed natively with
  `E0401`; the checker now checks a direct empty list literal against
  `List[type(value)]`, while non-empty arguments keep the old inference path
  and `E0301` diagnostics; dynamic `Any` values and still-untyped empty
  literals remain under the native `E0401` boundary).
- T-105 — Empty list literal adopts `join`'s `List[Str]` type (2026-11-26;
  pushed in `779783c`; CI run 45936065460 green; 724-test gate;
  `join([], separator)` previously checked as `List[Any]` and
  failed natively with `E0401`; the checker now passes `List[Str]` as the
  expected type for a direct empty list literal while non-empty arguments keep
  the old inference path and list-level `E0301`; `join_accepts_empty_list_literals`
  covers empty and annotated empty lists; `len([])`, `print([])`, and
  unannotated empty-list bindings remain under the native `E0401` boundary).
- T-104 — Native call-depth diagnostics report the call site (2026-11-26;
  723-test gate; the `R0012` guard now reports the call site's file, line, and
  column in both engines, including across modules, instead of the callee
  declaration; `native_call_depth_limit_exits_two` compares the headline and
  location lines; direct and two-module probes match; commit `4269010`; CI run
  45933776137 green).
- T-103 — Deterministic scalar-list sorting (2026-11-26; 723-test gate;
  `sorted(list)` returns a new ascending `List[T]` for `Int`, `Float`, `Str`,
  or `Bool`, leaves the input unchanged, and is stable with `-0.0`/`0.0`
  equal, NaN last, UTF-8 byte order for strings, and `false` before `true`;
  the checker rejects non-list arguments, wrong arity, and other element
  types; dynamic `Any` values are runtime-checked in the interpreter and
  remain under the native `E0401` boundary; the native backend uses a stable
  merge sort instead of `qsort`; four e2e tests and one differential native
  test; ADR-0050; commit `46b7d54`; CI run 45928466694 green).
- T-102 — Label-stratified JSONL dataset splits (2026-11-26; 718-test gate;
  `vantyx ai split --stratify <field>` groups records by the compact JSON
  spelling of a top-level scalar, assigns exact per-label counts, keeps every
  multi-record label in both splits, sends singletons to the majority split,
  preserves input order, reports sorted per-label counts, and bounds memory at
  64 MiB/1,000,000 records/1,000 labels; three unit tests and two CLI tests;
  commit `a331f2d`; CI run 45925745650 green).
- T-101 — Deterministic JSONL dataset shuffle (2026-11-26; 713-test gate;
  `vantyx ai shuffle` validates first, orders records by a domain-separated
  SHA-256 of the seed and exact record bytes with a byte tie-breaker, refuses
  overwrites and same-input output, removes failed output, holds the dataset in
  memory under 64 MiB and 1,000,000 records, normalizes BOM/CRLF/blank lines
  with warnings, and reports seed and input/output provenance; four unit tests
  and two CLI tests; commit `c54c61c`; CI run 45923211883 green).
- T-100 — Exact JSONL duplicate-record removal (2026-11-26; 707-test gate;
  `vantyx ai dedup` validates first, keeps the first exact record in input
  order, uses a domain-separated SHA-256 digest, refuses overwrites and
  same-input output, removes failed output, caps the digest set at 1,000,000
  distinct records, and reports input/output provenance; four unit tests and
  two CLI tests; commit `33c966e`; CI run 45920832199 green).
- T-099 — Native `range(...)` list values outside `for` loops (2026-11-26;
  701-test gate; the interpreter returned a `List[Int]` in any expression
  position while native rejected it with `E0401`; IR now lowers 1–3 `Int`
  bounds to `List[Int]`, codegen binds bounds in source order and builds a
  typed list through a new `vantyx_range_list` helper with the shared
  step-zero/10,000,000-element checks; `range_values_match` and two
  value-position error headlines; clang and GCC; `diff6.py` 31/31 with only
  the known `List[Any]` display limitation; commit `33c966e`; CI run 45920832199 green).
- T-098 — Deterministic train/held-out dataset splits (2026-11-26; 700-test
  gate; `vantyx ai split` validates first, assigns records by a seeded
  content hash independent of line order, refuses overwrites, normalizes
  BOM/CRLF/blank lines with warnings, guarantees non-empty splits, and reports
  actual counts and output SHA-256 values; five unit tests and two CLI tests;
  commit `33c966e`; CI run 45920832199 green).
- T-097 — Filesystem OS-error suffix and glob parse diagnostics (2026-11-26;
  693-test gate; native filesystem failures now append Rust's `(os error N)`,
  and malformed glob patterns use `invalid glob pattern: ...` with quoted range
  endpoints; `runtime_error_headlines_match_interpreter` gained 11 cases;
  `diff3.py` went from 10 mismatches to 44/44; clang and GCC; commit
  `33c966e`; CI run 45920832199 green).
- T-096 — Runtime diagnostic parity for conversions, map keys, range limits,
  and file I/O (2026-11-26; 693-test gate; both engines now agree on the first
  `error[...]` line for `int(Float)` including `Infinity`/`NaN` and the `2^63`
  boundary, missing `Str` map keys, `range` limits, and `read_file`/`list_dir`/
  `glob` failures; `runtime_error_headlines_match_interpreter` and Unix-only
  `file_error_headlines_match_interpreter`; both failed before the fixes;
  clang and GCC; `/tmp/opencode/probe/diff.py` 55/55 and `diff2.py` 34/34;
  commit `33c966e`; CI run 45920832199 green).
- T-095 — JSON Schema conformance and shared preflight (2026-11-26; 691-test
  gate; `required` applies only to objects; empty `enum`/`allOf` fail
  preflight; `anyOf` and sibling keywords evaluate cumulatively; `const`/`enum`
  use JSON Schema instance equality; `minimum`/`maximum` compare exactly beyond
  `2^53`; `extract` and tool calls preflight schemas; 13 `ai::schema` tests,
  `extract_rejects_malformed_schemas_before_calling_the_provider`,
  `rejects_invalid_tool_schemas_before_running`, ADR-0045; commit `33c966e`;
  CI run 45920832199 green).
- T-094 — Dataset schema validation (2026-11-26; 691-test gate;
  `vantyx ai dataset --schema <file>` preflights one bounded schema and
  validates every structurally valid record, reports the first 20 violations
  plus the total, adds an optional `schema` summary, and requires zero
  schema-invalid records for exit `0`; 10 dataset unit tests, 3 dataset CLI
  tests, `docs/ai/DATASET_INSPECTION.md`, ADR-0044; commit `33c966e`; CI run
  45920832199 green).

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
- T-076 — Repository rustfmt baseline and CI formatting gate (2026-11-24;
  mechanical `cargo fmt` across the tree, `cargo fmt --check --all` added to CI,
  621-test gate and clippy/build clean; no semantic changes).
- T-077 — Differential-fuzzer control-flow coverage and bounded shrinking
  (2026-11-24; tenth `control_program` shape, opt-in `--shrink` with
  `--shrink-budget`, timeout exit `124`, seven Python self-tests in CI, tenth
  shape 20/20 on seeds 1–20, full ten-shape run 200/200 on seed 1; later
  seeds also agreed 200/200; CI run 45920832199 green).
- T-085 — Lint rule for unused function parameters (2026-11-24;
  `vantyx lint` previously skipped parameters, so a never-read parameter
  produced no finding; `Binding` now records a `parameter` flag and `L0004`
  reports it distinctly from `L0001`, with underscore names exempt, any read
  suppressing the finding, and loop/match bindings unchanged; six lint unit
  tests and one e2e CLI test; 642-test gate, clippy, fmt, release build, five
  examples linted clean, seven Python self-tests, and fuzzer 20/20 on seed 1;
  commit `33c966e`; CI run 45920832199 green).
- T-084 — Evaluate the argument of `bool` in the native backend (2026-11-24;
  native `bool` lowered a statically non-`Bool` argument to the `R0007` helper
  without emitting the argument, so `print(bool(side()))` skipped `side`'s
  output; the backend now binds the argument to a typed temporary and discards
  it before the rejection, mirroring `type`; one e2e test, one differential
  native test, and a fuzzer error case; 636-test gate, 133 native tests under
  GCC, clippy, fmt, both builds, five examples, fuzzer 40/40 on seeds 1–3 under
  clang and GCC, and fuzz smoke clean; commit `33c966e`; CI run 45920832199 green).
- T-083 — Evaluate expressions interpolated as `Unit` (2026-11-24; native
  interpolation previously rendered `()` without emitting the expression, so
  `"{side()}"` skipped the call's output; `display_value` now keeps the
  expression via `((void)(value), vantyx_str_from_unit())`; one e2e test, one
  differential native test, and fuzzer cases; 634-test gate, 132 native tests
  under GCC, clippy, fmt, both builds, five examples, fuzzer 40/40 on seeds
  1–3 under clang and GCC, and fuzz smoke clean; commit `33c966e`; CI run
  45920832199 green).
- T-082 — Evaluate both operands of native `Unit` equality (2026-11-24;
  native `Unit ==`/`!=` lower to a constant, and the backend hoisted only when
  both sides had effects, so a sole effectful operand was dropped; `binary_expr`
  now hoists both operands whenever either has effects; one e2e test, one
  differential native test, and fuzzer cases; 634-test gate, 132 native tests
  under GCC; commit `33c966e`; CI run 45920832199 green).
- T-081 — Compiler-independent argument order for native `contains` on strings
  (2026-11-24; native string `contains` inlined both operands into a C helper,
  so GCC evaluated effectful arguments right to left while clang and the
  interpreter used left to right; the branch now binds both operands to typed
  temporaries in source order; extended native left-to-right text test plus a
  clang/GCC cross-compiler differential test; 630-test gate, clippy, fmt, both
  builds, five examples, fuzzer 80/80 on seed 1, and fuzz smoke clean; commit
  `33c966e`; CI run 45920832199 green).
- T-080 — Evaluate the argument of `type` in the native backend (2026-11-24;
  native `type(value)` previously yielded a static string without evaluating
  `value`, so nested output, mutation, and runtime errors were skipped and
  `print(type(assert(false)))` exited `0`; the backend now binds the argument
  to a typed temporary and discards it before yielding the name; one e2e test,
  one differential native test, and fuzzer cases; 629-test gate, clippy, fmt,
  fuzzer 60/60 on seed 1, and Python self-tests clean; commit `33c966e`; CI
  run 45920832199 green).
- T-079 — Deterministic `print` evaluation and float `min`/`max` semantics
  (2026-11-24; both engines now evaluate all `print` arguments, render them,
  then write the line, so a failing argument leaves no partial stdout and
  later mutations are visible; float `min`/`max` use one explicit rule (single
  NaN yields the other operand; ties return the second operand) instead of C
  `fmin`/`fmax`; two e2e tests, two differential native tests, and fuzzer
  coverage; 627-test gate, clippy, fmt, both builds, five examples, fuzzer
  100/100 on seeds 1, 2, and 500, and fuzz smoke clean; commit `33c966e`; CI
  run 45920832199 green).
- T-078 — Strict numeric string conversion in the native runtime (2026-11-24;
  `int()`/`float()` now trim Unicode whitespace and validate the interpreter's
  exact grammar before `strtoll`/`strtod`; the 28-form probe went from 8
  divergences to 0; e2e and differential native tests plus expanded fuzzer
  numeric/error shapes; 623-test gate, clippy, fmt, both builds, five examples,
  fuzzer 100/100, and fuzz smoke clean; commit `33c966e`; CI run 45920832199 green).
- T-086 — Trailing-dot path stem and extension parity (2026-11-25; native
  `path_stem`/`path_extension` used an earlier dot when the final dot was
  trailing, so `a/b..` produced `b`/`.` instead of the interpreter's `b..`/`""`;
  the C helpers now share one final-dot search, ADR-0030 states that no earlier
  dot is considered, the e2e and native round trips gained trailing-dot cases,
  and `path_trailing_dot_names_keep_the_whole_stem` compares both engines;
  a 2,514-path differential probe (2,500 random plus 14 fixed) emitted no
  diff; commit `33c966e`; CI run 45920832199 green).
- T-088 — Source-order evaluation of named constructor arguments (2026-11-25;
  named struct-constructor arguments were lowered into declaration order, so
  the native C call evaluated effectful arguments in declaration order while
  the interpreter used the order written; `IrExprKind::Construct` now carries
  `eval_order`, and `construct_expr` evaluates effectful arguments into typed
  temporaries in source order before the constructor call; e2e and
  differential native tests plus the fuzzer's collection shape; specification
  §5.5; 647-test gate, 136 native tests under GCC, clippy, fmt, both builds,
  five examples, and fuzzer 100/100 clean; commit `33c966e`; CI run 45920832199 green).
- T-087 — Lexical function scope in the interpreter (2026-11-25; the
  interpreter searched one flat scope stack from newest to oldest, so a callee
  read or mutated caller locals that shared a global's name and raised `R0010`
  against an immutable caller binding; `Interpreter` now records `frame_base`
  and searches only the active call frame plus root scope zero, restoring the
  boundary on return and error; the checker and native backend already used
  lexical scope; one e2e test, one differential native test, the fuzzer's
  functions shape, specification §3.1/§11, and ADR-0040; 645-test gate, 135
  native tests under GCC, clippy, fmt, and both builds clean; commit
  `33c966e`; CI run 45920832199 green).
- T-090 — Offline GGUF and Safetensors model inspection (2026-11-25/26;
  `vantyx ai inspect` recognizes GGUF v2/v3 and Safetensors, parses only
  bounded metadata and the tensor directory, validates ranges, overlaps, dtype
  sizes, alignment, and known GGML block sizes, and streams the whole file
  through an in-tree SHA-256; reports format, size, tensor/parameter counts,
  architecture/quantization/tokenizer metadata, dtype histogram, and warnings,
  with `--json` and `--metadata`; unsupported/corrupt input is `AI_E_FORMAT`,
  I/O failure is `AI_E_IO`, and usage errors exit `64`; nine unit tests, two
  CLI integration tests, `docs/ai/MODEL_INSPECTION.md`, and ADR-0041; 664-test
  gate, clippy, fmt, and both builds clean; commit `33c966e`; CI run 45920832199 green).
- T-089 — Same-file `copy_file` refusal (2026-11-25/26; opening the target for
  writing truncated the source before any bytes were read, and symbolic-link
  and hard-link aliases did the same; `paths_refer_to_same_file` compares
  device and inode on Unix and canonical paths elsewhere, and both engines now
  raise `R0015` `source and target are the same file` before the target is
  opened; two unit tests, one e2e test, one differential native test, ADR-0029,
  and specification §12; 664-test gate, clippy, fmt, and both builds clean;
  commit `33c966e`; CI run 45920832199 green).
- T-091 — Safetensors shard-index aggregation (2026-11-26; `vantyx ai inspect`
  recognizes a `*.safetensors.index.json`, parses a bounded `weight_map` and
  optional metadata, validates every referenced shard with the T-090
  Safetensors path, cross-checks index tensor names against each shard header,
  and rejects missing, extra, duplicate, escaping, absolute, backslash,
  wrong-extension, non-Safetensors, and nested-index shards; paths must stay
  under the canonical index directory; aggregates tensor/parameter/data-byte
  counts and dtypes, reports index and per-shard SHA-256 digests, and warns when
  `metadata.total_size` disagrees; four unit tests (one Unix-only), one CLI
  integration test, `docs/ai/MODEL_INSPECTION.md`, and ADR-0042; 669-test gate,
  clippy, fmt, both builds, a manual two-shard probe, and the existing
  differential fuzzer clean; commit `33c966e`; CI run 45920832199 green).
- T-092 — Bounded JSONL dataset inspection (2026-11-26; `vantyx ai dataset`
  streams UTF-8 JSONL through SHA-256, accepts a first-line BOM, CRLF, and
  blank lines, requires one top-level object per non-blank line, reports byte
  size, line/blank/record/invalid counts, sorted field/type histograms, and up
  to 20 numbered invalid lines, with `--json`; caps are 256 MiB per file,
  16 MiB per line, and 10,000 distinct top-level fields; invalid content or I/O
  exits `1`, usage exits `64`, and a valid dataset exits `0`; five unit tests,
  two CLI integration tests, `docs/ai/DATASET_INSPECTION.md`, ADR-0043, the
  README AI section, and `examples/ai/train.jsonl`; 676-test gate, clippy, fmt,
  both builds, and the full gate clean; commit `33c966e`; CI run 45920832199 green).
- T-093 — Reusable agent skill library (2026-11-26; thirteen project-local
  OpenCode skills under `.opencode/skills/` with unique kebab-case IDs and
  `name`/`description` frontmatter, each with a Vantyx-specific workflow,
  commands, safety constraints, acceptance criteria, troubleshooting, and an
  example; `docs/AGENT_SKILLS.md` records discovery, inventory, selection, and
  maintenance; a CI validator checks frontmatter, required sections, size, and
  unique IDs, and the harness advertised all thirteen after creation; no Rust
  code changed; commit `33c966e`; CI run 45920832199 green).
