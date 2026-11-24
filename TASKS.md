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
- Status: BACKLOG

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
- Scope limits: heuristic token estimates, in-memory-only vector index, no streaming CLI, no persistent agent state, no write/shell/network tools, and no live provider evidence. Embedding providers were not part of T-037 and were added later by T-045.

### T-038 — `vantyx ai` CLI
- Priority: P1
- Status: VERIFIED
- Description: Expose the AI host foundation through `vantyx ai providers`, `doctor`, `models`, `chat`, and `tokens`, with offline defaults, environment configuration, JSON output, opt-in metrics, and explicit remote-network permission.
- Acceptance: mock and echo chat work offline; `tokens` labels estimates; `--json` emits parseable output; `--metrics` writes redacted JSONL; remote HTTPS requires `--allow-network`; unknown providers, options, and invalid numbers fail with usage errors; loopback OpenAI-compatible chat works without credentials.
- Dependencies: T-037.
- Implementation: `src/ai_cli.rs`; dispatch and usage in `src/main.rs`.
- Tests: 3 CLI unit tests; `tests/ai.rs` covers providers, mock/echo chat, stdin prompts, JSON output, token labelling, network denial, doctor, unknown providers, invalid temperature, metrics privacy, loopback adapter and CLI chat, and conservative capabilities.
- Verification: 2026-11-23 — `cargo test` 340 passed; `./target/debug/vantyx ai providers`, `chat --provider mock`, `chat --provider echo`, `chat --json`, `tokens`, and `doctor --provider mock` were run manually; remote HTTPS without `--allow-network` exited 1 with `AI_E_NETWORK_DISABLED` and help.
- Scope limits: no streaming, no model download or removal, no keyring, and no live local/cloud request was possible in this environment.

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
- Status: BLOCKED
- Description: Run one real local model (Ollama or llama.cpp) and one real cloud OpenAI-compatible provider through `vantyx ai chat`, recording the exact commands and redacted evidence.
- Acceptance: both requests succeed; failure paths 401, 429, and timeout are observed or covered by a recorded harness; no key is written to the repository; results and limitations are recorded in `PROJECT_STATUS.md`.
- Dependencies: T-039.
- Blocked by: no local inference server/model and no cloud API credentials in this environment.
- Verification: Pending.

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
- Scope limits: `HashEmbeddingProvider` is a deterministic lexical feature-hash baseline, not a trained semantic model; the cache is in-memory, process-local, FIFO, and has no TTL or persistence; `openai_compatible().embeddings` describes the adapter route, while actual server and model support varies and is reported at request time; no live local/cloud embedding model was available in this environment. RAG ingestion, chunking, citations, hybrid fusion, and retrieval evaluation are implemented separately in T-046.

### T-046 — Local RAG indexing, hybrid retrieval, citations, and evaluation
- Priority: P1
- Status: VERIFIED (offline/loopback)
- Description: Build a local-first retrieval pipeline over text, Markdown, and code: bounded recursive ingestion, stable line-cited chunks, provider embeddings, persistent strict-JSON indexes, BM25/semantic/hybrid search with reciprocal-rank fusion, and a reproducible recall/precision/MRR evaluation harness exposed through `vantyx ai index`, `search`, and `eval`.
- Acceptance: ingestion stays under one canonical root, skips symlinks, hidden paths, VCS/build directories, disallowed extensions, and enforces file-count, per-file, total-source, chunk-count, chunk-size, overlap, and embedding-input limits; chunk ids are stable and line ranges remain correct for empty, short, long, and pathological single-line documents; indexes round-trip through strict JSON, reject unknown versions, malformed vectors, mixed sparse/complete vector sets, duplicate ids, out-of-range line spans, non-finite values, and size or dimension mismatches before rebuilding lexical state; saved indexes use atomic replacement and owner-only permissions on Unix; lexical search never calls an embedding provider; semantic and hybrid search require vectors and matching dimensions; hybrid uses reciprocal-rank fusion with deterministic tie breaking; search can filter by relative source prefix before top-k; evaluation reports recall@k, precision@k, MRR, and per-query retrievals without claiming answer grounding.
- Dependencies: T-045 (embedding boundary), `src/ai/bm25.rs`, `src/ai/json.rs`.
- Implementation: `src/ai/rag.rs` (ingestion, chunking, `RagIndex`, persistence, search modes, fusion, evaluation, 14 unit tests); exports in `src/ai/mod.rs`; `vantyx ai index`, `vantyx ai search`, and `vantyx ai eval` in `src/ai_cli.rs` with `--root`, `--out`, `--index`, `--query`, `--fixture`, `--mode`, `--top-k`, `--chunk-chars`, `--overlap-chars`, `--ext`, `--source-prefix`, and `--json`; format and security decision in ADR-0018.
- Tests: 14 `rag` unit tests (line-cited stable chunking, pathological-line splitting without loss, invalid chunk limits, deterministic ingestion filtering, lexical citations, source-prefix filtering, vector requirements, semantic/hybrid ranking, persisted round trip and 0600 permissions, malformed/mixed index rejection, evaluation metrics, fixture parsing, provider capability rejection, query limits, mode parsing); 2 CLI integration tests (`rag_index_search_and_eval_are_offline_and_cited`, `lexical_rag_index_rejects_semantic_search_and_bad_limits`).
- Verification: 2026-11-24 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 276 library unit + 12 CLI unit + 27 AI integration + 43 end-to-end + 72 native = 430 tests after T-050 (413 at the time of T-046). Manual offline run indexed three Markdown files with the hash provider at 32 dimensions, returned `rust.md:1-4` as the top hybrid hit for "memory safety" with lexical and semantic ranks, and evaluated a one-query fixture at recall@2 = 1.0 and MRR = 1.0. No network or live model was used. `python3 benchmarks/rag.py --reps 5 --docs 200 --dimensions 64 --top-k 10` indexed 200 generated Markdown documents (600 chunks) at 64 hash dimensions: index median 41.639 ms, hybrid search median 26.720 ms, index size 1,247,300 bytes, 10 cited hits per query; raw results in `benchmarks/results/2026-11-24-rag.{json,md}`. This is a single-machine plumbing benchmark, not a semantic-quality or vector-database claim.
- Scope limits: text, Markdown, and code files only; no PDF, HTML, Office, OCR, audio, or video extraction; no incremental re-index, deletion API, deduplication across sources, arbitrary metadata filters (only relative source-prefix filtering), reranking, or grounded answer generation; the saved index stores full chunk text and vectors as JSON and search scans vectors linearly; the hash provider is a deterministic lexical baseline, not a trained semantic model; no live embedding-model evidence; index persistence is format version 1 with no migration; evaluation labels match exact chunk ids or source paths and do not measure hallucinations or answer quality.

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
- Status: VERIFIED (rewritten history and public contributors page; REST cache pending refresh)
- Description: Remove the unrelated `auren` account from the public repository's contributor attribution. All twelve historical commits used the placeholder identity `Auren Contributors <auren@users.noreply.github.com>`, which GitHub maps to an unrelated user with no work in the repository.
- Acceptance: every commit reachable from `main` and `v0.1.0` is authored and committed by `Zeidex <48607619+zeiad6@users.noreply.github.com>`; the GitHub contributors API no longer lists `auren`; the working tree, tests, examples, and release tag content are unchanged; `.mailmap` remains for any external references to the old hashes; documentation records that commit hashes and earlier CI run references were replaced.
- Dependencies: T-035, T-036, T-042.
- Implementation: `git filter-branch --env-filter` over `--branches --tags` to replace author and committer identity, `--tag-name-filter cat` to move `v0.1.0`, and a force-with-lease push of `main` and the tag. `.mailmap`, `PROJECT_STATUS.md`, `AGENTS.md`, `docs/PROJECT_BASELINE.md`, and this entry record the correction.
- Verification: 2026-11-24 — all reachable commits report the Zeidex identity; the public contributors page lists only `zeiad6`; GitHub's REST contributors endpoint still served a cached `auren` entry immediately after the push, so that acceptance check remains open until the cache refreshes; the full local gate passed before the rewrite (457 tests, clippy, release build, five examples, fuzz smoke, link check, streaming smoke); CI run 45365926013 passed on the rewritten `e3d8bdd` with 457 tests; a clean clone of the rewritten history passed 457 tests; the rewritten tree is byte-identical to the verified tree.
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
- Scope limits: every update still walks and chunks the complete corpus; there is no file-mtime or content-manifest shortcut, so ingestion cost is not avoided. The current `--root` is treated as the complete corpus, so partial-prefix updates and multi-root merging are not supported. There is no deduplication across sources, arbitrary metadata filtering, reranking, or grounded generation. The hash provider is a deterministic lexical baseline, not a trained semantic model, and no live embedding-model evidence exists. The index format remains version 1; an empty vector index is now valid.
