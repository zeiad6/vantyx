# Active Task

## T-075 — Result-enum propagation with postfix `?`

- Status: VERIFIED locally; commit, push, and CI pending
- Priority: P1 (language feature)
- Dependencies: T-014, T-030, T-032, T-048, ADR-0001, ADR-0019, ADR-0038

### Implemented

- `TokenKind::Question`, lexer punctuation, and `ExprKind::Try`.
- Postfix parsing at precedence level 1 with expression-depth accounting.
- `?` is accepted only as the whole value of a `let` initializer, a plain
  assignment value, or an expression statement. The checker sets a
  `try_allowed` flag only for those positions; every nested expression clears
  it.
- The operand must be a nominal enum with exactly two variants, each with
  exactly one payload field. The first declared variant is success and the
  second is failure. The enclosing function must have an explicit return type
  equal to the operand's enum. Invalid cases report `E0319`.
- Interpreter: `eval_try` returns either the success payload or the failure
  value; statement handling turns the failure into `Flow::Return`. A defensive
  `R0017` covers operands that bypass the checker.
- Native IR: a dedicated `IrStmtKind::Try` with `lower_try` and
  `lower_try_assignment`. Codegen evaluates the operand once, returns it on the
  failure tag (decrementing the call depth), and binds the success payload field
  to a fresh local. It is not lowered as a match expression.
- Lint traversal visits the `Try` child.
- `tools/differential_fuzz.py` gained a ninth shape, `result_program`, covering
  success, failure, loops, assignment, and bare-statement propagation.
- Specification §5.10, the §11 native subset, §12 not-implemented wording,
  runtime-error `R0017`, ADR-0038, `TASKS.md`, `PROJECT_STATUS.md`, `AGENTS.md`,
  and `docs/PROJECT_BASELINE.md` are updated.

### Evidence

- `cargo test --test e2e try_` — 5 passed.
- `cargo test --test native try_` — 3 passed.
- Full `cargo test` — 348 library unit + 15 CLI unit + 32 AI integration +
  101 end-to-end + 125 native = 621 passed.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo build` and `cargo build --release` — success.
- `python3 tools/differential_fuzz.py --iterations 180 --seed 1` — 180/180.
- Same tool on seeds 500 and 1000 — 180/180 each.
- `python3 -m py_compile tools/differential_fuzz.py` — success.
- All five `examples/*.vtx` matched interpreter/native output.

### Remaining

- Commit, push, watch CI, and record the run ID.
- Scope limits: statement-position only; no generic `Result`, error
  conversion, `try` block, or expression-position `?`; positional
  first-success/second-failure convention; variant constructors stay globally
  unique; native allocations still leak.

## Recently completed

- T-072 fuzzer expansion: committed in `53a8b82`; CI run 45499019615 passed
  with the expanded fuzzer. 160/160 on seeds 1, 500, and 1000 before the ninth
  shape.
- T-073/T-074 linter and reachability-aware returns: committed in `5e1092f`;
  CI run 45496041422 passed with 613 tests.
- T-070/T-071 native condition and short-circuit fixes: committed in
  `a4fe86b`; CI run 45493833079 passed with 584 tests.
