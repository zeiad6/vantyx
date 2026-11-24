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
- Tests: `tests/native.rs` — 68 differential tests covering arithmetic, control flow, recursion, structs, enums, scalar literal patterns, match guards, globals, conversions, ranges, lists, maps, strings, equality (including cycles), float display, top-level code, runtime errors, and unsupported-construct rejection.
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
- Status: BACKLOG

### T-014 — Enums and pattern matching
- Priority: P2
- Status: VERIFIED
- Description: Add closed enums with typed payload variants, construction, exhaustive `match` statements and expressions, pattern bindings, and runtime/checker/native-boundary semantics.
- Acceptance: enum declarations parse and type-check; variant constructors are checked for arity and payload types; closed-enum matches are exhaustive or use `_` (`E0316`); wrong/unknown variants (`E0315`), non-enum scrutinees (`E0314`), nested patterns (`E0112`), and mixed match-expression arm types (`E0301`) are rejected; interpreter construction, matching, equality, display, `type`, and `R0014` work. At the time of T-014 the native backend rejected enums with `E0401`; that boundary was lifted by T-030 and extended to scalar literals by T-032.
- Implementation: `Item::Enum`, `EnumDecl`, `VariantDecl`, `StmtKind::Match`, `ExprKind::Match`, `Pattern`, `Type::Enum`, `EnumInfo`/`VariantInfo`, `Value::Enum`; checker collection/exhaustiveness/pattern scopes; interpreter construction/matching; IR `E0401` boundary.
- Tests: parser unit test; 16 checker unit tests (payload binding, return paths, exhaustiveness, `Any` warning/fallback, non-enum scrutinee, wrong enum, arity, mixed arm types, global variant/enum uniqueness, named payload rejection, payload type mismatch, unreachable arms, duplicate pattern bindings); 3 value unit tests (display/equality/conformance); 12 e2e tests; 3 native `E0401` tests.
- Verification: 2026-11-22 — `cargo build`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass: 110 unit + 34 e2e + 57 native = 201 tests. Manual smoke: enum payload construction, match statements/expressions, recursion, equality, display, and native rejection.
- Scope limits: no nested/or-patterns; no generic enums; variant names are globally unique; enum values are immutable but payload references alias; native enum/match support landed in T-030; scalar literal patterns landed in T-032; guards landed in T-033.

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
- Scope limits: `Any` enum payloads and `match` on `Any` are rejected with `E0401`; nested/or-patterns remain interpreter-unsupported too; enum values are heap-allocated and never freed; enum allocation cost is not yet benchmarked. The defensive native `R0014` branch is unreachable for closed enums because the checker requires exhaustiveness, but remains reachable for guarded wildcard chains on `Any` (which native still rejects).

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
- Scope limits: float, interpolated-string, range, or-pattern, and nested patterns are not supported; guards landed in T-033; string literals are compared with `strcmp`; native matches on `Any` remain rejected with `E0401`; a `Bool` match without `_` must cover both values, while `Int`/`Str` always require `_`.

### T-033 — Match guards
- Priority: P2
- Status: VERIFIED
- Description: Allow an optional `if <expr>` guard after any match pattern. The guard is evaluated only after the pattern matches, with payload bindings in scope, and the arm body runs only when the guard is `Bool` `true`. A guarded arm never contributes to exhaustiveness coverage.
- Acceptance: guards parse after wildcard, variant, and literal patterns in both match statements and expressions; non-`Bool` guards are rejected (`E0301`) and dynamic non-`Bool` guards raise `R0005`; payload bindings are visible to the guard and body; a guarded wildcard or variant does not satisfy exhaustiveness; unreachable arms after an unguarded wildcard still warn (`W0003`) while repeated guarded variants do not; guard evaluation short-circuits arms in source order; interpreter and native output agree.
- Dependencies: T-014, T-032, ADR-0011.
- Implementation: `MatchArm::guard` in `src/ast.rs`; `parse_match_arms` parses `if` guards before the colon; checker checks the guard as `Bool` after pattern bindings are in scope and passes `counts = guard.is_none()` to `check_pattern`; interpreter evaluates the guard in the arm scope (`arm_guard`); IR carries `IrMatchArm::guard` and validates it lowers to `Bool`; C backend builds `arm_condition`, using a GNU C statement expression to test the tag, declare payload bindings, and evaluate the guard when bindings are present.
- Tests: parser unit `parses_match_guards`; checker units `accepts_match_guards_and_payload_bindings`, `rejects_non_bool_match_guard`, `guarded_arms_do_not_satisfy_exhaustiveness`, `warns_on_arm_after_unguarded_wildcard_with_guard`, `duplicate_guarded_variant_arms_are_not_unreachable`; e2e `match_guards_select_arms_at_runtime`; native differential `match_guards_match` (payload guards, `Str` literal guards, wildcard guards with a side-effecting function, and guarded match expressions).
- Verification: 2026-11-23 — `cargo test` 230 passed (126 unit + 37 e2e + 67 native); `cargo clippy --all-targets -- -D warnings` clean; `cargo build` and `cargo build --release` succeed. Manual smoke: guarded enum payloads, guarded literals, guarded wildcards, and guarded match expressions produced byte-identical interpreter/native output (`diff` empty). Negative checks confirmed `E0301` for non-`Bool` guards, `E0316` when guards are the only arms, and `R0005` for a dynamic non-`Bool` guard.
- Scope limits: guards are expressions, not patterns; they cannot introduce bindings; a guard may read but not mutate payload bindings (bindings are immutable); or-patterns and nested patterns remain unsupported; guards on `Any` scrutinees are dynamically checked and native `match` on `Any` remains `E0401`.

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
- Scope limits: heuristic token estimates, in-memory-only vector index, no embedding provider, no streaming CLI, no persistent agent state, no write/shell/network tools, and no live provider evidence.

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
