# Integration proposal: tuple types, destructuring, and tuple patterns

Date: 2026-11-28
Owner: Agent 01 (core language, compiler, and runtime)
Status: implemented and verified in core-owned files. Protected status files are
untouched and need the edits in §8.

## 1. What changed

Vantyx now has structural tuple types, tuple values, numeric field access,
destructuring bindings, and tuple patterns in the interpreter. The native
backend rejects every tuple construct with `E0401` and a concrete help message;
it never lowers or miscompiles one.

The feature completes the first item in `ROADMAP.md`'s next-core-priorities
list ("tuple patterns once a tuple type exists"). It is interpreter-only by
design: the native subset stays statically typed and the tuple representation
is not part of the typed IR yet.

## 2. Semantics implemented

### Types and values

- Type expressions: `(Int, Str)`, nested `((Int, Int), Str)`. `()` in type
  position resolves to `Unit`. A one-element tuple type requires a trailing
  comma: `(Int,)`.
- Literals: `(1, "x")`, nested `((1, 2), "x")`. A one-element tuple literal
  requires a trailing comma: `(1,)`. `()` is not a value; it is rejected with
  `E0101` and a help message that points at `Unit`/zero-argument calls.
- Parentheses without a comma keep their existing grouping meaning, so
  `(1)` is `Int`, not a one-element tuple.
- Tuples are immutable values. `pair.0 = value` is `E0306` statically and
  `R0010` at runtime when reached through `Any`.
- Field access uses numeric postfix indices only: `pair.0`, `pair.1`,
  chained `pair.1.0`. Out-of-range and non-numeric field access are static
  errors; tuples are not indexable with `[]` and are not iterable.
- Structural equality and display: `(1, "x")` displays as `(1, "x")` and
  nested strings are quoted. A one-element tuple displays with a trailing
  comma, `(1,)`. `str(tuple)`, `type(tuple)`, interpolation, and `==`/`!=`
  work. Tuples have no ordering (`<`, `>`) and cannot be sorted.

### Bindings and patterns

- `let (a, b) = pair` and `var (a, b) = pair`, including nesting and `_`
  wildcards: `let ((x, _), y) = value`.
- Destructured names join one immutable or mutable scope according to the
  binding form; duplicate names and arity mismatches are static errors.
  Runtime values reached through `Any` are checked before binding (`R0005`).
- Tuple patterns in `match` statements and expressions: scalar literals,
  integer ranges, enum variants, nested tuples, and top-level or-patterns.
  An arity mismatch is a static error, and a non-exhaustive tuple match is
  `E0316`.
- A fixed-arity tuple pattern on an `Any` scrutinee never marks coverage
  complete, because it cannot cover non-tuples or other arities; this avoids
  a spurious `W0003` for later arms.
- Tuple values are usable as map keys, struct fields, enum payloads, list
  elements, function parameters, and return values in the interpreter.

### Native boundary

`src/ir.rs` rejects tuple types, tuple literals, tuple field access, tuple
patterns, and tuple destructuring with `E0401`. Top-level destructuring is
rejected during global collection so the diagnostic is the tuple `E0401`, not
a confusing `E0402`.

## 3. Files changed (core-owned only)

| File | Change |
| --- | --- |
| `src/ast.rs` | `ExprKind::Tuple`, `TypeExprKind::Tuple`, `Pattern::Tuple`, `StmtKind::LetPattern`, `LetPattern` |
| `src/types.rs` | `Type::Tuple(Vec<Type>)`, structural assignability/display, unit-tuple resolution, unit tests |
| `src/parser.rs` | Tuple types/literals/patterns, `(x,)`, numeric postfix fields, destructuring bindings, unit tests |
| `src/lexer.rs` | Digits after `.` lex as an integer tuple index so `pair.0.1` works; ordinary floats unchanged; unit tests |
| `src/checker.rs` | Tuple typing/access/destructuring/patterns, `E0306`, `E0316`, coverage and `W0003` rules |
| `src/value.rs` | `Value::Tuple`, display/equality/conformance/tightening, unit tests |
| `src/interp.rs` | Tuple evaluation, destructuring, pattern bindings, `R0010`/`R0005` |
| `src/ir.rs` | `E0401` rejections for every tuple construct, including top-level destructuring |
| `src/lint.rs` | Destructured bindings tracked for `L0001`/`L0002`/`L0004`; tuple AST traversal |
| `src/main.rs` | REPL completion for destructured names |
| `tests/e2e.rs` | 8 tuple end-to-end and regression tests |
| `tests/native.rs` | 1 differential `E0401` rejection test |
| `docs/language_specification.md` | Tuple syntax, semantics, errors, native limits |
| `docs/type_system.md` | Tuple typing, covariance, conformance |
| `docs/decisions/ADR-0059-tuple-types-and-destructuring.md` | Design decision and consequences |

No protected file, AI-owned file, or shared status document was modified by
this work.

## 4. Tests

13 new library unit tests:

- `src/types.rs`: `tuples_are_structural_and_position_sensitive`,
  `tuple_display_uses_a_trailing_comma_for_one_element`
- `src/value.rs`: `tuples_display_with_nested_string_quoting`,
  `tuples_compare_structurally_and_never_with_other_kinds`,
  `tuple_conformance_and_tightening_are_element_wise`
- `src/checker.rs`: `tuple_element_mismatch_is_reported_once`
- `src/parser.rs`: `parses_tuple_literals_and_keeps_parentheses_grouping`,
  `parses_nested_tuple_index_access`,
  `parses_tuple_types_and_destructuring_bindings`,
  `parses_tuple_patterns_with_one_element_trailing_comma`,
  `rejects_empty_tuple_expressions_and_binding_patterns`
- `src/lexer.rs`: `lexes_nested_tuple_indices_as_separate_dots`,
  `still_lexes_floats_after_ordinary_numbers`

8 new end-to-end tests (`tests/e2e.rs`):

- `tuples_construct_access_and_display`
- `tuple_destructuring_and_patterns_work`
- `tuple_static_errors_are_rejected`
- `tuple_any_boundary_is_checked_at_runtime`
- `lint_tracks_destructured_bindings`
- `tuple_pattern_arity_error_does_not_cascade_to_unreachable_arm`
- `tuple_patterns_on_any_do_not_make_later_arms_unreachable`
- `tuple_match_expressions_and_or_patterns_work`

1 new differential native test (`tests/native.rs`):

- `tuple_constructs_are_rejected_with_e0401`

The e2e tests cover tuple construction, annotation, chained access, display,
interpolation, equality, map keys, struct fields, enum payloads, `let`/`var`
destructuring with wildcards and nesting, match statements/expressions,
literals, ranges, variants, or-patterns, static errors (`E0101`, `E0306`,
`E0316`), `Any` runtime boundaries (`R0005`), lint tracking, and the two
coverage regressions.

## 5. Verification evidence

Observed on 2026-11-28 in the shared repository:

```sh
cargo build                                  # success
cargo build --release                        # success
cargo clippy --all-targets -- -D warnings    # clean
cargo fmt --check --all                      # clean
cargo test                                   # exit 0
```

`cargo test` totals:

```text
460 library unit + 15 CLI unit + 56 AI integration + 145 end-to-end + 166 native = 842 passed, 0 failed
```

The 56 AI integration tests and 22 of the library tests belong to the
concurrent Agent 02 bounded-retry work. The T-128 core delta is therefore
**13 library unit + 8 e2e + 1 native = 22 tests**.

Additional checks:

```sh
python3 tools/test_differential_fuzz.py                 # 9 tests passed
python3 tools/differential_fuzz.py --iterations 100 --seed 1   # 100/100 agreed
```

- All five `examples/*.vtx` ran under the interpreter and as native binaries
  with matching output.
- `VANTYX_CC=gcc cargo test --test native`: see §5.1.
- The ASan/UBSan native suite (`tools/clang-sanitize.sh`, leak detection
  disabled because v0.1 native allocations are intentionally leaked): see §5.1.
- Manual boundary probes confirmed that an `Any` tuple reaching a typed
  parameter or return reports `R0005` with the expected tuple type and the
  runtime value's `Tuple` type.

### 5.1 GCC and sanitizer native runs

```text
VANTYX_CC=gcc cargo test --test native
  test result: ok. 166 passed; 0 failed; finished in 67.48s

ASAN_OPTIONS=detect_leaks=0:abort_on_error=1 \
UBSAN_OPTIONS=halt_on_error=1:print_stacktrace=1 \
VANTYX_CC="$PWD/tools/clang-sanitize.sh" cargo test --test native
  test result: ok. 166 passed; 0 failed; finished in 184.47s
```

Leak detection is disabled because v0.1 native allocations are intentionally
leaked. The tuple native test only exercises the `E0401` rejection path; no
tuple code is emitted into generated C.

## 6. Limitations and non-goals

- Tuples are immutable and have no assignment, iteration, `[]` indexing,
  ordering, hashing beyond structural equality, named fields, spread, or
  generic parameters.
- Tuples are interpreter-only. The native backend rejects all tuple
  constructs with `E0401`; no typed-IR tuple representation or C lowering
  exists yet.
- `let (a, b) = value` derives static binding types from the initializer when
  the checker knows its tuple type; otherwise the bindings are `Any` and are
  checked at runtime.
- A fixed-arity tuple pattern never completes coverage of an `Any` scrutinee.
- Tuple display and conformance are structural; there is no tuple-specific
  optimization or benchmark claim. This change adds no overhead to existing
  non-tuple programs.
- No new dependencies, no lexer/parser recursion-limit changes, and no changes
  to existing pattern semantics beyond the tuple coverage rules above.

## 7. Why this feature

The roadmap named tuple patterns as the first next core priority once a tuple
type existed. Tuples remove the need for ad-hoc single-use structs for
fixed-size heterogeneous groups, make destructuring and match arms expressive,
and exercise the gradual type system's structural conformance and runtime
boundary rules. The design keeps the interpreter as the oracle and the native
boundary explicit rather than silently lowering an unsupported representation.

## 8. Required edits to protected files

These edits are required by repository rule 4 (behavior changes update
`TASKS.md` and `PROJECT_STATUS.md`, plus the specification/ADR). They are
**not applied** here.

### 8.1 `TASKS.md` — append a new T-128 section after T-127

```markdown
### T-128 — Tuple types, tuple values, destructuring, and tuple patterns
- Priority: P2 (language expressiveness; completes the first item in the roadmap's next-core-priorities list)
- Status: VERIFIED locally on 2026-11-28; commit pending; CI pending
- Description: Vantyx had no tuple type or value, so fixed-size heterogeneous groups required ad-hoc structs, and `ROADMAP.md` listed "tuple patterns once a tuple type exists" as the first next core priority. `(Int, Str)` and `(1, "x")` were parse errors, and no binding or pattern could destructure a group.
- Acceptance: Tuple type expressions and tuple literals of arity ≥ 2; `()` only as `Unit` in type position; one-element tuples require a trailing comma (`(x,)`); numeric field access with chained indices (`pair.0`, `pair.1.0`); `let`/`var` destructuring with `_`; tuple patterns in both `match` forms with scalar literals, integer ranges, enum variants, nested tuples, and top-level or-patterns. Structural equality, display, `str`, `type`, interpolation, tuple map keys, struct fields, and enum payloads. The interpreter is the oracle; every tuple construct is rejected by the native backend with `E0401` and concrete help, never miscompiled.
- Dependencies: ADR-0011, ADR-0019, ADR-0020, ADR-0021, ADR-0022, ADR-0059.
- Implementation: `src/types.rs` adds `Type::Tuple(Vec<Type>)` with structural assignability and display; `src/ast.rs` adds `ExprKind::Tuple`, `TypeExprKind::Tuple`, `Pattern::Tuple`, `StmtKind::LetPattern`, and `LetPattern`; `src/parser.rs` parses tuple types/literals/patterns, `(x,)`, numeric postfix fields, and destructuring bindings while keeping `(...)` grouping unchanged; `src/lexer.rs` lexes digits after `.` as an integer index so `pair.0.1` works without changing floats; `src/checker.rs` types construction/access/destructuring/patterns, reports `E0306` for tuple field assignment and `E0316` for non-exhaustive tuple matches, tracks destructured bindings for `W0003`, and makes tuple literals adopt expected element types so a boundary mismatch is reported once at the element; `src/value.rs` adds `Value::Tuple` with structural equality, display, conformance, and contract tightening; `src/interp.rs` evaluates tuples, destructures bindings, matches tuple patterns, and raises `R0010` for tuple mutation; `src/lint.rs` tracks destructured bindings; `src/main.rs` completes destructured names in the REPL; `src/ir.rs` rejects tuple types, expressions, patterns, and destructuring with `E0401`, including top-level destructuring.
- Tests: 13 new library unit tests (`src/types.rs`, `src/value.rs`, `src/checker.rs`, `src/parser.rs`, `src/lexer.rs`); 8 new e2e tests covering construction/access/display, destructuring and patterns, static errors, `Any` runtime boundaries (`R0005`), lint tracking, arity-cascade and `Any`-coverage regressions, and match expressions/or-patterns; `tests/native.rs::tuple_constructs_are_rejected_with_e0401` covers every rejected form, including top-level destructuring.
- Verification: 2026-11-28 — `cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check --all` clean; `cargo test` = 842 passed (460 library unit + 15 CLI unit + 56 AI integration + 145 e2e + 166 native, 0 failed; the totals include 22 library and 9 AI integration tests from concurrent Agent 02 retry work, so the core delta is 13 unit + 8 e2e + 1 native); `VANTYX_CC=gcc cargo test --test native` = 166 passed; the same 166-test suite passed under clang AddressSanitizer/UndefinedBehaviorSanitizer with leak detection disabled; 9 fuzzer self-tests; 100/100 differential cases on seed 1; all five examples matched interpreter and native output.
- Scope limits: Tuples are immutable values; no tuple assignment, iteration, `[]` indexing, or ordering beyond structural equality. No named tuple fields, spread, or generic tuples. Native code rejects all tuple constructs. Destructured `let` bindings derive static types from the initializer when known, otherwise `Any`; a fixed-arity tuple pattern never completes coverage of an `Any` scrutinee.
```

### 8.2 `PROJECT_STATUS.md`

(a) Line 3 begins `Updated: 2026-11-28 (T-127 makes...`. Insert a T-128
clause immediately after the opening parenthesis:

```text
Updated: 2026-11-28 (T-128 adds interpreter tuple types, tuple values, numeric
field access, destructuring bindings, and tuple patterns with structural
equality/display and `E0401` native rejection; T-127 makes...
```

(b) Line 44 ends `Payload or-patterns and tuple patterns remain unsupported.`
Replace with:

```text
Payload or-patterns remain unsupported.
```

Then add a new bullet after it:

```markdown
- Tuple types and patterns (T-128, ADR-0059): `(Int, Str)` type expressions and `(1, "x")` values, numeric access (`pair.0`, chained `pair.1.0`), `let`/`var` destructuring with `_`, and tuple patterns in both match forms (literals, integer ranges, enum variants, nested tuples, or-patterns). Tuples are immutable values with structural equality/display and can be map keys, struct fields, and enum payloads; field assignment is `E0306` (runtime `R0010` through `Any`), a non-exhaustive tuple match is `E0316`, and a fixed-arity tuple pattern never completes coverage of an `Any` scrutinee. The native backend rejects every tuple construct with `E0401`.
```

(c) Line 49, the runtime value list, change:

```text
`Map` (insertion-ordered), nominal `Struct`, nominal `Enum`.
```

to:

```text
`Map` (insertion-ordered), `Tuple`, nominal `Struct`, nominal `Enum`.
```

(d) After the or-pattern/nested-pattern runtime bullet (line 54), add:

```markdown
- Tuple construction, field access, destructuring, pattern matching, structural equality/display, interpolation, `str`, and `type` (T-128). Tuple field assignment is `R0010`; an arity or type mismatch reached through `Any` is `R0005`.
```

(e) Line 150, replace the partially-implemented bullet with:

```markdown
- **Enums and pattern matching** — closed enums, payloads, exhaustive matching, `Any` matching, scalar literal patterns, nested payload patterns, top-level or-patterns, integer range patterns, and guards are implemented in the interpreter and native backend. Tuple types, tuple values, destructuring, and tuple patterns are implemented in the interpreter and rejected natively with `E0401` (T-128, ADR-0059). Range expressions, float/interpolated-string patterns, generic enums, and explicit discriminants are not implemented (ADR-0011, ADR-0019, ADR-0020, ADR-0021, ADR-0022).
```

(f) Line 169 in the not-yet-implemented list, replace:

```text
- Tuple patterns once a tuple type exists; usefulness-based interval analysis
```

with:

```text
- Usefulness-based interval/unreachable analysis for nested, alternative, and range patterns
```

(g) Line 264 in "Next tasks", replace:

```text
4. Tuple patterns once a tuple type exists, and usefulness-based interval/unreachable analysis
```

with:

```text
4. Usefulness-based interval/unreachable analysis for nested, alternative, and range patterns (tuple types and patterns now ship as T-128)
```

(h) The "Last verification" opening sentence currently says `... = 789 tests
passed, clippy and fmt clean.` Replace that opening sentence with:

```text
`cargo build`, `cargo build --release`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check --all`, and `cargo test` on 2026-11-28: 460 library unit + 15 CLI unit + 56 AI integration + 145 end-to-end + 166 native = 842 tests passed, clippy and fmt clean. The 56 AI integration tests and part of the library total come from concurrent Agent 02 retry work; the T-128 core delta is 13 library unit + 8 e2e + 1 native tests. T-128 adds interpreter tuple types, tuple values, numeric field access, destructuring bindings, and tuple patterns, with structural equality/display, `E0306`/`E0316` diagnostics, `R0005`/`R0010` runtime checks, and `E0401` native rejection; ADR-0059, specification §2.1, §2.4, §4.1, §4.6, §5.7–§5.9, and §9, and `docs/type_system.md` document the design.
```

Keep the rest of the paragraph (the T-124/T-125/T-126/T-127 and historical
evidence) unchanged.

### 8.3 `ROADMAP.md`

Replace the first item in "Next core priorities":

```markdown
1. **Tuple patterns** once a tuple type exists, plus usefulness-based
   unreachable warnings for nested, alternative, and range patterns.
```

with:

```markdown
1. **Usefulness-based unreachable warnings** for nested, alternative, and
   range patterns; tuple types, destructuring, and tuple patterns now ship in
   the interpreter (T-128) and are rejected natively with `E0401`.
```

### 8.4 `README.md`

(a) In the v0.1 feature paragraph (around line 75), after the
`Map[K,V]` sentence and before "Top-level or-patterns", insert:

```text
Tuple types such as `(Int, Str)` with `(1, "x")` values, numeric access
(`pair.0`), destructuring bindings (`let (a, b) = ...`), and tuple patterns in
`match` run in the interpreter.
```

(b) In the "Known limits" list, replace:

```markdown
- No tuple patterns. Nested payload patterns, top-level or-patterns, and
  integer range patterns (`low..high`, `low..=high`) run in both the
  interpreter and native builds.
```

with:

```markdown
- Tuple types, values, access, destructuring, and patterns run only in the
  interpreter; native builds reject every tuple construct with `E0401`.
  Nested payload patterns, top-level or-patterns, and integer range patterns
  (`low..high`, `low..=high`) run in both the interpreter and native builds.
```

(c) In the Status table, extend the `Pattern matching` row with
`; tuple patterns in the interpreter (native `E0401`)`, and the
`Native boundaries` row with `, and tuple constructs`.

### 8.5 `docs/PROJECT_BASELINE.md`

- Change `Date: 2026-11-27` to `Date: 2026-11-28` and prepend
  `the T-128 tuple types and patterns, ` to the `Session: re-baselined after
  the ...` list.
- Change the `## Current re-baseline (2026-11-26)` heading to
  `## Current re-baseline (2026-11-28)`.
- `Type system` row: append
  `, and structural tuple types with immutable values, numeric field access, destructuring bindings, and tuple patterns (T-128, ADR-0059)`.
- `Interpreter` row: append
  `, and tuple construction, access, destructuring, and pattern matching (T-128)`.
- `Native path` row: append
  `; tuple types, values, access, destructuring, and patterns are rejected with `E0401` (T-128)`.
- `Native exclusions` row: add
  `tuple types, tuple values, numeric tuple field access, tuple destructuring, and tuple patterns, ` before `and dynamically typed file...`.
- `Tests` row: replace the `789 passing` breakdown with
  `460 library unit + 15 CLI unit + 56 AI integration + 145 end-to-end + 166 native = **842 passing** locally on 2026-11-28`,
  and add a T-128 sentence describing the 22 core tests and the concurrent
  Agent 02 delta.
- `Main risks` row: add
  `tuples are immutable and interpreter-only (native `E0401`), `.

### 8.6 `AGENTS.md`

- Replace the test bullet:

```markdown
- `cargo test`: 789 passed (425 library unit + 15 CLI unit + 47 AI integration +
  137 e2e + 165 native); the full native suite also passes with `VANTYX_CC=gcc`.
```

with:

```markdown
- `cargo test`: 842 passed (460 library unit + 15 CLI unit + 56 AI integration +
  145 e2e + 166 native); the library and AI totals include concurrent Agent 02
  retry tests, so the T-128 core delta is 13 unit + 8 e2e + 1 native. The full
  native suite also passes with `VANTYX_CC=gcc`.
```

- After the T-123 runtime-contract bullet, add:

```markdown
- Tuple types, tuple values, numeric field access, destructuring bindings, and
  tuple patterns are implemented in the interpreter (T-128, ADR-0059). Tuples
  are immutable and structural, support equality/display/interpolation, and
  may be map keys, struct fields, or enum payloads. The native backend rejects
  every tuple construct with `E0401`; there is no typed-IR tuple
  representation.
```

- In the known-gaps bullet, add
  `immutable interpreter-only tuples with no native lowering (T-128), ` after
  `no generics/closures/async/FFI, `.

## 9. Commit and CI

No commit was made and no branch was changed. Suggested commit message:

```text
feat(core): add tuple types, destructuring, and tuple patterns
```

CI is pending. The working tree also contains Agent 02's uncommitted AI retry
work (`src/ai/retry.rs`, `src/ai/*`, `src/ai_cli.rs`, `tests/ai.rs`,
`docs/ai/*`, `docs/decisions/ADR-0058-*`); do not stage those files with this
change.

## 10. Coordination notes

- All T-128 edits are confined to core-owned files plus the three core
  documentation files listed in §3.
- The shared test totals include Agent 02's concurrent tests; the core-only
  delta is stated explicitly in every proposed status edit.
- The existing `tests/e2e.rs` and `tests/native.rs` helpers and temp-root
  policy were reused; no hermeticity behavior changed.
