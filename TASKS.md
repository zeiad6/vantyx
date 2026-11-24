# Tasks

Statuses: `BACKLOG`, `READY`, `IN_PROGRESS`, `BLOCKED`, `IMPLEMENTED`, `VERIFIED`.

A task is VERIFIED only after its acceptance criteria are exercised by real tests.

## Phase 2–5 — First vertical slice

### T-001 — Bootstrap Rust project
- Priority: P0
- Status: VERIFIED
- Description: Create Cargo package, module layout, baseline docs, Git repository.
- Acceptance: `cargo build` succeeds; repository layout matches docs.
- Tests: build.

### T-002 — Indentation-aware lexer
- Priority: P0
- Status: VERIFIED
- Description: Tokenize identifiers, keywords, Int/Float/Bool/Str literals, operators, comments, newlines, INDENT/DEDENT; reject tabs in indentation; track byte spans.
- Acceptance: All token categories covered by unit tests; malformed input yields diagnostics, never panics.
- Tests: lexer unit tests, fuzz-ish malformed input cases.

### T-003 — Parser and AST
- Priority: P0
- Status: VERIFIED
- Description: Parse declarations (fn, struct), statements (let/var, assignment, if/elif/else, while, for, return, break, continue), Pratt expressions, calls, indexing, field access, lists, maps, string interpolation.
- Acceptance: AST matches specification; syntax errors report line/column and expected token.
- Tests: parser unit tests + negative tests.

### T-004 — Diagnostic renderer
- Priority: P0
- Status: VERIFIED
- Description: Error codes, severity, source line with caret, help notes, multiple errors.
- Acceptance: Golden-style assertions on rendered diagnostics; non-ASCII spans do not panic.
- Tests: unit tests.

### T-005 — Gradual type checker
- Priority: P0
- Status: VERIFIED
- Description: Scopes, duplicate declarations, let immutability, arity checks, operator typing, struct fields/constructors, return checking, `Any` deferral with runtime checks.
- Acceptance: Type errors rejected; valid programs accepted; no silent unsafe coercion.
- Tests: checker unit tests + e2e negative tests.

### T-006 — Tree-walking runtime
- Priority: P0
- Status: VERIFIED
- Description: Execute checked AST; values for Unit/Int/Float/Bool/Str/List/Map/Struct; checked arithmetic; bounds checks; call depth limit; runtime errors with spans and stack trace.
- Acceptance: Examples run and produce expected output; runtime failures exit nonzero with diagnostics.
- Tests: e2e stdout assertions + negative runtime tests.

### T-007 — CLI
- Priority: P0
- Status: VERIFIED
- Description: `vantyx run`, `vantyx check`, `vantyx repl`, `vantyx version`, `vantyx help`; correct exit codes.
- Acceptance: Commands perform real work; invalid usage explains usage.
- Tests: integration tests invoking the binary.

### T-008 — End-to-end test suite
- Priority: P0
- Status: VERIFIED
- Description: Compile/run programs in tests with captured output; cover control flow, recursion, structs, collections, interpolation, errors.
- Acceptance: `cargo test` passes; at least one test per implemented feature.
- Tests: `tests/e2e.rs`.

### T-009 — REPL
- Priority: P1
- Status: VERIFIED
- Description: Persistent environment, multiline blocks, error recovery.
- Acceptance: Expressions and declarations persist across entries; errors do not crash.
- Implementation: The session accumulates successful entries as source, recompiles them with the new entry, and replays earlier top-level statements with output muted. This reconstructs the environment without retaining interpreter borrows across entries. Failed entries are not appended.
- Tests: `tests/e2e.rs` — bindings and mutable globals persist, functions/structs persist, prior output is not repeated, compile errors and runtime errors leave the session usable.
- Limits: Replay cost grows with session length; redefining an existing name is an error. Line editing, history, completion, and expression echo were added in T-029 (ADR-0013); multi-line history recall and field completion remain in T-034.

## Phase 6+ — Compiler and ecosystem

### T-010 — Explicit IR and native backend
- Priority: P1
- Status: VERIFIED (statically typed subset; ADR-0008)
- Description: Lower checked AST to a typed IR; emit C or Cranelift/LLVM; link executable. C11 backend selected first via ADR-0008; Cranelift/LLVM deferred until benchmarks justify them.
- Acceptance: `vantyx build examples/hello.vtx` produces a native executable with identical output to `run`.
- Tests: `tests/native.rs` — 73 differential tests covering arithmetic, control flow, recursion, structs, enums, scalar literal patterns, nested payload patterns, top-level or-patterns, integer range patterns, match guards, globals, conversions, ranges, lists, maps, strings, equality (including cycles), float display, top-level code, runtime errors, imported modules, and unsupported-construct rejection.
- Scope limits: `Any`, `List[Any]`, `Map[Any, *]`, `Any` enum payloads, dynamic `match` on `Any`, and unsupported loop forms are rejected with `E0401`. Native strings, structs, list buffers, map buffers, and enum values are leaked. A 256-frame call-depth guard exists (`R0012`).

### T-011 — Bytecode VM
- Priority: P2
- Status: BACKLOG
- Description: Stack/register VM for faster startup and a stable execution target.
- Acceptance: Same e2e suite passes on VM; benchmark documented.

### T-012 — Ownership and escape analysis
- Priority: P2
- Status: BACKLOG
- Description: Optional ownership/borrow annotations, move checking, escape analysis for stack allocation.
- Acceptance: Safety guarantees documented and tested; no mandatory syntax for ordinary code.

### T-013 — Modules and imports
- Priority: P2
- Status: VERIFIED
- Description: Add file-based modules with `import "path.vtx"` and `from "path.vtx" import name, ...`, resolving paths relative to the importing file, flattening declarations into one checked program, and supporting the same behavior in the interpreter and native backend.
- Acceptance: imports parse as declarations; paths are plain string literals with a `.vtx` extension; absolute paths and paths that escape the entry file's directory are rejected; missing files, cycles, duplicate imports, and unknown selected names produce targeted diagnostics; non-entry modules contain only imports and top-level declarations (`fn`, `struct`, `enum`), reject executable statements (`E0505`) and `main` (`E0506`), and every file is parsed exactly once; imported declarations share one global scope with checker duplicate detection at the imported span; interpreter and native execution produce identical output; `run -` and the REPL reject imports with `E0510`.
- Implementation: `src/module.rs` (loader, path confinement, cycle detection, declaration selection, `E0501`–`E0510`, 12 unit tests); `Item::Import`/`ImportDecl` in `src/ast.rs`; import parsing and `E0511`/`E0512` in `src/parser.rs`; `Span.file` and `SourceRegistry` in `src/span.rs`; `ExprId` widened to `u64` and seeded per source file so cross-file checker type maps cannot collide; `load_file`/`compile_entry` integration in `src/lib.rs` and `src/main.rs`; per-file source-name restoration in `src/codegen_c.rs`; visible-import rejection in `src/checker.rs`.
- Tests: 12 `module` unit tests (selection, import forms, duplicates, cycles, path escape, missing/unknown names, non-entry statements, `main`, limits); parser import tests; `tests/e2e.rs::modules_run_and_check_end_to_end`; `tests/native.rs::imported_functions_structs_and_enums_match`; existing suites for unchanged behavior.
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 276 library unit + 12 CLI unit + 27 AI integration + 43 end-to-end + 72 native = 430 tests after T-050 (413 at the time of T-013). Manual module smoke: an entry file importing `lib/math.vtx` (struct `Point`, functions `sum` and `double`) printed `14` under both `vantyx run` and `vantyx build`; all five examples match interpreter/native output; `python3 tools/fuzz_smoke.py` reported 9 limit cases and 400 inputs with controlled diagnostics.
- Scope limits: file-based flat imports only; no namespaces, aliases, re-exports, package-relative lookup, or module initializers; all imported declarations share the global namespace; modules may only declare functions, structs, and enums; imports are confined to the entry file's directory; resource limits are 256 files, depth 64, 1 MiB per module, and 8 MiB total source; `run -` and the REPL reject imports; no incremental compilation.

### T-014 — Enums and pattern matching
- Priority: P2
- Status: VERIFIED
- Description: Add closed enums with typed payload variants, construction, exhaustive `match` statements and expressions, pattern bindings, and runtime/checker/native-boundary semantics.
- Acceptance: enum declarations parse and type-check; variant constructors are checked for arity and payload types; closed-enum matches are exhaustive or use `_` (`E0316`); wrong/unknown variants (`E0315`), non-enum scrutinees (`E0314`), and mixed match-expression arm types (`E0301`) are rejected; nested patterns were rejected at the time of T-014 and were later implemented by T-048; interpreter construction, matching, equality, display, `type`, and `R0014` work. At the time of T-014 the native backend rejected enums with `E0401`; that boundary was lifted by T-030 and extended to scalar literals by T-032.
- Implementation: `Item::Enum`, `EnumDecl`, `VariantDecl`, `StmtKind::Match`, `ExprKind::Match`, `Pattern`, `Type::Enum`, `EnumInfo`/`VariantInfo`, `Value::Enum`; checker collection/exhaustiveness/pattern scopes; interpreter construction/matching; IR `E0401` boundary.
- Tests: parser unit test; 16 checker unit tests (payload binding, return paths, exhaustiveness, `Any` warning/fallback, non-enum scrutinee, wrong enum, arity, mixed arm types, global variant/enum uniqueness, named payload rejection, payload type mismatch, unreachable arms, duplicate pattern bindings); 3 value unit tests (display/equality/conformance); 12 e2e tests; 3 native `E0401` tests.
- Verification: 2026-11-22 — `cargo build`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 110 unit + 34 e2e + 57 native = 201 tests. Manual smoke: enum payload construction, match statements/expressions, recursion, equality, display, and native rejection.
- Scope limits: nested payload patterns landed in T-048/T-049; top-level or-patterns landed in T-047; no generic enums; variant names are globally unique; enum values are immutable but payload references alias; native enum/match support landed in T-030; scalar literal patterns landed in T-032; guards landed in T-033.

### T-015 — Closures and iterators
- Priority: P2
- Status: BACKLOG

### T-016 — Error handling (`Result`/`?`)
- Priority: P2
- Status: BACKLOG

### T-017 — Concurrency and async
- Priority: P3
- Status: BACKLOG

### T-018 — Traits and generics
- Priority: P3
- Status: BACKLOG

### T-019 — Formatter and linter
- Priority: P3
- Status: IN_PROGRESS (linter delivered as T-073; formatter remains backlog)
- Description: The linter half is implemented as `vantyx lint` with `L0001`–`L0003` (T-073). The formatter remains future work because the current lexer discards comments; it needs a lossless syntax representation before it can preserve user source.

### T-020 — Package manager
- Priority: P3
- Status: BACKLOG

### T-021 — FFI (C ABI)
- Priority: P3
- Status: BACKLOG

### T-022 — Benchmarks vs Python/Rust/C
- Priority: P3
- Status: VERIFIED
- Description: Equivalent workloads, documented methodology, raw results.
- Acceptance: No performance claim without stored measurements.
- Implementation: `benchmarks/` contains six workloads in `.vtx`, `.py`, `.c`, and `.rs`; `benchmarks/run.py` verifies identical stdout before timing, discards one warm-up, takes 5 measured runs, and measures peak RSS through `benchmarks/measure.c`. Raw results are in `benchmarks/results/2026-11-22.{json,md}` and `benchmarks/results/2026-11-24.{json,md}`.
- Verification: 2026-11-22 — all implementations produced identical output. Native vs interpreter medians: `arith_loop` 5.21 ms vs 1300.9 ms, `fib` 3.31 ms vs 167.5 ms, `list_ops` 4.66 ms vs 383.6 ms, `map_ops` 64.2 ms vs 106.2 ms, `struct_ops` 9.11 ms vs 405.1 ms, `string_ops` 7.72 ms vs 59.8 ms. Native was in the same band as C/Rust on arithmetic/recursion/list/struct; maps and string allocation were the measured gaps. Re-measured on 2026-11-24 after the map hash index (T-044): `map_ops` native 3.597 ms vs interpreter 121.728 ms, C 2.884 ms, and Rust 4.495 ms. Methodology and caveats: `docs/benchmarks.md`.
- Scope limits: single machine, no CPU pinning; not a general language performance claim. C, Rust, and Python map workloads use their hash maps while the interpreter remains linear, so the map comparison is between algorithmically equivalent native implementations, not a claim about the interpreter.

### T-023 — Security audit
- Priority: P2
- Status: VERIFIED
- Description: Audit reachable panics, CLI temp-file handling, resource exhaustion, generated-C safety, and misleading safety claims.
- Acceptance: Findings recorded with file:line, severity, and reproductions; confirmed critical/high issues fixed and tested.
- Implementation: `docs/security-audit-2026-11-22.md` records 12 findings (A-001…A-012), including fixed parser stack exhaustion, UTF-8 span panic, cyclic equality divergence, and 32-bit allocation truncation. Added `tools/fuzz_smoke.py` for reproducible depth-limit and malformed-input testing.
- Verification: 2026-11-22 — 166 tests pass; fuzz smoke harness reports 9 depth/limit + 400 random/mutated inputs with controlled diagnostics; clippy clean.

### T-024 — Native List support
- Priority: P1
- Status: VERIFIED (ADR-0009)
- Description: Extend the typed IR and C backend to represent `List[T]` with Vantyx's reference semantics. Cover list literals, indexing (read/write), `len`, `push`, `pop`, `contains`, `for x in list`, display, bounds checks, aliasing, and differential tests.
- Dependencies: T-010; ADR for native collection representation.
- Acceptance: `vantyx build` accepts a program using a typed list and produces output identical to `vantyx run`, including aliasing/mutation and bounds-error exit code 2. Unsupported element types are rejected with `E0401`, never miscompiled.
- Tests: `tests/native.rs` differential tests for list creation, mutation through aliases, iteration, display, nested lists, recursive structs, bounds errors, and unsupported `List[Any]` rejection.
- Verification: 2026-11-22 — 54 native differential tests pass; `cargo test` 166 passed; clippy clean; all examples build natively with matching output.
- Scope limits: `List[Any]` is rejected; native list buffers are intentionally leaked. Structural equality of distinct cycles is coinductive and matches the interpreter.

### T-025 — Native string indexing and iteration
- Priority: P2
- Status: VERIFIED
- Description: Support `s[i]` (one-character Str, UTF-8 aware) and `for c in s` in the native backend, matching interpreter behavior.
- Dependencies: T-024 runtime helpers.
- Acceptance: Differential tests for ASCII/Unicode indexing, bounds errors, and iteration; unsupported dynamic strings rejected.
- Verification: 2026-11-21 — `s[i]`, `for c in s`, and `len(s)` match the interpreter for ASCII and multi-byte UTF-8; negative/out-of-bounds indices raise `R0003` with matching messages and exit code 2; string index assignment remains rejected by the checker (`E0306`).

### T-026 — Documentation/status reconciliation
- Priority: P1
- Status: VERIFIED
- Description: Update README, baseline, roadmap, architecture, compiler architecture, language specification, memory model, and ADR set to match the verified interpreter + native subset.
- Acceptance: No document claims native compilation is absent; all limitations are explicit.
- Tests: Manual review against `src/`, `tests/`, and observed commands.
- Verification: 2026-11-23 — reconciled again after native enums and `match` (T-030) and the bracketed-match diagnostic (T-031). Every document that claimed native enums were rejected was updated against observed `cargo test` and CLI behavior.

### T-027 — Native `Map[K, V]` support
- Priority: P1
- Status: VERIFIED (ADR-0010, amended by ADR-0016)
- Description: Extend the typed IR and C backend with insertion-ordered maps that preserve interpreter semantics for statically known key/value types. Cover literals, indexing (read/write/compound), `len`, `contains`, `keys`, `values`, key iteration, display, aliasing, structural keys, missing-key errors, and differential tests.
- Dependencies: T-010, T-024; ADR for map representation.
- Acceptance: `vantyx build` accepts typed maps and produces output identical to `vantyx run`, including insertion order, alias mutation, structural struct keys, and missing-key exit code 2. `Any`/untyped map types are rejected with `E0401`, never miscompiled.
- Implementation: `IrType::Map`, `IrExprKind::Map`, `IrStmtKind::ForMap`, map index assignment and compound assignment; `VantyxMap` runtime with parallel key/value buffers, descriptor-driven equality, checked growth, cycle-safe display and equality; checker precision for `keys`/`values`/`contains`/`pop`.
- Tests: `tests/native.rs` — map literals/builtins, mutation/iteration/aliasing, struct keys, nested-map equality, missing key, annotated empty map, map-of-lists struct fields, cyclic equality, untyped/`Any` rejection.
- Verification: 2026-11-22 — 12 map-focused differential tests pass; full suite 166 passed; clippy clean; all examples build natively with matching output.
- Scope limits: scalar-key lookup/insertion use an optional hash index (ADR-0016); struct/enum/list keys still use O(n) structural equality. Map buffers and the index are intentionally leaked; keys are compared structurally, so mutating a key's contents after insertion can make it unreachable (documented in ADR-0010 and ADR-0016).

### T-028 — Prior-art review and lessons
- Priority: P2
- Status: VERIFIED
- Description: Research comparable language projects (Python, Rust, Mojo, Zig, Julia, Swift, Go, Nim, Crystal, Carbon, Vale, Roc, Koka, and others) and record concrete lessons, mistakes to avoid, and design implications for Vantyx.
- Acceptance: `docs/research/prior-art-lessons-2026.md` contains cited findings, explicit "adopt / avoid / revisit" decisions, and links to affected ADRs or tasks. Claims must come from primary sources where possible.
- Verification: 2026-11-22 — `docs/research/prior-art-lessons-2026.md` completed with 25 primary-source URLs checked for HTTP 200, access date, method limitations, adopt/avoid/revisit tables, and links to ADR-0011/T-014/T-030. Web search returned HTTP 402, so findings are limited to directly fetched primary pages and general language knowledge; no third-party benchmarks were run. A sustainability addendum was added on 2026-11-23 using PEP 3000/PEP 373, Go generics and 1.18 notes, Swift ABI notes, Zig 0.15.1 notes, the Mojo open-source announcement, Carbon/Roc READMEs, the Raku rename plan, and Crystal 1.0 release metadata.

### T-029 — REPL completion, history, and expression echo
- Priority: P3
- Status: VERIFIED
- Description: Add line editing/history and tab completion to the REPL, and echo expression-statement values without breaking persistent replay semantics.
- Acceptance: History survives within a session; completion covers keywords, builtins, and visible bindings; expression values print once; existing REPL persistence tests still pass.
- Dependencies: T-009.
- Tests: CLI unit `completion_covers_keywords_builtins_and_names`, `completion_extends_to_the_longest_common_prefix`, `completion_leaves_unknown_prefixes_untouched`, `completion_only_replaces_the_current_token`, `history_deduplicates_and_skips_multiline_entries`, `history_file_round_trips_and_is_private`, `history_file_is_capped`; e2e `repl_echoes_new_expression_values_once`, `repl_history_persists_across_sessions`, `repl_names_lists_visible_bindings`, `repl_does_not_echo_after_compile_errors`; existing REPL persistence and error-recovery tests still pass.
- Verification: 2026-11-23 — `cargo test` 241 passed (126 library unit + 7 CLI unit + 41 e2e + 67 native), clippy clean, debug and release builds succeed. A pty smoke under `script` verified raw-mode editing: Tab completed `pri` to `print` and `alp` to `alpha`, Up recalled the previous entry, Ctrl-C returned to the prompt, Ctrl-D exited 0, and the history file contained the submitted entries. `VANTYX_HISTORY=off` is set by the e2e helper so tests never touch the user's history.
- Limits: multi-line blocks are session-only for recall; completion covers top-level names, not fields or locals; non-TTY input and a missing/failing `stty` fall back to canonical input; echoed strings use `display()` and are unquoted. See ADR-0013.

### T-030 — Native enums and pattern matching
- Priority: P2
- Status: VERIFIED
- Description: Lower enums, variant construction, and `match` to the typed IR and C backend without changing interpreter semantics.
- Acceptance: native builds accept closed enums and exhaustive match for statically known enum types; differential tests cover construction, payload access, nested/recursive enum values, equality, display, wildcard arms, and missing-arm runtime behavior where dynamic; `Any` enum payloads remain rejected with `E0401`; C layout is documented and allocation/reclamation policy is explicit.
- Dependencies: T-014, ADR-0011, ADR-0008.
- Implementation: `IrType::Enum`, `IrEnum`/`IrVariant`, `IrExprKind::ConstructVariant`, `IrExprKind::Match`, `IrPattern`, and `IrMatchBody`; `codegen_c.rs` emits one tagged struct per enum (`tag` + union of per-variant payload structs), one constructor per variant, per-enum display/equality callbacks, and descriptor registration. `match` lowers to an ordered `tag` comparison chain with payload bindings; match expressions use a GNU C11 statement expression. Layout, construction, matching, and leak policy are recorded in ADR-0012.
- Tests: 8 differential `tests/native.rs` cases (match statements, match expressions, struct/payload nesting, mutual recursion, recursive enums, deep display/equality, enum collections/map keys, wildcards/shadowing, cyclic payloads) plus 2 rejection cases (`Any` payload, dynamic `match`), all included in the native suite (65 tests at the time; 66 after T-032; 67 after T-033).
- Verification: 2026-11-23 — `cargo test` 212 passed (112 unit + 35 e2e + 65 native); `cargo clippy --all-targets -- -D warnings` clean; `cargo build --release` succeeds. Manual smoke: `examples/enums.vtx` and a separate enum program using lists, maps, match statements/expressions, equality, display, and `type` produced byte-identical interpreter/native output.
- Scope limits: `Any` enum payloads and `match` on `Any` are rejected with `E0401`; nested payload patterns landed in T-048 with native lowering in T-049; top-level or-patterns landed in T-047 with native lowering in T-050; enum values are heap-allocated and never freed; enum allocation cost is not yet benchmarked. The defensive native `R0014` branch is unreachable for closed enums because the checker requires exhaustiveness, but remains reachable for guarded wildcard chains on `Any` (which native still rejects).

### T-031 — Actionable diagnostic for bracketed `match` expressions
- Priority: P3
- Status: VERIFIED
- Description: A block-shaped match expression cannot be parsed inside parentheses or brackets because the lexer suppresses newlines there. Replace the generic "expected end of line" error with a targeted diagnostic and help text.
- Acceptance: `print(match e: ...)` reports `E0101` with a message explaining that match arms must be on following indented lines and a help note suggesting binding the match to a variable first; existing match parsing is unchanged.
- Implementation: `parse_match_expr` checks for a newline after the header and emits a targeted `E0101` instead of delegating to `expect`.
- Tests: parser unit test `bracketed_match_expression_gets_a_targeted_diagnostic`; end-to-end test `bracketed_match_expression_reports_actionable_help`.
- Verification: 2026-11-23 — both tests pass in the full 212-test run; manual reproduction shows the new message and help.
- Scope limits: this is a diagnostic improvement, not support for bracketed block expressions.

### T-032 — Scalar literal patterns in `match`
- Priority: P2
- Status: VERIFIED
- Description: Allow `match` to dispatch on `Int`, `Str`, and `Bool` values using integer, string, and boolean literal patterns, with `_` as the open-ended fallback. Keep the interpreter as the oracle and lower the same patterns to the native backend.
- Acceptance: integer (including negative), string, and boolean literals parse as patterns; literals type-check against the scrutinee; `Bool` matches require both literals or `_`; `Int`/`Str` matches require `_`; duplicate literals warn (`W0003`); literal-on-enum and variant-on-scalar are rejected (`E0315`); mixed literal/scrutinee types are rejected (`E0301`); interpreter and native output agree.
- Dependencies: T-014, T-030, ADR-0011.
- Implementation: `Pattern::Literal` and `LiteralPattern` in `src/ast.rs`; literal/negative/string parsing in `parse_pattern` with `E0113` for float and interpolated-string patterns; checker literal typing, duplicate tracking, and exhaustiveness in `check_match`/`check_pattern` (coverage bundled in `MatchCoverage`); interpreter matching in `pattern_bindings`; `IrPattern::Literal`/`IrLiteral` lowering; C conditions using integer/boolean comparisons and `strcmp`.
- Tests: parser unit `parses_literal_patterns`; checker units `accepts_literal_patterns_on_scalars`, `rejects_nonexhaustive_int_match`, `rejects_bool_match_missing_a_literal`, `rejects_literal_pattern_type_mismatch`, `rejects_variant_pattern_on_scalar`, `rejects_literal_pattern_on_enum`, `warns_on_duplicate_literal_pattern`; e2e `scalar_literal_patterns_dispatch_at_runtime`; native differential `scalar_literal_match_patterns_match`.
- Verification: 2026-11-23 — `cargo test` 222 passed (120 unit + 36 e2e + 66 native); `cargo clippy --all-targets -- -D warnings` clean; `cargo build` and `cargo build --release` succeed. Manual smoke: a program using `Int` (including `-1`), `Str`, and `Bool` matches, plus a `match` expression on a literal scrutinee, produced byte-identical interpreter/native output (`diff` empty).
- Scope limits: float, interpolated-string, and range patterns are not supported; or-patterns landed in T-047/T-050; nested payload patterns landed in T-048/T-049; guards landed in T-033; string literals are compared with `strcmp`; native matches on `Any` remain rejected with `E0401`; a `Bool` match without `_` must cover both values, while `Int`/`Str` always require `_`.

### T-033 — Match guards
- Priority: P2
- Status: VERIFIED
- Description: Allow an optional `if <expr>` guard after any match pattern. The guard is evaluated only after the pattern matches, with payload bindings in scope, and the arm body runs only when the guard is `Bool` `true`. A guarded arm never contributes to exhaustiveness coverage.
- Acceptance: guards parse after wildcard, variant, and literal patterns in both match statements and expressions; non-`Bool` guards are rejected (`E0301`) and dynamic non-`Bool` guards raise `R0005`; payload bindings are visible to the guard and body; a guarded wildcard or variant does not satisfy exhaustiveness; unreachable arms after an unguarded wildcard still warn (`W0003`) while repeated guarded variants do not; guard evaluation short-circuits arms in source order; interpreter and native output agree.
- Dependencies: T-014, T-032, ADR-0011.
- Implementation: `MatchArm::guard` in `src/ast.rs`; `parse_match_arms` parses `if` guards before the colon; checker checks the guard as `Bool` after pattern bindings are in scope and passes `counts = guard.is_none()` to `check_pattern`; interpreter evaluates the guard in the arm scope (`arm_guard`); IR carries `IrMatchArm::guard` and validates it lowers to `Bool`; C backend builds `arm_condition`, using a GNU C statement expression to test the tag, declare payload bindings, and evaluate the guard when bindings are present.
- Tests: parser unit `parses_match_guards`; checker units `accepts_match_guards_and_payload_bindings`, `rejects_non_bool_match_guard`, `guarded_arms_do_not_satisfy_exhaustiveness`, `warns_on_arm_after_unguarded_wildcard_with_guard`, `duplicate_guarded_variant_arms_are_not_unreachable`; e2e `match_guards_select_arms_at_runtime`; native differential `match_guards_match` (payload guards, `Str` literal guards, wildcard guards with a side-effecting function, and guarded match expressions).
- Verification: 2026-11-23 — `cargo test` 230 passed (126 unit + 37 e2e + 67 native); `cargo clippy --all-targets -- -D warnings` clean; `cargo build` and `cargo build --release` succeed. Manual smoke: guarded enum payloads, guarded literals, guarded wildcards, and guarded match expressions produced byte-identical interpreter/native output (`diff` empty). Negative checks confirmed `E0301` for non-`Bool` guards, `E0316` when guards are the only arms, and `R0005` for a dynamic non-`Bool` guard.
- Scope limits: guards are expressions, not patterns; they cannot introduce bindings; a guard may read but not mutate payload bindings (bindings are immutable); nested payload patterns landed in T-048/T-049; top-level or-patterns landed in T-047/T-050; guards on `Any` scrutinees are dynamically checked and native `match` on `Any` remains `E0401`.

### T-034 — REPL multi-line history and field completion
- Priority: P4
- Status: BACKLOG
- Description: Extend the REPL editor to recall multi-line blocks as a unit and complete struct fields, enum variants in expression position, and names visible in the current top-level scope.
- Acceptance: Up/Down recall a submitted block with its original indentation; Tab completes `p.` to the fields of `Point` when the binding type is known; completion never inserts an ambiguous candidate; history persistence remains line-oriented and private.
- Dependencies: T-029, ADR-0013.
- Verification: Pending.

### T-035 — Public repository and launch kit
- Priority: P2
- Status: VERIFIED
- Description: Publish Vantyx as a public GitHub repository under the short name `vantyx`, with an MIT license, CI, a contribution and security policy, a reproducible launch kit, and AI-discovery material that states the project's limits accurately.
- Acceptance: the public repository builds and tests from a clean checkout; CI is green; `README.md`, `llms.txt`, `PROJECT_STATUS.md`, and `docs/launch/` agree on version, status, test counts, native subset, and known limits; no credentials, build output, or history files are committed; the launch kit contains canonical descriptions, recommendation boundaries, and demo commands that actually run.
- Dependencies: T-028, T-029, ADR-0013.
- Implementation: `.gitignore`, MIT `LICENSE`, `.github/workflows/ci.yml`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `CITATION.cff`, `llms.txt`, `docs/launch/README.md`, `docs/launch/ai-recommendation-kit.md`, and `docs/launch/promotion-kit.md`.
- Verification: 2026-11-23 — local gate passed 241 tests (126 library unit + 7 CLI unit + 41 e2e + 67 native), clippy clean, debug and release builds succeeded, all five examples matched interpreter/native output, the `Any` native rejection demo exited 1 with `E0401`, and all relative launch-material links resolved. Published publicly at https://github.com/zeiad6/vantyx (MIT, default branch `main`, ten discovery topics) on commit `ea4f2ec`. GitHub Actions CI run 45308499010 passed on the launch commit, and run 45308796021 passed after the checkout-action update on commit `86bfcb7`; both covered build, all 241 tests, clippy, release build, and the differential examples. The [v0.1.0 pre-release](https://github.com/zeiad6/vantyx/releases/tag/v0.1.0) is published and [issue #1](https://github.com/zeiad6/vantyx/issues/1) is pinned. No credentials, build output, history files, or generated bytecode were committed.


### T-036 — Rename the project to Vantyx
- Priority: P0
- Status: VERIFIED
- Description: Rename the language, CLI, file extension, crate, environment variables, repository references, tests, examples, benchmarks, and documentation from Auren to Vantyx, preserving history and recording the naming tradeoffs.
- Acceptance: `cargo build` and the full test suite pass under the new names; the extension is `.vtx`; the CLI and crate are `vantyx`; environment variables use `VANTYX_`; no stale `auren`/`AUREN_` references remain outside historical records; the public GitHub repository is renamed and CI passes from a clean clone.
- Dependencies: T-035.
- Implementation: `Cargo.toml` package/lib/bin; `src/` imports; `tests/`; `examples/*.vtx`; `benchmarks/*.vtx`; `.github/workflows/ci.yml`; `AGENTS.md`, `README.md`, `PROJECT_STATUS.md`, `TASKS.md`, `llms.txt`, launch material, and ADR-0014. Historical references remain in ADR-0006, `RESEARCH/NAMING_AUDIT.md`, and this audit.
- Verification: 2026-11-23/24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` passed with 340 tests (208 library unit + 10 CLI unit + 14 AI integration + 41 e2e + 67 native); all five examples ran under the interpreter and as native binaries with identical output. The GitHub repository was renamed to https://github.com/zeiad6/vantyx; CI run 45317367818 passed on commit `e425d04`; a fresh `git clone` of the public repository passed all 340 tests and all five example comparisons. No stale `auren` references remain outside historical ADR/audit records.

### T-037 — Dependency-free AI host foundation
- Priority: P0
- Status: VERIFIED
- Description: Add a host-level AI toolkit under `src/ai/` that keeps the compiler dependency-free: strict JSON, provider abstraction, mock/echo providers, token estimates, budgets, embeddings, vector index, BM25, JSON Schema subset, typed tools, bounded agents, redaction, metrics, SSE, and HTTP transport.
- Acceptance: every module has tests; no third-party crates; no implicit network access; unsupported parameters fail with stable errors; model output is treated as untrusted; tool capabilities deny by default; agent loops always have limits; metrics never contain prompts or secrets.
- Dependencies: ADR-0015, AI-006, AI-011, AI-012, AI-013, AI-016, AI-017, AI-025, AI-026, AI-030.
- Implementation: `src/ai/{json,provider,budget,tokens,mock,embedding,bm25,schema,tools,agent,sse,redact,http,openai,config,metrics}.rs` and `src/ai/mod.rs`; `pub mod ai` in `src/lib.rs`.
- Tests: 82 AI library unit tests plus 14 integration tests in `tests/ai.rs`, covering JSON limits, provider errors, budgets, token estimates, vector math, BM25 ranking, schema validation, denied tool capabilities, agent cancellation and step limits, redaction, SSE parsing, metrics privacy, HTTP policy, and loopback OpenAI-compatible requests.
- Verification: 2026-11-23 — `cargo test` 340 passed (208 library unit + 10 CLI unit + 14 AI integration + 41 e2e + 67 native); `cargo clippy --all-targets -- -D warnings` clean; `cargo build` and `cargo build --release` succeeded.
- Scope limits: heuristic token estimates, in-memory-only vector index, no streaming CLI, no persistent agent state, no write/shell/network tools, and no live provider evidence at T-037 completion (the cloud path was verified later under T-067). Embedding providers were not part of T-037 and were added later by T-045.

### T-038 — `vantyx ai` CLI
- Priority: P1
- Status: VERIFIED
- Description: Expose the AI host foundation through `vantyx ai providers`, `doctor`, `models`, `chat`, and `tokens`, with offline defaults, environment configuration, JSON output, opt-in metrics, and explicit remote-network permission.
- Acceptance: mock and echo chat work offline; `tokens` labels estimates; `--json` emits parseable output; `--metrics` writes redacted JSONL; remote HTTPS requires `--allow-network`; unknown providers, options, and invalid numbers fail with usage errors; loopback OpenAI-compatible chat works without credentials.
- Dependencies: T-037.
- Implementation: `src/ai_cli.rs`; dispatch and usage in `src/main.rs`.
- Tests: 3 CLI unit tests; `tests/ai.rs` covers providers, mock/echo chat, stdin prompts, JSON output, token labelling, network denial, doctor, unknown providers, invalid temperature, metrics privacy, loopback adapter and CLI chat, and conservative capabilities.
- Verification: 2026-11-23 — `cargo test` 340 passed; `./target/debug/vantyx ai providers`, `chat --provider mock`, `chat --provider echo`, `chat --json`, `tokens`, and `doctor --provider mock` were run manually; remote HTTPS without `--allow-network` exited 1 with `AI_E_NETWORK_DISABLED` and help.
- Scope limits: no streaming, no model download or removal, no keyring, and no live local/cloud request was possible at T-038 completion (the cloud path was verified later under T-067; the local path remains blocked).

### T-039 — OpenAI-compatible HTTP adapter
- Priority: P1
- Status: VERIFIED
- Description: Implement a non-streaming OpenAI-compatible chat and model-list adapter behind the provider trait, with bounded response sizes, timeouts, error mapping, redaction, and explicit network policy.
- Acceptance: request mapping includes model, messages, tools, response format, temperature, top-p, max tokens, and seed; response mapping handles text, tool calls, usage, and finish reasons; 401/429/5xx/timeouts map to stable errors; loopback HTTP is allowed without `--allow-network`; remote HTTPS requires it; no secret appears in errors or metrics.
- Dependencies: T-037, ADR-0015.
- Implementation: `src/ai/openai.rs`, `src/ai/http.rs`, `src/ai/sse.rs`, `src/ai/redact.rs`, and `src/ai/config.rs`.
- Tests: provider unit tests with a stub transport and `tests/ai.rs` loopback server tests for adapter and CLI chat, including an `Authorization` header and request body assertion.
- Verification: 2026-11-23 — all adapter tests pass in the 340-test run; a real loopback HTTP server returned a chat response consumed by both the library and the CLI.
- Scope limits: non-streaming only; remote requests require system `curl`; no live cloud or local-model request was made; Ollama native discovery, model pull/remove, and keyring storage are not implemented.

### T-040 — Live local and cloud AI verification
- Priority: P1
- Status: BLOCKED (local model unavailable; cloud path verified under T-067)
- Description: Run one real local model (Ollama or llama.cpp) and one real cloud OpenAI-compatible provider through `vantyx ai chat`, recording the exact commands and redacted evidence.
- Acceptance: both requests succeed; failure paths 401, 429, and timeout are observed or covered by a recorded harness; no key is written to the repository; results and limitations are recorded in `PROJECT_STATUS.md`.
- Dependencies: T-039.
- Blocked by: no local inference server/model in this environment. The cloud OpenAI-compatible path was exercised successfully on 2026-11-24 (T-067) and re-verified at 20:59Z with 21 passing checks; an earlier retry was rejected with HTTP 400 `User location is not supported`, so continuous cloud access is not available.
- Verification: 2026-11-24 — cloud chat, streaming, validated extraction, 768-dimension embeddings, semantic RAG, and retrieval evaluation succeeded through the Gemini OpenAI-compatible endpoint, and a 20:59Z re-verification passed all 21 live checks; the 401/429/timeout harness passed 16 checks across 7 scenarios; local-model evidence is still missing. Evidence: `docs/ai/LIVE_VERIFICATION.md`, `docs/ai/live-evidence-2026-11-24.json`, and `docs/ai/live-evidence-2026-11-24-retry2.json`.

### T-041 — AI discovery and status synchronization
- Priority: P1
- Status: VERIFIED
- Description: Update `README.md`, `llms.txt`, `PROJECT_STATUS.md`, `TASKS.md`, `docs/roadmap.md`, `AGENTS.md`, and the launch kit so the rename, AI host CLI, verified test counts, and remaining limits agree.
- Acceptance: every document states the same version, test count, AI commands, offline/network policy, and known gaps; no document claims live AI, GPU, tensors, or language-level AI that does not exist.
- Dependencies: T-036, T-038, T-039.
- Verification: 2026-11-23 — documentation updated with observed commands and 340-test result; link check and consistency grep performed; no live AI or GPU claim added.

### T-042 — GitHub repository rename and clean-clone CI
- Priority: P0
- Status: VERIFIED
- Description: Rename `zeiad6/auren` to `zeiad6/vantyx`, push the rename and AI foundation, update the remote, and verify CI from a clean clone.
- Acceptance: the old URL redirects; the new URL serves the repository; CI passes build, 340 tests, clippy, release build, and differential examples; the v0.1.0 release and pinned issue remain reachable; no credentials are committed.
- Dependencies: T-036, T-041.
- Verification: 2026-11-23/24 — `gh repo view zeiad6/vantyx` reports the public repository at https://github.com/zeiad6/vantyx on `main`; `git push origin main` advanced `e4361b5..e425d04`; GitHub Actions run 45317367818 passed build, all 340 tests, clippy, release build, and differential examples; a clean `git clone` passed all 340 tests and all five interpreter/native comparisons. The old `auren` URL redirects to the new repository.

### T-043 — Validated structured extraction
- Priority: P1
- Status: VERIFIED
- Description: Add `vantyx ai extract` and `ai::extract` to request JSON, validate it against a caller-supplied JSON Schema subset, and repair invalid output within a hard attempt limit before returning it.
- Acceptance: invalid JSON and schema mismatches are rejected; repair attempts are bounded (max 3) and charged to the shared budget; providers without JSON mode are rejected; schema and response sizes are capped at 1 MiB; output is strict JSON; offline mock and loopback HTTP paths are covered; no untrusted output is returned unvalidated.
- Dependencies: T-037, T-038, T-039.
- Implementation: `src/ai/extract.rs`; `vantyx ai extract` in `src/ai_cli.rs`; schema and walkthrough in `examples/ai/`.
- Tests: 10 library unit tests (first-attempt success, JSON repair, schema repair, repair exhaustion, fenced JSON, missing JSON capability, invalid limits, non-object schemas, fence stripping, usage accumulation); 1 CLI option test; 6 integration tests (mock extraction, repair, exhaustion error, schema file, missing arguments, loopback HTTP extraction with `response_format=json_object`).
- Verification: 2026-11-23/24 — `cargo test` 357 passed (218 library unit + 11 CLI unit + 20 AI integration + 41 e2e + 67 native); `cargo clippy --all-targets -- -D warnings` clean; manual `vantyx ai extract` with `examples/ai/person.schema.json` returned validated JSON and repaired one invalid fixture. GitHub Actions run 45318733338 passed build, all 357 tests, clippy, release build, and differential examples on commit `2d991c6`.
- Scope limits: server-side JSON Schema and constrained decoding are not claimed; validation is strict JSON plus the documented subset; repair is capped at 3; returned values remain untrusted data for the caller.

### T-044 — Native scalar-key map hash index
- Priority: P1
- Status: VERIFIED (ADR-0016)
- Description: Replace linear scalar-key lookup in native maps with an insertion-order-preserving open-addressing index while keeping structural equality for non-scalar keys.
- Acceptance: native output remains identical to the interpreter for integer, string, float, bool, and unit keys; insertion order, replacement in place, `keys`/`values`, display, equality, missing-key errors, aliasing, and structural-key fallback are unchanged; the measured `map_ops` gap closes without regressing other workloads; no new dependency.
- Dependencies: T-027.
- Implementation: `VantyxMap` gains `hashed`, `index_cap`, and `index`; `vantyx_map_hashable`, `vantyx_map_hash`, `vantyx_map_index_rehash`, `vantyx_map_index_reserve`, and `vantyx_map_index_insert`; `vantyx_map_find` probes the index for scalar descriptors and falls back to linear structural search otherwise. ADR-0016 amends ADR-0010.
- Tests: `tests/native.rs` — new `large_scalar_maps_match` covers 5,000 integer keys, 1,000 string keys, float keys, replacement without reordering, `keys` ordering, and `contains`; existing struct-key, nested-map, equality, missing-key, iteration, and `Any` rejection tests cover the fallback and boundaries. The benchmark runner verifies identical output across Vantyx native, interpreter, Python, C, and Rust before timing.
- Verification: 2026-11-24 — `cargo test` 358 passed (218 library unit + 11 CLI unit + 20 AI integration + 41 e2e + 68 native); `cargo clippy --all-targets -- -D warnings` clean; debug and release builds succeed; all five examples produce identical interpreter/native output. `python3 benchmarks/run.py --reps 5` produced `benchmarks/results/2026-11-24.{json,md}`: `map_ops` native median 3.597 ms (2026-11-22: 64.212 ms), interpreter 121.728 ms, C 2.884 ms, Rust 4.495 ms, Python 42.253 ms. GitHub Actions run 45321718008 passed build, all 358 tests, clippy, release build, and differential examples on commit `f55c91c`.
- Scope limits: struct, enum, list, and map keys keep O(n) structural search; the fixed hash seed is not collision-resistant against adversarial key streams; the index adds roughly 16 bytes per entry at a 50% load factor; allocations remain intentionally leaked; single-machine microbenchmark only.

### T-045 — Embedding provider interface, batch embeddings, and bounded cache
- Priority: P1
- Status: VERIFIED (ADR-0017)
- Description: Extend the provider boundary with typed embedding requests and responses, add a deterministic offline embedding provider and a bounded in-memory cache, implement the OpenAI-compatible `/v1/embeddings` adapter, and expose `vantyx ai embed`.
- Acceptance: batches are capped at 128 inputs, 8 KiB per input, 32 KiB per request, and 1–4096 requested dimensions; invalid requests fail before transport; hash vectors are deterministic, finite, and L2-normalized; the cache is keyed by model, dimensions, and text, deduplicates repeated inputs in one batch, is bounded by entries and bytes, and can be cleared; the OpenAI adapter sorts response items by index, rejects duplicate/out-of-range indexes and count or dimension mismatches, checks provider usage against the budget, and redacts secrets; `echo` is rejected for embeddings; stdin batches and JSON output work; no model is downloaded automatically.
- Dependencies: T-037, T-039, ADR-0015.
- Implementation: `src/ai/provider.rs` (embedding request/response, limits, capabilities, default `embed`), `src/ai/embedding_provider.rs` (`HashEmbeddingProvider`, `CachingEmbeddingProvider`), `src/ai/openai.rs` (`/v1/embeddings`), `src/ai/tokens.rs` (embedding token estimate), `src/ai_cli.rs` (`vantyx ai embed`), and `src/ai/mod.rs` exports.
- Tests: 13 `embedding_provider` unit tests (determinism, normalization, order, punctuation-only input, token budget, cache hits/misses, FIFO eviction, model/dimension separation, byte-limit skipping, batch deduplication, capability rejection, clear); embedding-request validation tests; OpenAI embedding request/response, budget, capability, malformed-response, and redaction tests; CLI option and capability tests; 5 integration tests (loopback adapter, loopback CLI, deterministic offline CLI, stdin batch, `echo` rejection).
- Verification: 2026-11-24 — `cargo test` 383 passed (237 library unit + 12 CLI unit + 25 AI integration + 41 e2e + 68 native); `cargo clippy --all-targets -- -D warnings` clean; debug and release builds succeed; all five examples produce identical interpreter/native output. Manual `vantyx ai providers`, `doctor --provider hash`, `embed --provider mock` (human and JSON), and loopback OpenAI-compatible embedding tests succeeded. GitHub Actions run 45330680289 passed build, all 383 tests, clippy, release build, and differential examples on commit `52dd1ae`.
- Scope limits: `HashEmbeddingProvider` is a deterministic lexical feature-hash baseline, not a trained semantic model; the cache is in-memory, process-local, FIFO, and has no TTL or persistence; `openai_compatible().embeddings` describes the adapter route, while actual server and model support varies and is reported at request time; no live local/cloud embedding model was available in this environment at T-045 completion (live cloud embeddings were verified later under T-067; no local embedding model has been run). RAG ingestion, chunking, citations, hybrid fusion, and retrieval evaluation are implemented separately in T-046.

### T-046 — Local RAG indexing, hybrid retrieval, citations, and evaluation
- Priority: P1
- Status: VERIFIED (offline/loopback)
- Description: Build a local-first retrieval pipeline over text, Markdown, and code: bounded recursive ingestion, stable line-cited chunks, provider embeddings, persistent strict-JSON indexes, BM25/semantic/hybrid search with reciprocal-rank fusion, and a reproducible recall/precision/MRR evaluation harness exposed through `vantyx ai index`, `search`, and `eval`.
- Acceptance: ingestion stays under one canonical root, skips symlinks, hidden paths, VCS/build directories, disallowed extensions, and enforces file-count, per-file, total-source, chunk-count, chunk-size, overlap, and embedding-input limits; chunk ids are stable and line ranges remain correct for empty, short, long, and pathological single-line documents; indexes round-trip through strict JSON, reject unknown versions, malformed vectors, mixed sparse/complete vector sets, duplicate ids, out-of-range line spans, non-finite values, and size or dimension mismatches before rebuilding lexical state; saved indexes use atomic replacement and owner-only permissions on Unix; lexical search never calls an embedding provider; semantic and hybrid search require vectors and matching dimensions; hybrid uses reciprocal-rank fusion with deterministic tie breaking; search can filter by relative source prefix before top-k; evaluation reports recall@k, precision@k, MRR, and per-query retrievals without claiming answer grounding.
- Dependencies: T-045 (embedding boundary), `src/ai/bm25.rs`, `src/ai/json.rs`.
- Implementation: `src/ai/rag.rs` (ingestion, chunking, `RagIndex`, persistence, search modes, fusion, evaluation, 14 unit tests); exports in `src/ai/mod.rs`; `vantyx ai index`, `vantyx ai search`, and `vantyx ai eval` in `src/ai_cli.rs` with `--root`, `--out`, `--index`, `--query`, `--fixture`, `--mode`, `--top-k`, `--chunk-chars`, `--overlap-chars`, `--ext`, `--source-prefix`, and `--json`; format and security decision in ADR-0018.
- Tests: 14 `rag` unit tests (line-cited stable chunking, pathological-line splitting without loss, invalid chunk limits, deterministic ingestion filtering, lexical citations, source-prefix filtering, vector requirements, semantic/hybrid ranking, persisted round trip and 0600 permissions, malformed/mixed index rejection, evaluation metrics, fixture parsing, provider capability rejection, query limits, mode parsing); 2 CLI integration tests (`rag_index_search_and_eval_are_offline_and_cited`, `lexical_rag_index_rejects_semantic_search_and_bad_limits`).
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 276 library unit + 12 CLI unit + 27 AI integration + 43 end-to-end + 72 native = 430 tests after T-050 (413 at the time of T-046). Manual offline run indexed three Markdown files with the hash provider at 32 dimensions, returned `rust.md:1-4` as the top hybrid hit for "memory safety" with lexical and semantic ranks, and evaluated a one-query fixture at recall@2 = 1.0 and MRR = 1.0. No network or live model was used. `python3 benchmarks/rag.py --reps 5 --docs 200 --dimensions 64 --top-k 10` indexed 200 generated Markdown documents (600 chunks) at 64 hash dimensions: index median 41.639 ms, hybrid search median 26.720 ms, index size 1,247,300 bytes, 10 cited hits per query; raw results in `benchmarks/results/2026-11-24-rag.{json,md}`. This is a single-machine plumbing benchmark, not a semantic-quality or vector-database claim.
- Scope limits: text, Markdown, and code files only; no PDF, HTML, Office, OCR, audio, or video extraction; no incremental re-index, deletion API, deduplication across sources, arbitrary metadata filters (only relative source-prefix filtering), reranking, or grounded answer generation; the saved index stores full chunk text and vectors as JSON and search scans vectors linearly; the hash provider is a deterministic lexical baseline, not a trained semantic model; no live embedding-model evidence at T-046 completion (live cloud embeddings were verified later under T-067); index persistence is format version 1 with no migration; evaluation labels match exact chunk ids or source paths and do not measure hallucinations or answer quality.

### T-047 — Top-level or-patterns
- Priority: P1
- Status: VERIFIED (interpreter; native lowering added by T-050)
- Description: Allow `|` alternatives inside a single match arm, with one shared binding set per arm, checker-enforced binding consistency, exhaustiveness integration, and native lowering.
- Acceptance: `Circle(size) | Square(size)` parses in match statements and expressions; every alternative binds the same names with identical types (`E0317`); the arm receives one immutable binding set; alternatives are tried in source order at runtime; unguarded alternatives contribute coverage and guarded or-patterns contribute none; duplicate alternatives warn (`W0003`); `Bool` or-patterns can satisfy exhaustiveness without `_`; `|` is rejected as an expression operator; nested payload patterns inside alternatives are handled by T-048; native lowering was rejected with `E0401` at the time of T-047 and was implemented by T-050.
- Dependencies: T-014, T-032, T-033.
- Implementation: `TokenKind::Pipe` and lexer support in `src/token.rs`/`src/lexer.rs`; `Pattern::Or` and `span()` in `src/ast.rs`; alternation in `parse_pattern` in `src/parser.rs`; `check_or_pattern` and `check_or_binding_compatibility` with `E0317` in `src/checker.rs`; ordered alternative matching in `pattern_bindings` in `src/interp.rs`; semantics in `docs/language_specification.md` and `docs/type_system.md`. The original native `E0401` rejection in `src/ir.rs` was replaced by T-050.
- Tests: parser unit test `parses_or_patterns`; 5 checker unit tests (enum coverage, `Bool` literal coverage, binding-name mismatch, binding-type mismatch, guarded or-pattern exhaustiveness); e2e `or_patterns_match_alternatives_and_bind_payloads` (payloads, guards, literals, `Bool` coverage); native `or_patterns_match_alternatives_and_bind_payloads` and `or_patterns_bind_nested_payloads` (added by T-050).
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 276 library unit + 12 CLI unit + 27 AI integration + 43 end-to-end + 72 native = 430 tests. Manual release-binary smoke ran the interpreter or-pattern program and the native build produced identical output after T-050.
- Scope limits: top-level alternatives only; alternatives may contain nested payload patterns (T-048/T-049); no float/range/interpolated-string patterns or or-patterns on `Any` in native code; the `|` token is pattern-only; generated native code re-tests alternative conditions when initializing shared bindings.

### T-048 — Nested payload patterns
- Priority: P1
- Status: VERIFIED (interpreter; native lowering added by T-049)
- Description: Allow patterns to recursively inspect enum payloads, bind names at any nesting level, and check exhaustiveness over the resulting pattern matrix.
- Acceptance: nested variant and literal patterns parse inside payload positions up to 64 levels; bindings from every level join one immutable arm scope; duplicate names are `E0202`; or-pattern alternatives compare nested bindings (`E0317`); nested type mismatches are `E0301`/`E0315`; recursive exhaustiveness accepts complete combinations and rejects partial ones (`E0316`); guards contribute no coverage; native lowering was rejected with `E0401` at the time of T-048 and was implemented by T-049.
- Dependencies: T-014, T-032, T-033, T-047, ADR-0019.
- Implementation: `PatternArg::Nested` and `PatternArg::span` in `src/ast.rs`; recursive field parsing and `MAX_PATTERN_DEPTH` in `src/parser.rs`; nested checking and matrix exhaustiveness (`CoverPattern`, `Constructor`, `matrix_exhaustive`) in `src/checker.rs`; recursive `pattern_bindings` in `src/interp.rs`; ADR-0019 and specification updates. The original native `E0401` rejection in `src/ir.rs` was replaced by T-049.
- Tests: parser `parses_nested_patterns` and `deeply_nested_patterns_are_rejected_without_overflow`; checker `accepts_nested_patterns_without_wildcard`, `rejects_incomplete_nested_pattern_coverage`, `rejects_nested_pattern_payload_type_mismatch`, `rejects_duplicate_bindings_across_nested_patterns`; e2e `nested_patterns_bind_payloads_at_every_level`; native `nested_patterns_match` (added by T-049).
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 276 library unit + 12 CLI unit + 27 AI integration + 43 end-to-end + 72 native = 430 tests after T-050 (429 at T-048). Manual interpreter run printed `7`, `5`, `4` for a recursive tree match; at the time of T-048 the native build exited 1 with `E0401`, and T-049 later made the same program build and match.
- Scope limits: T-048 itself was interpreter-only; native lowering landed in T-049. No nested or-patterns (an or-pattern cannot appear inside a payload position), tuple patterns, range patterns (added by T-051), or float patterns; duplicate partial nested arms are not reported as unreachable; matrix checking is bounded by the parsed pattern tree and can grow with wide variant products.

### T-049 — Native nested-pattern lowering
- Priority: P1
- Status: VERIFIED (interpreter/native differential)
- Description: Lower recursive nested payload patterns to native C without desugaring arm bodies, preserving binding scope and single guard evaluation.
- Acceptance: native builds accept nested variant and literal payload patterns, recursive enums, zero-payload nested variants, enum-typed nested bindings, and guards that read nested bindings; native stdout and exit codes match the interpreter for the differential fixture; or-patterns were rejected with `E0401` at the time of T-049 and were implemented by T-050; no arm body or guard is duplicated; malformed patterns fail closed.
- Dependencies: T-048, ADR-0019, ADR-0020.
- Implementation: recursive `IrPattern::Bind`/`IrPattern::Variant` with `IrPatternField`, `IrFieldPath`, and path-based `IrBinding` in `src/ir.rs`; recursive `lower_pattern` resolving variants against the expected enum type; `pattern_condition`, `collect_pattern_bindings`, `binding_expr`, and `binding_declarations` in `src/codegen_c.rs`; nested pattern descriptor registration; ADR-0020 and specification/status updates.
- Tests: `tests/native.rs::nested_patterns_match` compares interpreter/native stdout and exit codes for recursive `Tree` and `Maybe` matches, nested literals, zero-payload nested variants, enum-typed nested bindings, and guards; the e2e test `nested_patterns_bind_payloads_at_every_level` covers the same interpreter program.
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 276 library unit + 12 CLI unit + 27 AI integration + 43 end-to-end + 72 native = 430 tests after T-050 (429 at T-049). Manual `vantyx build` of a recursive tree program produced output identical to `vantyx run`; the generated C used short-circuit tag tests and path-based binding declarations. All five examples still match interpreter/native output.
- Scope limits: tuple/float patterns and nested or-patterns remain unsupported (integer ranges were added by T-051); no full nested unreachability/usefulness analysis; generated conditions grow with the pattern tree and wide variant products; parser depth is bounded at 64.

### T-050 — Native or-pattern lowering
- Priority: P1
- Status: VERIFIED (interpreter/native differential)
- Description: Lower top-level or-patterns to native C with one shared binding set, ordered alternatives, and single guard evaluation.
- Acceptance: native builds accept or-patterns in match statements and expressions; every alternative is lowered against the same expected type and prefix; the IR rejects mismatched binding signatures as an internal fail-closed error; alternative conditions are tested in source order with `||`; shared bindings are declared once and initialized from the matched alternative; guards that read bindings evaluate once; nested payload patterns inside alternatives work; arm bodies are not duplicated; interpreter and native stdout/exit codes agree.
- Dependencies: T-047, T-048, T-049, ADR-0021.
- Implementation: `IrPattern::Or` and `IrPattern::collect_bindings` in `src/ir.rs`; shared-signature validation in `lower_pattern`; `IrPattern::Or` arms in `pattern_condition`, `collect_pattern_bindings`, `binding_declarations`, `or_binding_declarations`, and descriptor collection in `src/codegen_c.rs`; ADR-0021 and specification/status updates.
- Tests: `tests/native.rs::or_patterns_match_alternatives_and_bind_payloads` covers enum alternatives with multiple payloads, guards, scalar literals, `Bool` coverage, zero-payload variants, and match expressions; `or_patterns_bind_nested_payloads` covers alternatives that bind nested payloads at different depths. The existing e2e `or_patterns_match_alternatives_and_bind_payloads` covers the interpreter.
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 276 library unit + 12 CLI unit + 27 AI integration + 43 end-to-end + 72 native = 430 tests. Manual `vantyx build` of the or-pattern fixtures produced output identical to `vantyx run`; all five examples still match interpreter/native output; fuzz smoke reported 9 limit cases and 400 inputs with controlled diagnostics.
- Scope limits: top-level alternatives only; no nested or-patterns, tuple/float patterns, or native `Any` matching; generated conditions re-test alternatives when initializing bindings; no usefulness-based redundant-alternative warning beyond duplicate literal/alternative coverage. (Range patterns were added separately in T-051.)

### T-051 — Integer range patterns
- Priority: P1
- Status: VERIFIED (interpreter/native differential)
- Description: Add pattern-only integer ranges `low..high` (half-open) and `low..=high` (inclusive) to match statements, match expressions, nested payload positions, and or-pattern alternatives, with checker validation and native lowering.
- Acceptance: both forms parse with non-negative and negative literal bounds; a range requires an `Int` or `Any` scrutinee (`E0301`); an empty range is `E0318`; ranges are refutable and never satisfy `Int` exhaustiveness, so `_` is still required; ranges work in nested payload positions and or-pattern alternatives; interpreter and native stdout/exit codes agree at every boundary; float bounds and non-literal bounds are rejected with actionable diagnostics.
- Dependencies: T-032, T-033, T-047, T-048, T-049, T-050, ADR-0011, ADR-0022.
- Implementation: `TokenKind::DotDot`/`DotDotEq` in `src/token.rs` and `src/lexer.rs`; `Pattern::Range` in `src/ast.rs`; `finish_range_pattern` and range suffixes in `src/parser.rs`; `Pattern::Range` checking and `E0318` in `src/checker.rs`; range matching in `src/interp.rs`; `IrPattern::Range` and lowering in `src/ir.rs`; comparison emission in `src/codegen_c.rs`; ADR-0022 and specification/status updates.
- Tests: parser `parses_range_patterns` and `parses_nested_range_patterns`; checker `accepts_range_patterns_on_int`, `rejects_range_pattern_on_non_int`, and `rejects_empty_range_patterns`; e2e `range_patterns_match_half_open_and_inclusive_bounds` (boundaries, negative bounds, nested payload ranges, or-pattern alternatives); native `range_patterns_match_half_open_and_inclusive_bounds` compares interpreter/native output for the same program.
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 281 library unit + 12 CLI unit + 27 AI integration + 44 end-to-end + 73 native = 437 tests. Manual `vantyx run` and `vantyx build` of the range fixture produced identical output for half-open, inclusive, negative, nested, and or-pattern cases; `vantyx check` rejected a `Str` range (`E0301`), empty ranges (`E0318`), and a float bound (`E0113`). All five examples still match interpreter/native output.
- Scope limits: pattern-only syntax; no range expressions, iterators over ranges, float ranges, character ranges, or computed bounds; overlapping and redundant intervals are not diagnosed beyond exact duplicate literals; `Int` matches still require `_`.

### T-052 — Streamed OpenAI-compatible chat
- Priority: P1
- Status: VERIFIED (offline loopback/stub; live provider blocked)
- Description: Stream text deltas from OpenAI-compatible chat endpoints through the existing provider abstraction, HTTP transport, and `vantyx ai chat` CLI without adding a dependency or weakening the local-by-default network policy.
- Acceptance: `Provider::chat_stream` defaults to `AI_E_UNSUPPORTED`; the OpenAI-compatible adapter sends `stream: true` and `stream_options.include_usage` with `Accept: text/event-stream`, forwards text deltas, aggregates the final message, captures finish reason and usage, and records the budget once; split UTF-8, `[DONE]`, post-`[DONE]` frames, error frames, 429 mapping, and omitted usage are covered; tools and tool-call deltas are rejected with `AI_E_UNSUPPORTED`; the CLI prints deltas incrementally and rejects `--stream` with `--json` or non-OpenAI providers before transport; loopback streaming decodes content-length, chunked, and close-delimited bodies; HTTPS streaming works through the existing `curl` boundary; no live provider claim is made.
- Dependencies: T-037, T-039, ADR-0015, ADR-0023.
- Implementation: `Provider::chat_stream` and `ProviderCapabilities.streaming` in `src/ai/provider.rs`; `SseParser::push_bytes` with UTF-8 carry in `src/ai/sse.rs`; `Transport::stream`, `StreamHandlers`, incremental loopback decoding, interim 1xx handling, and `curl --include --no-buffer` HTTPS streaming in `src/ai/http.rs`; `OpenAiProvider::chat_stream`, `ChatStreamState`, and streaming request JSON in `src/ai/openai.rs`; `--stream` parsing, validation, live output, and capability reporting in `src/ai_cli.rs`; `tools/streaming_smoke.py`; ADR-0023 and documentation updates.
- Tests: `src/ai/openai.rs` streaming tests (`streams_text_deltas_and_usage`, `stream_preserves_utf8_split_across_chunks`, `stream_ignores_frames_after_done`, `stream_redacts_error_frames`, `stream_maps_http_errors`, `stream_rejects_tools_and_tool_call_deltas`, `stream_estimates_usage_when_the_provider_omits_it`); `src/ai/http.rs` content-length/chunked/close-delimited/curl-head tests; `src/ai/sse.rs` split-UTF-8 and framing tests; `src/ai_cli.rs` `parses_the_stream_flag` and capability reporting; `tests/ai.rs` loopback SSE CLI test, provider-error frame test, and option-rejection test; `tools/streaming_smoke.py` verifies HTTP and HTTPS deltas arrive incrementally with a self-signed certificate.
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 297 library unit + 13 CLI unit + 30 AI integration + 44 end-to-end + 73 native = 457 tests. `python3 tools/streaming_smoke.py --binary target/debug/vantyx` reported HTTP deltas first at 0.00 s and last at 1.20 s, and HTTPS deltas first at 0.06 s and last at 1.22 s, with `stream: true` and `Accept: text/event-stream` observed on the wire. All five examples still match interpreter/native output; no live local or cloud model was available.
- Scope limits: text deltas only; streaming tool calls, streaming structured-output guarantees, interactive cancellation, and provider-side constrained decoding are not implemented; HTTPS streaming depends on a system `curl` with `--no-buffer`; usage is estimated when the provider omits the usage frame; SSE `event:`/`id:` fields are ignored; no live-provider latency or throughput evidence exists.

### T-053 — Correct placeholder commit attribution
- Priority: P1
- Status: VERIFIED (rewritten history, public contributors page, and refreshed REST API)
- Description: Remove the unrelated `auren` account from the public repository's contributor attribution. All twelve historical commits used the placeholder identity `Auren Contributors <auren@users.noreply.github.com>`, which GitHub maps to an unrelated user with no work in the repository.
- Acceptance: every commit reachable from `main` and `v0.1.0` is authored and committed by `Zeidex <48607619+zeiad6@users.noreply.github.com>`; the GitHub contributors API no longer lists `auren`; the working tree, tests, examples, and release tag content are unchanged; `.mailmap` remains for any external references to the old hashes; documentation records that commit hashes and earlier CI run references were replaced.
- Dependencies: T-035, T-036, T-042.
- Implementation: `git filter-branch --env-filter` over `--branches --tags` to replace author and committer identity, `--tag-name-filter cat` to move `v0.1.0`, and a force-with-lease push of `main` and the tag. `.mailmap`, `PROJECT_STATUS.md`, `AGENTS.md`, `docs/PROJECT_BASELINE.md`, and this entry record the correction.
- Verification: 2026-11-24 — all reachable commits report the Zeidex identity; the public contributors page lists only `zeiad6`; the REST contributors endpoint now also lists only `zeiad6` (15 contributions), so the acceptance check is closed; the full local gate passed before the rewrite (457 tests, clippy, release build, five examples, fuzz smoke, link check, streaming smoke); CI run 45365926013 passed on the rewritten `e3d8bdd` with 457 tests; a clean clone of the rewritten history passed 457 tests; the rewritten tree is byte-identical to the verified tree.
- Scope limits: old commit hashes and CI run references remain historical and no longer resolve on the default branch; external clones must re-fetch rather than merge old history; no other contributor is credited without verified work.

### T-054 — Incremental RAG re-indexing with vector reuse and pruning
- Priority: P1
- Status: VERIFIED
- Description: Update an existing semantic retrieval index from a fresh corpus scan without re-embedding unchanged chunks, while pruning deleted chunks and preserving provider/model/dimension compatibility.
- Acceptance: chunks whose stable id, provider, model, and dimensions match reuse their stored vector; new or changed chunks are embedded; chunks absent from the current corpus are pruned; a no-op update makes no provider call; an empty corpus produces a valid empty semantic index that round-trips and searches to zero hits; provider, model, dimension, lexical-index, and missing-index mismatches are rejected with actionable diagnostics; the CLI exposes `--incremental` with JSON/human reuse counts; search after an update reflects additions and deletions; the full local gate passes and the RAG plumbing benchmark records no-op and one-changed-chunk update timings.
- Dependencies: T-045, T-046, ADR-0018, ADR-0024.
- Implementation: `ReuseStats` and `IncrementalPlan` with `RagIndex::plan_incremental`, `validate_chunk_set`, empty-index search/loader handling in `src/ai/rag.rs`; `--incremental` parsing, continuity checks, conditional provider construction, atomic save, and reuse reporting in `src/ai_cli.rs`; exports in `src/ai/mod.rs`; unit and CLI integration tests; `benchmarks/rag.py` incremental phases; ADR-0024 and documentation updates.
- Tests: `src/ai/rag.rs` (`incremental_plan_reuses_unchanged_chunks_and_prunes_removed`, `incremental_plan_rejects_configuration_and_chunk_mismatches`, `incremental_plan_can_remove_every_chunk_and_round_trip`, `incremental_finish_requires_one_vector_per_pending_chunk`); `src/ai_cli.rs` (`parses_the_incremental_index_flag`); `tests/ai.rs` (`rag_incremental_index_reuses_vectors_and_prunes_deleted_documents`).
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 301 library unit + 14 CLI unit + 31 AI integration + 44 end-to-end + 73 native = 463 tests. All five examples ran under the interpreter and as native binaries with identical output. The extended `python3 benchmarks/rag.py --reps 5 --docs 200 --dimensions 64 --top-k 10` recorded a 42.746 ms full index build, 27.659 ms hybrid search, 47.377 ms no-op incremental update (600 reused, 0 embedded), and 48.049 ms one-changed-chunk update (599 reused, 1 embedded) in `benchmarks/results/2026-11-24-rag.{json,md}`. The manual 3-document reproduction reported `previous_chunks:3`, `reused_chunks:1`, `embedded_chunks:2`, `removed_chunks:2`, `removed_sources:1`, `added_sources:1`.
- Scope limits: every update still walks and chunks the complete corpus; there is no file-mtime or content-manifest shortcut, so ingestion cost is not avoided. The current `--root` is treated as the complete corpus, so partial-prefix updates and multi-root merging are not supported. There is no deduplication across sources, arbitrary metadata filtering, reranking, or grounded generation. The hash provider is a deterministic lexical baseline, not a trained semantic model, and no live embedding-model evidence existed at T-054 completion (live cloud embeddings were verified later under T-067). The index format remains version 1; an empty vector index is now valid.

### T-055 — Shared ASCII character buffers for native string iteration and indexing
- Priority: P1
- Status: VERIFIED
- Description: Remove the per-character heap allocation in native `for c in text` loops and `text[i]` indexing for single-byte ASCII characters, which dominated the measured `string_ops` gap.
- Acceptance: single-byte ASCII characters share immutable static buffers; multibyte UTF-8 characters still allocate and behave identically; interpreter and native output match for iteration, indexing, reversal, and concatenation; the measured `string_ops` native median improves without changing semantics or the public language surface.
- Dependencies: T-022, T-044.
- Implementation: `vantyx_str_slice` and a lazily initialized static 128-entry ASCII table in `src/codegen_c.rs`; `IrStmtKind::ForStr` and `vantyx_str_index` call the shared helper. Native strings are never mutated in place and the runtime is single-threaded, so shared pointers are safe.
- Tests: `tests/native.rs::string_ascii_indexing_iteration_and_concat_match`; the existing Unicode indexing/iteration and string control-flow differential tests still pass.
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 301 library unit + 14 CLI unit + 31 AI integration + 44 end-to-end + 74 native = 464 tests. All five examples matched. `python3 benchmarks/run.py --reps 5` recorded `string_ops` native median 3.190 ms (min 3.134, max 3.986) versus 8.318 ms before the change, with C 2.796 ms and Rust 3.733 ms in the same run; an isolated `--workload string_ops` run measured 1.616 ms under lighter load. Raw results: `benchmarks/results/2026-11-24.{json,md}`.
- Scope limits: only single-byte ASCII characters use the shared table; multibyte UTF-8 characters still allocate a string per character. Immutable concatenation still allocates a fresh exact-size buffer, and native allocations remain intentionally leaked. The table is process-global and immutable after initialization; thread safety is not claimed because the runtime is single-threaded.

### T-056 — Native enum construction and match benchmark
- Priority: P1
- Status: VERIFIED
- Description: Measure the native enum representation against equivalent C and Rust implementations before deciding whether to change it. The backend heap-allocates every enum value; this task establishes whether that cost is material.
- Acceptance: an `enum_ops` workload exists in Vantyx, Python, C, and Rust; all four produce identical output; the harness includes it in the default suite; medians, peak RSS, build time, and binary size are recorded under `benchmarks/results/`; the result is interpreted honestly and does not trigger an unjustified representation rewrite.
- Dependencies: T-022, T-030.
- Implementation: `benchmarks/enum_ops.{vtx,py,c,rs}`; `enum_ops` added to `WORKLOADS` and `DESCRIPTIONS` in `benchmarks/run.py`; documentation tables updated.
- Tests: The harness verifies identical stdout across all five runs (interpreter, native, Python, C, Rust) before timing. Manual comparison produced `624732 204439` for every implementation.
- Verification: 2026-11-24 — `python3 benchmarks/run.py --reps 5` recorded `enum_ops` native median 3.646 ms (min 3.216, max 4.083), C 3.766 ms, Rust 4.529 ms, Python 164.903 ms, and interpreter 687.079 ms; peak RSS 2.01 MiB native, 3.72 MiB interpreter, 1.60 MiB C, 2.09 MiB Rust; build 0.143 s and binary 16,784 bytes. A separate targeted run measured native 4.208 ms, C 3.557 ms, and Rust 3.370 ms. The run-to-run spread shows native enums are in the same band as C/Rust, so the heap representation is not the dominant cost on this workload and a rewrite is not justified by this evidence.
- Scope limits: the workload exercises construction, payload matching, and integer operations, not enum equality, nested enums, display formatting, or allocation growth over long-running processes. Native allocations remain intentionally leaked. The benchmark does not prove that the enum representation is optimal, only that it is competitive on this workload.

### T-057 — Deterministic MMR diversity reranking for local RAG search
- Priority: P1
- Status: VERIFIED
- Description: Reduce redundant near-duplicate chunks in retrieval results with an opt-in, deterministic maximal-marginal-relevance (MMR) reranker that works with both vector and lexical indexes.
- Acceptance: `search_with_source_diverse` validates a finite `lambda` in `[0.0, 1.0]`; candidates come from a bounded wider pool; identical trimmed chunk text is suppressed keeping the highest-ranked copy; pairwise similarity is cosine over stored vectors when every candidate has one and BM25-token Jaccard otherwise; relevance is normalized by the best candidate score; ties keep candidate order so repeated runs are identical; `--diverse` applies the reranker for `vantyx ai search` and is rejected for `index`/`eval`; JSON and human output report the rerank state; the full local gate passes and the RAG benchmark records standard vs diverse search.
- Dependencies: T-045, T-046, T-054, ADR-0018, ADR-0025.
- Implementation: `DEFAULT_MMR_LAMBDA`, `search_with_source_diverse`, `diversify_hits`, `deduplicate_hits`, `jaccard_similarity`, `mmr_select`, and `diversity_candidates` in `src/ai/rag.rs`; `--diverse` parsing, rejection for `index`/`eval`, and `rerank` reporting in `src/ai_cli.rs`; exports in `src/ai/mod.rs`; unit and CLI integration tests; a diverse-search phase in `benchmarks/rag.py`; ADR-0025 and documentation updates.
- Tests: `src/ai/rag.rs` (`deduplication_keeps_the_first_copy_and_jaccard_is_bounded`, `mmr_balances_relevance_and_diversity_deterministically`, `diverse_search_deduplicates_and_rejects_invalid_lambda`); `src/ai_cli.rs` (`parses_the_diverse_search_flag`); `tests/ai.rs` (`rag_diverse_search_deduplicates_and_marks_reranking`).
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 304 library unit + 15 CLI unit + 32 AI integration + 44 end-to-end + 74 native = 469 tests. All five examples ran under the interpreter and as native binaries with identical output. `python3 benchmarks/rag.py --reps 5 --docs 200 --dimensions 64 --top-k 10` recorded hybrid search median 27.175 ms and MMR diverse search median 28.292 ms (+1.117 ms) with 10 hits, 0 duplicate texts, and the same top-1 citation (`doc-0007.md:1`); raw results in `benchmarks/results/2026-11-24-rag.{json,md}`.
- Scope limits: reranking is a deterministic context-efficiency control, not measured retrieval-quality improvement. Exact-duplicate suppression is textual, not semantic; paraphrases are only penalized through cosine/Jaccard similarity. Similarity is O(n²) over the bounded candidate pool (at most `MAX_TOP_K`). The CLI exposes only the fixed default `lambda` of 0.7; library callers can choose any value. There is still no cross-source deduplication, metadata filtering, grounded generation, or trained embedding-model evidence.

### T-058 — Basic UTF-8 file I/O builtins
- Priority: P1
- Status: VERIFIED (interpreter/native differential)
- Description: Add `read_file`, `write_file`, `append_file`, and `file_exists` to the interpreter and native backend with identical UTF-8, NUL, size-limit, and error behavior.
- Acceptance: all four builtins are statically checked (`Str` paths, `Str` contents); `read_file` returns only valid UTF-8 without NUL bytes and refuses files over 64 MiB; `write_file` creates/truncates and `append_file` creates/appends; failures raise `R0015` and exit 2 in both engines; `file_exists` returns `false` for missing or uninspectable paths and is the only non-failing probe; relative paths resolve against the process working directory; argument evaluation is left to right; statically non-`Str` arguments are `E0301`, and dynamically typed (`Any`) native paths are rejected with `E0401`; no new dependency is added.
- Dependencies: T-006, T-007, T-010, T-025, ADR-0007, ADR-0026.
- Implementation: `BUILTINS` and `E0301` checks in `src/checker.rs`; builtin dispatch and `MAX_FILE_BYTES` in `src/interp.rs`; `IrType` validation in `src/ir.rs`; builtin call emission, `vantyx_read_file`, `vantyx_write_file`, `vantyx_file_exists`, and a UTF-8 validator in `src/codegen_c.rs`; ADR-0026 and specification/status updates.
- Tests: `tests/e2e.rs` (`file_io_round_trips_and_reports_existence`, `file_io_rejects_invalid_utf8_and_nul_contents`, `file_io_rejects_non_str_arguments_at_compile_time`); `tests/native.rs` (`file_io_matches`, `file_io_evaluates_arguments_left_to_right`, `file_io_read_errors_match`, `file_io_write_and_missing_read_errors_match`, `file_io_size_limit_matches`, `file_io_any_paths_are_rejected_with_e0401`). The error test covers invalid leads, overlong encodings, surrogates, out-of-range code points, truncated sequences, and NUL bytes. The size test creates a sparse 64 MiB + 1 file and checks both engines report `R0015`. The boundary test confirms file builtins are not exempt from the native `Any` rejection.
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 304 library unit + 15 CLI unit + 32 AI integration + 47 end-to-end + 80 native = 478 tests. All five examples matched interpreter/native output; fuzz smoke reported 9 depth/limit cases and 400 random/mutated inputs with controlled diagnostics; the link check resolved 71 relative links in 92 Markdown files. The six file-I/O native tests also passed under `-fsanitize=address,undefined` with leak detection disabled (v0.1 allocations are intentionally leaked). Manual smoke: `write_file` + `append_file` + `read_file` + `file_exists` produced `hello world`, `true`, and `false` identically under `vantyx run` and `vantyx build`; invalid UTF-8 and NUL files exited 2 with `R0015` in both engines.
- Scope limits: synchronous only; no metadata preservation, glob, stdin/stdout streams, non-blocking I/O, or atomic writes (metadata probes were added in T-064). Errors are fatal `R0015` rather than a recoverable `Result`. Paths are host paths with no sandbox; check-then-read is subject to TOCTOU races, and a failed write can leave a partial file. The 64 MiB cap is fixed and applies to `read_file` only. Directory operations were added separately in T-059; rename and byte-preserving copy were added in T-061.

### T-059 — Single-level directory operations
- Priority: P1
- Status: VERIFIED (interpreter/native differential)
- Description: Add `list_dir`, `make_dir`, `remove_file`, and `remove_dir` to the interpreter and native backend with deterministic ordering, UTF-8 name validation, and identical fatal-error behavior.
- Acceptance: all four builtins are statically checked (`Str` paths); `list_dir` returns entry names without `.`/`..`, sorted by UTF-8 bytes, and fails with `R0015` on non-UTF-8 entry names; `make_dir` creates exactly one directory and rejects existing paths; `remove_file` removes files/symbolic links but rejects directories; `remove_dir` removes only empty directories; failures exit 2 with `R0015` in both engines; relative paths resolve against the process working directory; argument evaluation is left to right; statically non-`Str` arguments are `E0301`, and dynamically typed (`Any`) native paths are rejected with `E0401`; no new dependency is added.
- Dependencies: T-006, T-007, T-010, T-025, T-058, ADR-0007, ADR-0027.
- Implementation: `BUILTINS` and `E0301` checks in `src/checker.rs`; `list_dir`/`make_dir`/`remove_file`/`remove_dir` dispatch in `src/interp.rs`; `IrType` validation in `src/ir.rs`; builtin call emission, `vantyx_list_dir`, `vantyx_make_dir`, `vantyx_remove_file`, and `vantyx_remove_dir` in `src/codegen_c.rs`; ADR-0027 and specification/status updates.
- Tests: `tests/e2e.rs` (`directory_builtins_round_trip`, `directory_builtins_reject_non_str_arguments`, `directory_errors_are_fatal_r0015`, `list_dir_rejects_non_utf8_entry_names`); `tests/native.rs` (`directory_builtins_match`, `directory_errors_match`, `directory_listing_rejects_non_utf8_names_in_both_engines`). The error matrix covers missing directories, existing directories, nested creation without a parent, removing a directory as a file, removing a non-empty directory, and listing a regular file.
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 304 library unit + 15 CLI unit + 32 AI integration + 51 end-to-end + 83 native = 485 tests. The three directory native tests pass under `-fsanitize=address,undefined` with leak detection disabled (v0.1 allocations are intentionally leaked). Manual smoke: nested creation, sorted listing, existence, removal, and cleanup produced `["a", "b"]`, `["note.txt"]`, `true`, and `false` identically under `vantyx run` and `vantyx build`; missing paths, existing directories, wrong file/directory kinds, and non-empty directories exited 2 with `R0015` in both engines.
- Scope limits: single-level only; no recursive creation/removal, globbing, metadata objects or preservation, symlink mutation, current-directory changes, streaming iteration, or atomic operations (metadata and symbolic-link probes were added in T-064). Rename and byte-preserving file copy were added separately in T-061. `list_dir` reads the entire directory into memory and offers no entry-count or total-name-byte cap. Errors are fatal `R0015` until `Result` exists. Paths use host privileges with no sandbox, and all check-then-act sequences are subject to TOCTOU races. The generated native program now depends on POSIX `<dirent.h>`/`<unistd.h>`; Windows native support remains unverified.

### T-060 — UTF-8 text builtins
- Priority: P1
- Status: VERIFIED (interpreter/native differential)
- Description: Add `split`, `join`, `trim`, `starts_with`, `ends_with`, and `replace` to the interpreter and native backend with explicit UTF-8 semantics, identical output, and left-to-right argument evaluation.
- Acceptance: `split` is literal and lossless for a non-empty separator, preserves empty fields, and splits into Unicode scalar values when the separator is empty; `join` requires `List[Str]` and returns `""` for an empty list; `trim` removes exactly the Unicode `White_Space` set and leaves zero-width/format characters; `starts_with`/`ends_with` are byte-exact and accept empty affixes; `replace` is left-to-right, non-overlapping, does not rescan replacements, and rejects an empty pattern with `R0016`; static non-`Str` arguments are `E0301`, dynamic interpreter type errors are `R0005`, and native dynamically typed (`Any`) arguments are rejected with `E0401`; multi-argument evaluation is left to right in both engines; no new dependency or arbitrary size cap is added.
- Dependencies: T-006, T-007, T-010, T-025, ADR-0007, ADR-0028.
- Implementation: `BUILTINS` and `E0301` checks in `src/checker.rs`; `split`/`join`/`trim`/`starts_with`/`ends_with`/`replace` dispatch and the shared `is_text_whitespace` table in `src/interp.rs`; `IrType` validation in `src/ir.rs`; builtin call emission with ordered statement expressions and `vantyx_utf8_decode`, `vantyx_text_is_whitespace`, `vantyx_text_split`, `vantyx_text_join`, `vantyx_text_trim`, `vantyx_text_starts_with`, `vantyx_text_ends_with`, and `vantyx_text_replace` in `src/codegen_c.rs`; ADR-0028 and specification/status updates.
- Tests: `tests/e2e.rs` (`text_builtins_round_trip`, `text_builtins_reject_non_str_arguments`, `text_replace_rejects_empty_pattern`); `tests/native.rs` (`text_builtins_match`, `text_replace_empty_pattern_matches`, `text_builtins_evaluate_arguments_left_to_right`). The round trip covers empty fields, trailing separators, multibyte scalar splitting, empty lists, ASCII/NBSP/ideographic whitespace, zero-width and information-separator preservation, empty affixes, non-overlapping replacement, and deletion replacement.
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 304 library unit + 15 CLI unit + 32 AI integration + 54 end-to-end + 86 native = 491 tests. The three native text tests pass under `-fsanitize=address,undefined` with leak detection disabled (v0.1 allocations are intentionally leaked). All five examples match interpreter/native output; fuzz smoke reports 9 depth/limit cases and 400 random/mutated inputs with controlled diagnostics; the link check resolves 73 relative links in 96 Markdown files. Manual smoke produced identical output for empty fields, Unicode trimming, affix tests, and non-overlapping replacement; an empty `replace` pattern exited 2 with `R0016` in both engines.
- Scope limits: no regular-expression engine, case conversion, Unicode normalization, locale handling, or grapheme-cluster segmentation. `split` with an empty separator operates on Unicode scalar values, not grapheme clusters; `trim` does not remove zero-width or format characters; `replace` cannot express an empty pattern. `split`, `join`, and `replace` allocate proportional to their input and output sizes with no streaming API or output-size cap. Native text builtins require statically known `Str` and `List[Str]`; dynamically typed text is interpreter-only.

### T-061 — Rename and file-copy builtins
- Priority: P1
- Status: VERIFIED (interpreter/native differential)
- Description: Add `rename` and `copy_file` to the interpreter and native backend with fatal `R0015` semantics, byte-preserving copies, and left-to-right argument evaluation.
- Acceptance: `rename` moves or renames one file or directory without creating parents and delegates overwrite/atomicity behavior to the host; `copy_file` requires a regular source file, creates or truncates the target, copies bytes exactly (including NUL and invalid UTF-8), and preserves no metadata; source and target use host paths with no sandbox; NUL paths, missing paths, permission failures, cross-device renames, wrong source kinds, and I/O failures exit 2 with `R0015` in both engines; static non-`Str` arguments are `E0301`, dynamic interpreter type errors are `R0005`, and native dynamically typed paths are rejected with `E0401`; multi-argument calls evaluate left to right; no dependency or arbitrary file-size cap is added.
- Dependencies: T-006, T-007, T-010, T-058, T-059, ADR-0007, ADR-0026, ADR-0027, ADR-0029.
- Implementation: `BUILTINS` and `E0301` checks in `src/checker.rs`; `std::fs::rename` and a byte copy over `std::fs::File`/`std::io::copy` in `src/interp.rs`; `IrType` validation in `src/ir.rs`; ordered statement-expression emission and `vantyx_rename`/`vantyx_copy_file` POSIX helpers in `src/codegen_c.rs`; ADR-0029 and specification/status updates.
- Tests: `tests/e2e.rs` (`rename_and_copy_round_trip`, `copy_file_preserves_bytes`, `rename_and_copy_reject_non_str_arguments`, `rename_and_copy_dynamic_arguments_fail_at_runtime`, `rename_and_copy_errors_are_fatal_r0015`); `tests/native.rs` (`rename_and_copy_match`, `copy_file_preserves_bytes_match`, `rename_and_copy_errors_match`, `rename_and_copy_evaluate_arguments_left_to_right`, `rename_and_copy_any_paths_are_rejected_with_e0401`). The round trip covers rename, target truncation, directory copy, directory rename, and cleanup; the byte test uses NUL, invalid UTF-8, and high-bit bytes.
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 304 library unit + 15 CLI unit + 32 AI integration + 59 end-to-end + 91 native = 501 tests. The five native rename/copy tests pass under `-fsanitize=address,undefined` with leak detection disabled (v0.1 allocations are intentionally leaked). E2E tests also cover static `E0301` argument errors and dynamic `R0005` type errors for `Any` paths. All five examples match interpreter/native output; fuzz smoke reports 9 depth/limit cases and 400 random/mutated inputs with controlled diagnostics; the link check resolves 74 relative links in 97 Markdown files. Manual smoke produced identical output for rename, copy, truncation, directory rename, and cleanup; binary copies were byte-identical; missing sources, directory copy sources, and rename-over-directory exited 2 with `R0015` in both engines.
- Scope limits: no recursive copy or move, globbing, metadata preservation, atomic copy, fsync, progress reporting, or recoverable `Result`-based errors. `copy_file` copies bytes only and does not validate UTF-8/NUL or preserve permissions, ownership, timestamps, extended attributes, ACLs, or hard-link identity; a failed copy can leave a partial target. Source and target must be distinct; same-path copies can truncate the source. `rename` overwrite and atomicity behavior is host-defined and Windows native support remains unverified.

### T-062 — Lexical path builtins
- Priority: P1
- Status: VERIFIED (interpreter/native differential)
- Description: Add `path_join`, `path_parent`, `path_name`, `path_stem`, `path_extension`, and `path_is_absolute` to the interpreter and native backend as pure POSIX-style lexical helpers with no filesystem access.
- Acceptance: paths use `/` as the only separator; `path_join` discards an empty base, preserves an empty child, discards the base for an absolute child, and appends exactly one separator when the base does not already end with `/`; `path_parent` trims trailing separators and returns `.` or `/` when no parent component remains; `path_name` returns the final component or `""`; the last dot separates stem and extension only when it is neither the first byte nor the last byte; `path_is_absolute` is true exactly when the first byte is `/`; no function normalizes `.`/`..`, collapses repeated separators, accesses the filesystem, or adds a sandbox; UTF-8 names are preserved; static non-`Str` arguments are `E0301`, dynamic interpreter type errors are `R0005`, and native dynamically typed arguments are rejected with `E0401`; `path_join` evaluates its arguments left to right; no new dependency or error code is added.
- Dependencies: T-006, T-007, T-010, T-025, ADR-0007, ADR-0030.
- Implementation: `BUILTINS` and `E0301` checks in `src/checker.rs`; byte-scanned helpers and builtin dispatch in `src/interp.rs`; `IrType` validation in `src/ir.rs`; ordered statement-expression emission and C helpers (`vantyx_path_join`, `vantyx_path_parent`, `vantyx_path_name`, `vantyx_path_stem`, `vantyx_path_extension`, `vantyx_path_is_absolute`) in `src/codegen_c.rs`; ADR-0030 and specification/status updates.
- Tests: `tests/e2e.rs` (`path_builtins_round_trip`, `path_builtins_reject_non_str_arguments`, `path_builtins_dynamic_arguments_fail_at_runtime`); `tests/native.rs` (`path_builtins_match`, `path_builtins_evaluate_arguments_left_to_right`, `path_builtins_any_paths_are_rejected_with_e0401`). The round trip covers empty paths, absolute children, repeated/trailing separators, root and dot parents, empty names, dotfiles, trailing dots, multiple dots, missing extensions, Unicode names, and relative/absolute probes; the `E0401` test exercises all six builtins through `Any` parameters.
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 304 library unit + 15 CLI unit + 32 AI integration + 62 end-to-end + 94 native = 507 tests. The three native path tests pass under `-fsanitize=address,undefined` with leak detection disabled (v0.1 allocations are intentionally leaked). All five examples match interpreter/native output; fuzz smoke reports 9 depth/limit cases and 400 random/mutated inputs with controlled diagnostics; the link check resolves 75 relative links in 100 Markdown files. Manual smoke covered repeated/trailing separators, root parents, dotfile/trailing-dot stems, Unicode names, and `Any` rejection with identical interpreter/native behavior.
- Scope limits: lexical only; no `Path` type, normalization, component collapsing, `..` confinement, globbing, filesystem resolution, symlink metadata, Windows drive/UNC support, or current-directory changes. `path_join` takes exactly two arguments and can produce a path outside the base when the child is absolute or contains `..`; it is not a sandbox. Extension splitting is purely textual and does not understand compound extensions or file types. The helpers have no locale or normalization behavior, and Windows native support remains unverified.

### T-063 — Recursive directory creation and removal
- Priority: P1
- Status: VERIFIED (interpreter/native differential)
- Description: Add `make_dir_all` and `remove_dir_all` to the interpreter and native backend with matching recursive semantics, fatal `R0015` failures, and no new dependency.
- Acceptance: `make_dir_all` creates every missing parent, succeeds for an existing directory including a symlink to a directory, treats an empty path as a no-op success, and fails with `R0015` when a non-directory component blocks creation; `remove_dir_all` removes a directory and all descendants, unlinks symbolic links instead of traversing them (including a symbolic link passed as the root, with trailing separators trimmed before the root check), fails with `R0015` for a regular file, a missing path, or an empty path, and may leave a partial tree after a failure; both builtins reject NUL bytes, resolve relative paths against the process working directory, are statically checked as `Str`, map dynamic interpreter type errors to `R0005`, and are rejected with `E0401` when a native path is dynamically typed; no confirmation prompt, rollback, depth limit, entry-count limit, metadata preservation, atomicity, or sandbox is added.
- Dependencies: T-006, T-007, T-010, T-025, T-059, ADR-0007, ADR-0031.
- Implementation: `BUILTINS` and `E0301` checks in `src/checker.rs`; `make_dir_all`/`remove_dir_all` dispatch with trailing-separator root normalization in `src/interp.rs`; `IrType` validation in `src/ir.rs`; `vantyx_make_dir_all` and `vantyx_remove_dir_all` C helpers in `src/codegen_c.rs`; ADR-0031 and specification/status updates.
- Tests: `tests/e2e.rs` (`recursive_directory_builtins_round_trip`, `recursive_directory_builtins_reject_non_str_arguments`, `recursive_directory_errors_are_fatal_r0015`, `recursive_directory_dynamic_arguments_fail_at_runtime`, and Unix-only `recursive_directory_removal_does_not_follow_symlinks`); `tests/native.rs` (`recursive_directory_builtins_match`, `recursive_directory_errors_match`, `recursive_directory_any_paths_are_rejected_with_e0401`, and Unix-only `recursive_directory_removal_does_not_follow_symlinks`). Coverage includes nested creation, existing directories, empty paths, sorted listings, regular files in the way, missing and empty removal targets, `R0015` on both engines, `R0005` for dynamic interpreter paths, `E0401` for native `Any` paths, inner and root symlinks, trailing-separator root symlinks, and target preservation. T-069 replaced the original destructive `remove_dir_all(".")` test with `recursive_removal_refuses_destructive_roots`/`recursive_removal_allows_a_leading_dot_prefix` in e2e and `recursive_removal_refuses_destructive_roots_in_both_engines`/`recursive_removal_allows_a_leading_dot_prefix_in_both_engines` in native.
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 304 library unit + 15 CLI unit + 32 AI integration + 67 end-to-end + 99 native = 517 tests. The five recursive-directory native tests pass under `-fsanitize=address,undefined` with leak detection disabled (v0.1 allocations are intentionally leaked). All five examples match interpreter/native output; fuzz smoke reports 9 depth/limit cases and 400 random/mutated inputs with controlled diagnostics. Manual symlink checks confirmed that inner links are unlinked, a root `link/` is unlinked without traversing its target, and `make_dir_all` accepts a symlink to a directory.
- Scope limits: Fatal `R0015` only until `Result` exists. Recursive removal has no trash, dry run, confirmation, rollback, or atomic staging; a failure can leave a partially removed tree, and `remove_dir_all(".")` originally emptied the visible working directory before failing to remove `.` itself. T-069 later replaced that destructive behavior and the `link/.`/`link/..` traversal gap with lexical root refusals (ADR-0035). Both operations are subject to check-then-act races, host permissions, and native C stack depth; neither is a sandbox. Intermediate components such as `link/sub` can still reach through a symbolic link, so untrusted paths require caller validation. There is no globbing, metadata preservation, traversal callback, sorted recursive walk, entry-count limit, or Windows native evidence; read-only metadata probes were added in T-064.

### T-064 — File metadata probes
- Priority: P1
- Status: VERIFIED (interpreter/native differential)
- Description: Add `file_size`, `is_file`, `is_dir`, and `is_symlink` to the interpreter and native backend with matching follow-symlink rules, no new dependency, and no metadata mutation.
- Acceptance: `file_size(path)` follows symbolic links, requires an existing regular file, and returns the host-reported size in bytes; a missing path, a directory or other non-regular target, a permission failure, a NUL byte, or a size not representable as `Int` is `R0015`. `is_file` and `is_dir` follow symbolic links and return `false` when metadata cannot be read; `is_symlink` uses `lstat` and returns `true` only when the path itself is a symbolic link, including a dangling link. Missing paths, broken links, and permission failures produce `false` for predicates rather than errors; NUL bytes are `R0015` in the interpreter, while native strings cannot contain NUL and native NUL-containing source is already rejected with `E0401` by the IR string invariant. All four builtins are statically checked as `Str`, map dynamic interpreter type errors to `R0005`, and reject dynamically typed native paths with `E0401`. No mode, owner, timestamp, ACL, birthtime, inode, device, or recursive-size query is added, and no metadata is modified or preserved.
- Dependencies: T-006, T-007, T-010, T-025, T-058, T-059, ADR-0007, ADR-0032.
- Implementation: `BUILTINS` and `E0301` checks in `src/checker.rs`; `std::fs::metadata`/`symlink_metadata` dispatch in `src/interp.rs`; `IrType` validation in `src/ir.rs`; `vantyx_file_size`, `vantyx_is_file`, `vantyx_is_dir`, and `vantyx_is_symlink` C helpers in `src/codegen_c.rs`; ADR-0032 and specification/status updates.
- Tests: `tests/e2e.rs` (`file_metadata_builtins_round_trip`, `file_metadata_builtins_reject_non_str_arguments`, `file_metadata_errors_are_fatal_r0015`, `file_metadata_dynamic_arguments_fail_at_runtime`, and Unix-only `file_metadata_symlink_rules`); `tests/native.rs` (`file_metadata_builtins_match`, `file_metadata_errors_match`, `file_metadata_any_paths_are_rejected_with_e0401`, and Unix-only `file_metadata_symlink_rules_match`). Coverage includes logical size for a written file, regular-file and directory predicates, missing paths, a directory-valued `file_size` target, empty paths, dynamic interpreter `R0005`, native `E0401`, and Unix file/directory/dangling symlink rules.
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 304 library unit + 15 CLI unit + 32 AI integration + 72 end-to-end + 103 native = 526 tests. The four metadata native tests pass under `-fsanitize=address,undefined` with leak detection disabled (v0.1 allocations are intentionally leaked). All five examples match interpreter/native output; fuzz smoke reports 9 depth/limit cases and 400 random/mutated inputs with controlled diagnostics.
- Scope limits: Follows host metadata semantics; `file_size` reports `st_size` (logical size), not allocated blocks or recursive directory size, and follows symlinks. Predicates do not distinguish permission errors from missing paths. The interpreter rejects a NUL path with `R0015`; native NUL strings are already rejected with `E0401`. Check-then-act races apply, no sandbox, and Windows native support remains unverified.

### T-065 — Lexical glob expansion
- Priority: P1
- Status: VERIFIED (interpreter/native differential)
- Description: Add `glob(pattern: Str) -> List[Str]` to the interpreter and native backend as a zero-dependency lexical pattern expander with explicit hidden-entry, symbolic-link, ordering, and resource-limit rules.
- Acceptance: patterns split on `/`, discard empty components, and are absolute exactly when they begin with `/`; `*` matches zero or more Unicode scalar values, `?` matches one, `[abc]`/`[a-z]`/`[!a-z]` classes match scalar values, and a whole-component `**` recurses without following symbolic links; wildcards and `**` skip names beginning with `.` unless a component explicitly allows hidden names; results are lexical spellings sorted by UTF-8 bytes and deduplicated; a final literal is probed with `lstat` so dangling links are returned; missing paths and non-directories produce no matches; malformed patterns (empty, unclosed/empty class, descending range) and more than 1,024 components are `R0016`; listing or `**` `lstat` failures, non-UTF-8 entry names, more than 100,000 candidate paths before deduplication, and `**` depth beyond 128 are `R0015`; relative patterns resolve against the process working directory; static non-`Str` arguments are `E0301`, dynamic interpreter values are `R0005`, and native dynamically typed patterns are `E0401`; no dependency, sandbox, filesystem mutation, escaping, brace expansion, case folding, normalization, or silent non-UTF-8 skipping is added.
- Dependencies: T-006, T-007, T-010, T-025, T-059, ADR-0007, ADR-0033.
- Implementation: `src/glob.rs` (parser, component matcher, walk, caps, unit tests); `BUILTINS` and `E0301` checks in `src/checker.rs`; dispatch in `src/interp.rs`; `IrType` validation in `src/ir.rs`; `vantyx_glob` and POSIX runtime helpers in `src/codegen_c.rs`; module registration in `src/lib.rs`; ADR-0033 and specification/status updates.
- Tests: `src/glob.rs` unit tests (`components_match_literals_wildcards_and_classes`, `components_apply_hidden_rules`, `malformed_patterns_are_invalid`, `literal_probes_do_not_read_directories`, `glob_expands_a_tree_with_sorted_unique_results`, `limits_are_enforced`, `depth_limit_is_enforced`, and Unix-only `recursive_wildcards_do_not_follow_symlinks`); `tests/e2e.rs` (`glob_round_trip`, `glob_rejects_malformed_patterns_with_r0016`, `glob_rejects_non_str_arguments`, `glob_dynamic_arguments_fail_at_runtime`, `glob_depth_limit_is_fatal_r0015`, Unix-only `glob_rejects_non_utf8_entry_names`, `glob_rejects_nul_patterns_with_r0015`, `glob_absolute_and_repeated_separators_are_lexical`, and `glob_component_cap_is_r0016`); `tests/native.rs` (Unix-only `glob_matches_interpreter_in_both_engines`, Unix-only `glob_malformed_patterns_fail_in_both_engines`, `glob_depth_limit_matches_in_both_engines`, Unix-only `glob_rejects_non_utf8_names_in_both_engines`, Unix-only `glob_result_cap_matches_in_both_engines`, `glob_absolute_patterns_match_in_both_engines`, and `glob_component_cap_matches_in_both_engines`). Coverage includes exact sorted output for wildcard/recursive/hidden/literal/missing/dangling/base-link cases, repeated separators, absolute patterns, malformed patterns, NUL, non-UTF-8 names, all three caps, and interpreter/native agreement.
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 312 library unit + 15 CLI unit + 32 AI integration + 81 end-to-end + 110 native = 550 tests. The eight native glob tests pass under `-fsanitize=address,undefined` with leak detection disabled (v0.1 allocations are intentionally leaked). All five examples match interpreter/native output in debug and release; fuzz smoke reports 9 depth/limit cases and 400 random/mutated inputs with controlled diagnostics.
- Scope limits: Fatal `R0015`/`R0016` only until `Result` exists. The grammar is smaller than shell globs and common libraries: no escaping, braces, extglobs, tilde, case-insensitive mode, sorting options, or `Path` type. `**` avoids symlink cycles and hidden trees by construction, but explicit link components and `..` can leave the apparent base; there is no sandbox or containment check, and check-then-act races apply. Caps bound memory but not CPU: multiple `**` components can revisit overlapping subtrees, each directory is listed fully into memory, and the result list is materialized. Non-UTF-8 entry names fail the whole expansion rather than being skipped. Native expansion uses POSIX `DIR`/`readdir`/`lstat`; Windows native support remains unverified.

### T-066 — Recursive directory copy
- Priority: P1
- Status: VERIFIED (interpreter/native differential)
- Description: Add `copy_dir_all(from: Str, to: Str) -> Unit` to the interpreter and native backend with new-target semantics, resolved-self containment, raw entry-name copying, inner-symlink and special-file rejection, and explicit entry/depth caps.
- Acceptance: the source must resolve, following symbolic links in every component including a final link, to an existing directory; the target must not exist, including as a dangling link, its final name after trailing-separator trimming must not be empty/`.`/`..`, its parent must already exist and resolve to a directory, and the resolved target must not equal or lie inside the resolved source; existing targets are rejected rather than overwritten or merged. The target is created once, regular files are copied byte for byte, directories are copied recursively, inner symbolic links and sockets/FIFOs/devices/other special files are rejected, and entry names are copied as raw bytes so non-UTF-8 Unix names are supported. Permissions, ownership, timestamps, xattrs, ACLs, hard-link identity, and sparse layout are not preserved. A failed copy leaves a partial target; there is no rollback, atomic staging, cancellation, or sandbox. More than 100,000 entries or a directory beyond 128 levels below the source root is `R0015`. Static non-`Str` arguments are `E0301`, dynamic interpreter values are `R0005`, native dynamically typed paths are `E0401`, and a NUL path is `R0015` in the interpreter.
- Dependencies: T-006, T-007, T-010, T-025, T-058, T-059, T-061, T-063, ADR-0007, ADR-0029, ADR-0031, ADR-0034.
- Implementation: `src/copy_tree.rs` (source/target validation, canonical containment, byte-preserving walk, caps, unit tests); `BUILTINS` and two-`Str` `E0301` checks in `src/checker.rs`; dispatch with NUL handling in `src/interp.rs`; `IrType` validation in `src/ir.rs`; `vantyx_copy_dir_all`, `vantyx_copy_tree`, and POSIX `stat`/`realpath`/`opendir`/`readdir`/`lstat`/`mkdir`/stream helpers in `src/codegen_c.rs`; module registration in `src/lib.rs`; ADR-0034 and specification/status updates.
- Tests: `src/copy_tree.rs` unit tests (`copies_a_tree_and_preserves_the_source`, `copies_bytes_only_without_preserving_permissions`, `rejects_invalid_sources_and_targets`, `default_limits_are_pinned`, Unix-only `rejects_special_files`, Unix-only `rejects_inner_links_but_follows_a_linked_root`, and `limits_are_enforced`); `tests/e2e.rs` (`recursive_copy_round_trip`, `recursive_copy_rejects_non_str_arguments`, `recursive_copy_dynamic_arguments_fail_at_runtime`, `recursive_copy_errors_are_fatal_r0015`, `recursive_copy_depth_limit_is_fatal_r0015`, Unix-only `recursive_copy_symlink_rules`, and Unix-only `recursive_copy_preserves_non_utf8_names`); `tests/native.rs` (`recursive_copy_matches_in_both_engines`, `recursive_copy_errors_match_in_both_engines`, `recursive_copy_any_paths_are_rejected_with_e0401`, `recursive_copy_depth_limit_matches_in_both_engines`, Unix-only `recursive_copy_symlink_rules_match_in_both_engines`, Unix-only `recursive_copy_special_files_match_in_both_engines`, and Unix-only `recursive_copy_non_utf8_names_match_in_both_engines`). Coverage includes round-trip trees with hidden/empty directories and binary files, source preservation, existing/missing-parent/file/missing sources, empty/`.`/`..`/NUL target names, resolved self-containment, depth cap, entry cap (manual 100,001-entry probe), root-link following, inner/dangling link rejection, Unix socket rejection, non-UTF-8 names, static `E0301`, dynamic `R0005`, and native `E0401`.
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 319 library unit + 15 CLI unit + 32 AI integration + 88 end-to-end + 117 native = 571 tests. The seven native recursive-copy tests pass under `-fsanitize=address,undefined` with leak detection disabled (v0.1 allocations are intentionally leaked). A manual 100,001-entry tree exits 2 with `R0015` in both engines after copying exactly 100,000 entries; a 130-level source fails identically at the depth cap. All five examples match interpreter/native output in debug and release; fuzz smoke reports 9 depth/limit cases and 400 random/mutated inputs with controlled diagnostics.
- Scope limits: Fatal `R0015` only until `Result` exists. There is no overwrite or merge mode, rollback, atomic staging, cancellation, progress reporting, recursive move, metadata preservation, byte cap, or sandbox. The containment check only prevents copying a tree into itself; source and target parents can change between resolution, target creation, and traversal (TOCTOU), and the target can be redirected by concurrent renames or link swaps. Raw entry-name copying is Unix-oriented; native Windows support remains unverified. The entry cap bounds entry count but not total bytes, and the depth cap applies only to this walker. Native traversal uses the C stack and intentionally leaked allocations.

### T-067 — Live cloud OpenAI-compatible verification
- Priority: P1
- Status: VERIFIED (cloud live; re-verified at 20:59Z; provider access is not guaranteed)
- Description: Exercise the real cloud OpenAI-compatible path end to end through `vantyx ai` and record reproducible, redacted evidence for chat, streaming, validated extraction, embeddings, RAG indexing, semantic search, and retrieval evaluation.
- Acceptance: a real provider completes model discovery, non-streamed chat, streamed chat, schema-validated extraction with bounded repair, 768-dimension embeddings, semantic indexing, search, and evaluation; observed outputs and usage are recorded; the 401/429/timeout paths are covered by a reproducible harness; no key value is written to the repository; limitations are explicit.
- Dependencies: T-038, T-039, T-043, T-045, T-046, T-052.
- Implementation: `tools/live_ai_check.py` (opt-in live harness with redacted evidence and bounded retries); `tools/ai_failure_harness.py` (loopback 401/429/timeout harness); `docs/ai/LIVE_VERIFICATION.md`; `docs/ai/live-evidence-2026-11-24.json`; `docs/ai/live-evidence-2026-11-24-retry.json`; `docs/ai/live-evidence-2026-11-24-retry2.json`.
- Tests: manual live runs against `https://generativelanguage.googleapis.com/v1beta/openai` with `gemini-3.1-flash-lite` and `gemini-embedding-001`; the live harness validates output, usage, dimensions, top hit, and recall; the failure harness asserts 16 checks across 7 scenarios.
- Verification: 2026-11-24 — the first full run reported `doctor` `status: ok` with 65 models; chat returned `VANTYX LIVE` with usage 22/4/26; streaming returned the counted words; extraction returned a validated Ada Lovelace record with one repair (usage 237/105/342); embeddings returned two 768-component vectors (usage 28); a 3-document semantic index stored 3 vectors (usage 206); semantic search ranked `vantyx.md` first at 0.606483; evaluation reported recall@2 = 1.0, precision@2 = 0.5, and MRR = 1.0. The failure harness passed 16 checks across 7 scenarios, including `AI_E_AUTH`, `AI_E_RATE_LIMIT`, and `AI_E_TIMEOUT`. A later retry at 19:50Z returned HTTP 400 `User location is not supported` for both available credentials; no bypass was attempted. A re-verification at 20:59Z using `MY_API_KEY` passed all 21 live checks (`doctor`, chat, streaming, extraction, embeddings, RAG index/search, and evaluation; chat usage 13/4/17, extraction attempts 2 with usage 237/105/342, embeddings usage 28, index usage 206, search top `vantyx.md`, recall@2 = 1.0, precision@2 = 0.5, MRR = 1.0). The first re-run at 20:58Z failed only extraction with a transient HTTP 503 high-demand response; the next full run passed. Evidence: `docs/ai/live-evidence-2026-11-24.json`, `docs/ai/live-evidence-2026-11-24-retry.json`, and `docs/ai/live-evidence-2026-11-24-retry2.json`. The current 581-test gate and clippy are clean; CI run 45488725812 passed on `4982ab3` with 581 tests.
- Scope limits: No local Ollama/llama.cpp model was available, so T-040 remains blocked for the local-model half. Live cloud access is not guaranteed: an earlier retry was rejected by provider geography, a later retry hit a transient HTTP 503, and a subsequent run passed all 21 checks; the successful observations above are not a claim of continuous availability. The CLI reports retryable errors but does not run an automatic retry loop; the harness retries with recorded exponential backoff. Streaming tool calls, interactive cancellation, server-side constrained decoding, grounded generation, and throughput benchmarks remain open. First-byte latency was observed at 1,976 ms and 14,958 ms in two runs and is not a performance claim. Model names and availability changed during the session. No key value is stored.

### T-068 — Curl timeout error mapping
- Priority: P1
- Status: VERIFIED
- Description: Map the system-curl exit code 28 (operation timeout) to retryable `AI_E_TIMEOUT` on both the buffered and streaming HTTPS paths instead of the generic `AI_E_CONNECT`.
- Acceptance: curl exit 28 yields `AI_E_TIMEOUT` with `retryable: true`; other curl failures such as exit 7 remain `AI_E_CONNECT` and non-retryable; both buffered and streaming paths use the same mapping; unit tests cover both codes; no behavior change to loopback HTTP.
- Dependencies: T-039, T-052.
- Implementation: `curl_failure` helper in `src/ai/http.rs`, used by `send_curl` and `stream_curl`.
- Tests: `src/ai/http.rs` unit tests `curl_timeout_maps_to_a_retryable_timeout` and `curl_connect_failure_stays_a_connect_error`.
- Verification: 2026-11-24 — `cargo test curl_` passed 2 tests; `cargo clippy --all-targets -- -D warnings` clean; full `cargo test` passed 321 library unit + 15 CLI unit + 32 AI integration + 88 end-to-end + 117 native = 573 tests.
- Scope limits: Only curl exit 28 is reclassified. There is still no automatic retry loop or `Retry-After` handling in the CLI; DNS resolution failures and refused connections remain `AI_E_CONNECT`. The timeout value is still rounded to whole seconds for curl.

### T-069 — Refuse destructive recursive-removal roots
- Priority: P0
- Status: VERIFIED (interpreter/native differential)
- Description: Make `remove_dir_all` refuse `.`, `..`, final `.`/`..` components (including `link/.` and `link/..`), and the filesystem root `/` before any filesystem access, in both the interpreter and the native backend, so an accidental call cannot empty the working directory or traverse a root link.
- Acceptance: `remove_dir_all` returns fatal `R0015` for `.`, `./`, `..`, `../`, every root whose final component is `.` or `..` after trailing-separator trimming, and `/`/`//`/`///`; no file, directory, or source is deleted for those spellings; ordinary roots such as `tree`, `tree/`, `./tree`, `/tmp/tree`, and `foo/.hidden` remain accepted; an empty path keeps its ordinary missing-path `R0015`; the interpreter and native backend use the same lexical rule and stable explanation; no sandbox, intermediate-link protection, normalization, confirmation, rollback, or `Result` behavior is claimed.
- Dependencies: T-006, T-007, T-010, T-058, T-059, T-063, ADR-0007, ADR-0031, ADR-0035.
- Implementation: `src/fs_safety.rs` (classifier, stable messages, five unit tests); refusal dispatch in `src/interp.rs`; `vantyx_remove_root_refusal` and the refusal branch in `vantyx_remove_dir_all` in `src/codegen_c.rs`; module registration in `src/lib.rs`; ADR-0035 and specification/security/status updates.
- Tests: `src/fs_safety.rs` unit tests (`allows_ordinary_roots`, `refuses_dot_and_dot_dot_final_components`, `refuses_the_filesystem_root`, `empty_paths_are_left_to_the_filesystem_layer`, and `messages_are_stable`); `tests/e2e.rs` (`recursive_removal_refuses_destructive_roots` covering eight spellings with sentinel/tree/source preservation and `recursive_removal_allows_a_leading_dot_prefix`); `tests/native.rs` (`recursive_removal_refuses_destructive_roots_in_both_engines` covering seven spellings, preserved files, and the executable, and `recursive_removal_allows_a_leading_dot_prefix_in_both_engines`). The previous differential test that locked the destructive `remove_dir_all(".")` behavior was replaced.
- Verification: 2026-11-24 — `cargo test fs_safety` passed 5; `cargo test --test e2e recursive_removal` passed 2; `cargo test --test native recursive_removal` passed 2; `cargo clippy --all-targets -- -D warnings` clean; full `cargo test` passed 326 library unit + 15 CLI unit + 32 AI integration + 90 end-to-end + 118 native = 581 tests. CI run 45488725812 passed on `4982ab3` with 581 tests; the preceding run 45488152955 failed only the glob result-cap test because ext4 caps hard links per inode, and the test now spreads links across four anchors.
- Scope limits: The refusal is lexical and does not resolve symbolic links or normalize intermediate `.`/`..` components; `link/sub` can still reach through a link, and check-then-act races remain. There is no confirmation, trash, dry run, rollback, depth limit, entry-count limit, atomic staging, `Result` recovery, or sandbox. Windows native support remains unverified. The interpreter and native helper must be kept in sync by review.

### T-070 — Parenthesize statement-expression conditions in the native backend
- Priority: P0
- Status: VERIFIED (interpreter/native differential)
- Description: Fix native `if`/`elif`/`while` code generation so a condition that lowers to a GNU C statement expression (a call, a short-circuit expression, and so on) is emitted as `if (({ ... }))` instead of the invalid `if ({ ... })`.
- Acceptance: `vantyx build` accepts programs whose `if`, `elif`, and `while` conditions call functions or contain effectful expressions; interpreter and native stdout and exit codes match; conditions that are already parenthesized comparisons keep their existing form so clang does not emit `-Wparentheses-equality`; match guards remain valid; no `E0401` boundary change.
- Dependencies: T-010, T-024, T-030, T-033, ADR-0008, ADR-0036.
- Implementation: `Emitter::condition` in `src/codegen_c.rs` adds one pair of parentheses when the rendered condition starts with the statement-expression prefix `({`; `if`, `elif`, and `while` keep using the helper.
- Tests: `tests/native.rs::conditions_with_calls_match` covers calls in `if`, `elif`, and `while`, a compound condition, single evaluation of the condition, and a call in a comparison; the existing native suite is unchanged.
- Verification: 2026-11-24 — `cargo test --test native conditions_with_calls_match` passed 1; full `cargo test` passed 326 library unit + 15 CLI unit + 32 AI integration + 90 end-to-end + 121 native = 584 tests; `cargo build --release` and `cargo clippy --all-targets -- -D warnings` succeeded; all five examples matched interpreter/native output; `tools/differential_fuzz.py` passed 100/100 on seed 1.
- Scope limits: The extra pair is added only for statement expressions; the backend still depends on GNU C11 extensions and `-std=gnu11`. The fix changes code generation, not the supported type set or the `E0401` boundary.

### T-071 — Preserve short-circuit evaluation for `and`/`or` in the native backend
- Priority: P0
- Status: VERIFIED (interpreter/native differential)
- Description: Stop hoisting both operands of `and`/`or` into temporaries before applying the operator. The native backend evaluated the right operand even when the left operand decided the result, silently changing observable side effects relative to the interpreter.
- Acceptance: `false and f()` and `true or f()` do not call `f`; `true and f()` and `false or f()` call it once; nested short-circuit expressions, `if` conditions, and match guards match interpreter call counts; effectful arithmetic and comparison operands still evaluate left-to-right; no `E0401` boundary change.
- Dependencies: T-010, T-033, ADR-0036.
- Implementation: `Emitter::binary_expr` in `src/codegen_c.rs` special-cases `BinOp::And` and `BinOp::Or`. When the right operand has effects, it emits a `bool` statement expression that evaluates the left operand once and selects the right operand with a conditional expression; otherwise it emits the ordinary `&&`/`||` form.
- Tests: `tests/native.rs::short_circuit_operands_match` covers `false and f()`, `true or f()`, `true and f()`, `false or f()`, a nested short-circuit expression, an `if` condition, and a match guard, checking the call count after each step; `tests/native.rs::effectful_binary_operands_keep_left_to_right_order` guards the non-short-circuit hoisting path.
- Verification: 2026-11-24 — `cargo test --test native short_circuit_operands_match` and `cargo test --test native effectful_binary_operands_keep_left_to_right_order` each passed 1; full `cargo test` passed 326 library unit + 15 CLI unit + 32 AI integration + 90 end-to-end + 121 native = 584 tests; `cargo build --release` and `cargo clippy --all-targets -- -D warnings` succeeded; all five examples matched; `tools/differential_fuzz.py` passed 100/100 on seed 1 and 200/200 on seed 500.
- Scope limits: Effect analysis remains conservative: every `Binary` and `Unary` node is treated as effectful, so some pure expressions are hoisted unnecessarily. That is a performance detail, not a semantic claim. Only side-effect preservation is verified; no new optimizer or evaluation-order guarantee is added.

### T-072 — Deterministic differential fuzzer
- Priority: P2
- Status: VERIFIED (tooling)
- Description: Add a standard-library-only deterministic generator that produces well-typed Vantyx programs from fixed grammars, runs each through the interpreter and the native backend, compares exit code and stdout, and saves failing programs; wire it into CI.
- Acceptance: A fixed seed reproduces the same programs; a mismatch prints the program and exits 1; programs rejected by both engines are counted separately; the tool skips with a clear message when no C compiler is available; CI runs a deterministic seed.
- Dependencies: T-008, T-010, ADR-0036.
- Implementation: `tools/differential_fuzz.py` with eight program shapes: scalar expressions and control flow; collections/structs/enums; guarded/range/or-pattern matching; nested collections with aliasing and mutation; typed functions with early returns, recursion, and calls in conditions; string escapes, interpolation, indexing, and UTF-8 builtins; numeric formatting, checked arithmetic, and conversions; and runtime-error agreement. `.github/workflows/ci.yml` runs 80 iterations on seed 1 after `cargo test`.
- Tests: the tool itself found T-070 and T-071 before the fixes. The expansion also found a test-quality bug: the collection generator used `xs[-1]`, which is an invalid negative index, so that program stopped at the first index and never exercised the remaining 20+ statements; it now uses `xs[len(xs) - 1]`. The string and expression generators also avoided empty `replace` patterns, which are runtime errors, and the expression generator bounds `**` bases so programs do not overflow early. After the fixes, 160/160 cases agreed on each of seeds 1, 500, and 1000, with 0 programs rejected by both engines.
- Verification: 2026-11-24 — `python3 tools/differential_fuzz.py --iterations 160 --seed 1` reported 160/160; `--iterations 160 --seed 500` reported 160/160; `--iterations 160 --seed 1000` reported 160/160; the earlier runs of 100/100 on seed 1 and 200/200 on seed 500 remain valid; `python3 -m py_compile tools/differential_fuzz.py` succeeded.
- Scope limits: Eight program shapes are not a proof of correctness. There is no property-based shrinking, no sanitizer integration, and no coverage measurement. The error-path generator checks exit code and the stdout prefix before the failure, not diagnostic text. Native cases skip when no C compiler is installed.

### T-073 — Static linter and `vantyx lint`
- Priority: P2
- Status: VERIFIED (tooling)
- Description: Add a dependency-free, read-only AST lint pass with `L0001` unused binding, `L0002` mutable binding never reassigned, and `L0003` unreachable statement after a direct `return`/`break`/`continue`; expose it as `vantyx lint <file>` without changing `run`, `check`, or `build`.
- Acceptance: each rule reports the expected warning with source span and help; clean programs are silent; names starting with `_`, parameters, loop variables, and match bindings are ignored; a plain assignment is a write and a compound assignment reads and writes; imported modules are linted; lint findings exit `0`, compile errors exit `1`, and standard input works; no lint warning is emitted by `run`, `check`, or `build`.
- Dependencies: T-003, T-005, T-007, T-019, ADR-0037.
- Implementation: `src/lint.rs` (scope stack, binding read/write tracking, conservative block termination, stable diagnostic codes); `src/lib.rs` module registration; `src/main.rs` usage line, `lint` dispatch, and `lint_file`; `docs/linting.md`; specification section 14; ADR-0037.
- Tests: 19 `src/lint.rs` unit tests (unused local/global, plain versus compound assignment, `var` never reassigned, shadowing, initializer scope, unreachable after `return`/`break`, match arm blocks, underscore names, parameters, loop variables, clean programs); `tests/e2e.rs` (`lint_reports_unused_and_unreachable_warnings`, `lint_is_silent_on_a_clean_program`, `lint_reports_compile_errors_and_exits_one`, `lint_reads_source_from_standard_input`).
- Verification: 2026-11-24 — `cargo test --lib lint::tests` passed 19; `cargo test --test e2e lint_` passed 4; full `cargo test` passed 348 library unit + 15 CLI unit + 32 AI integration + 96 end-to-end + 122 native = 613 tests; `cargo clippy --all-targets -- -D warnings` clean; debug and release builds succeeded; `python3 tools/differential_fuzz.py --iterations 100 --seed 1` agreed 100/100; all five examples matched native output; `tools/fuzz_smoke.py` produced 9 depth/limit and 400 random/mutated controlled diagnostics; CI run 45496041422 green on `5e1092f`.
- Scope limits: No suppression attributes, configuration files, severity levels, machine-readable output, or automatic fixes. `L0003` does not analyze all-terminating `if`/`match` or loops, and there is no data-flow or interprocedural analysis. The formatter remains unimplemented.

### T-074 — Reachability-aware return checking
- Priority: P1
- Status: VERIFIED (checker correctness)
- Description: Fix `block_returns` so dead code after a terminating statement no longer makes a value-returning function fail with `E0309`. The old analysis inspected only the last statement of a block, so `return 1` followed by an unreachable statement looked like a missing return.
- Acceptance: a direct `return` followed by dead code is accepted; an `if`/`else` or `match` whose every branch returns followed by dead code is accepted; a `break`/`continue` stops the reachable walk and a later `return` does not count; interpreter and native output still agree; `vantyx lint` reports the dead code as `L0003`.
- Dependencies: T-005, T-073, ADR-0037.
- Implementation: `src/checker.rs::block_returns` walks statements in order, returns `true` at a reachable `return`, returns `false` at a reachable `break`/`continue`, and recognizes a fully returning `if` with `else` or `match`.
- Tests: `src/checker.rs` (`dead_code_after_return_still_counts_as_returning`, `dead_code_after_a_terminating_if_still_counts_as_returning`, `break_before_a_return_does_not_count_as_returning`); `tests/e2e.rs` (`check_accepts_dead_code_after_return`, `run_does_not_execute_dead_code_after_return`); `tests/native.rs::dead_code_after_return_matches`.
- Verification: 2026-11-24 — `cargo test --lib checker::tests::dead_code` passed 2; `cargo test --lib checker::tests::break_before` passed 1; `cargo test --test e2e dead_code` passed 2; `cargo test --test native dead_code_after_return_matches` passed 1; full `cargo test` passed 613 tests; `cargo clippy --all-targets -- -D warnings` clean; all five examples matched native output.
- Scope limits: The analysis is not a full control-flow graph. It does not prove termination of loops, does not treat `while true` as returning, and does not reason about conditions. An `if` without `else` never terminates a block for this check. Unreachable code after an all-terminating construct is still lowered and never executed, but only direct trailing statements are linted.
