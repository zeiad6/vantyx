# ADR-0037: Static lints and reachability-aware return checking

- Status: Accepted
- Date: 2026-11-24
- Related: T-005, T-019, T-073, T-074, ADR-0001

## Context

T-019 originally combined a formatter and a linter. The lexer discards
comments, so a comment-preserving formatter needs a separate design and is not
part of this change. A conservative AST linter can be implemented now with no
new dependencies and no effect on program semantics.

While adding an unreachable-statement lint, the checker's `block_returns`
analysis turned out to be wrong: it inspected only the last statement of a
block. A value-returning function with dead code after `return` therefore failed
with `E0309` ("not all paths return"), even though the block always returns.
That is a false compiler error, and it prevented the linter from reporting the
dead code in exactly the functions where it matters.

The interpreter is the semantic oracle. The checker must accept programs whose
reachable control flow returns a value, and the linter must not change what a
program does.

## Decision

- Add `src/lint.rs`, a read-only pass over the checked AST exposed as
  `vantyx::lint::lint(&Program) -> Diagnostics`. It reports warnings only:
  - `L0001`: a local or global `let`/`var` binding is never read.
  - `L0002`: a `var` binding is read but never written after its declaration.
  - `L0003`: a statement follows a direct `return`, `break`, or `continue` in
    the same block.
- Add `vantyx lint <file.vtx>`. It runs the normal lexer, parser, and checker
  first, then the lint pass. Compilation errors still exit `1`; successful
  compilation exits `0` even when lints are reported. Lints are never emitted
  by `vantyx run`, `vantyx check`, or `vantyx build`.
- Keep the first pass deliberately conservative:
  - names starting with `_` are treated as intentionally unused;
  - parameters, loop variables, and `match` payload bindings are not reported;
  - a plain assignment is a write, a compound assignment reads and writes;
  - only the first unreachable statement in a block is reported, and dead code
    is not linted further;
  - `L0003` does not report code after an all-terminating `if`/`match`.
- Fix `block_returns` to walk reachable statements in order. A direct `return`
  terminates the block, a direct `break`/`continue` stops the walk without a
  return, and an `if` with an `else` (or a `match`) whose every branch returns
  terminates the block. Unreachable trailing statements no longer change the
  result.
- Do not implement a formatter, suppression attributes, configuration, JSON
  output, severity levels, or automatic fixes in this change.

## Amendment (T-118, 2026-11-27)

The original decision kept `L0003` to direct terminators. T-118 extends the same
conservative local analysis: a statement is also unreachable after an `if` with
an `else` where the then block, every `elif` block, and the else block
terminate, or after a `match` statement with at least one arm whose every arm
body terminates. An `if` without an `else`, any falling branch, any falling
`match` arm, `while`/`for` loops, and function calls still do not prove
termination. Match-expression arms are single expressions and cannot contain a
terminating statement, so they are treated as falling through. The original
bullet saying `L0003` does not report after an all-terminating `if`/`match` is
superseded; the rest of the original decision stands.

## Consequences

- `vantyx check` and `vantyx run` now accept a value-returning function with
  dead code after `return`; the dead code is still never executed. `vantyx lint`
  reports `L0003` for it.
- Lint findings do not block builds or native compilation. Projects that want a
  gate must inspect the output until a deny mode exists.
- Imported modules are flattened by the module loader before checking, so their
  declarations are linted too; diagnostics render against the originating file.
- The pass has no control-flow graph, data-flow analysis, or interprocedural
  reasoning. Ambiguous cases stay silent by design.
- The linter adds no third-party dependency and no runtime cost outside the
  `lint` command.
- The formatter half of T-019 remains open and requires preserving comments
  through the lexer/parser pipeline.

## Tests

- `src/lint.rs` unit tests cover unused locals and globals, plain versus
  compound assignment, `var` never reassigned, shadowing and initializer scope,
  unreachable code after `return`/`break`, match arm blocks, underscore names,
  parameters, loop variables, and clean programs.
- `src/checker.rs` unit tests cover dead code after `return`, dead code after a
  terminating `if`/`else`, and a `break` that makes a later `return`
  unreachable (which still fails `E0309`).
- `tests/e2e.rs` covers `vantyx lint` findings, a clean run, compile errors,
  standard input, and interpreter execution of dead code after `return`.
- `tests/native.rs::dead_code_after_return_matches` confirms the interpreter
  and native backend agree on the same program.

## Alternatives considered

1. **Report lints from the checker during `run`/`check`.** Rejected: ordinary
   execution would become noisy, and existing programs and tests that expect
   silence from `check` would change behavior. Keeping lint opt-in preserves
   the compiler's contract.
2. **Build a control-flow graph and full data-flow analysis first.** Rejected
   for this change: it is a much larger design with more risk, and the three
   conservative rules are useful without it.
3. **Automatic fixes.** Rejected: the AST has no comment or formatting
   information, so rewrites could delete user intent. A formatter and fixes
   need a lossless syntax representation.
4. **Leave `block_returns` alone and lint only `Unit` functions.** Rejected:
   the `E0309` result is a genuine false positive, and the fix is small and
   directly testable.

## Amendment (2026-11-24, T-085): unused parameters

- Add `L0004` for a function parameter that is never read. It is a separate
  code from `L0001` so a dead local and a dead interface parameter are
  distinguishable.
- Parameters now participate in binding tracking. The leading-underscore
  exemption still applies. A read anywhere in the function, including a guard,
  a nested block, or a match arm block, suppresses the finding. Parameters are
  immutable, so `L0002` never applies to them.
- Loop variables and `match` payload bindings remain unreported.
- The rest of the original decision is unchanged: no suppression attributes,
  no configuration, no interprocedural analysis, and no effect on `run`,
  `check`, or `build`.
- Tests: six `src/lint.rs` unit tests cover used, unused, underscore, void
  function, nested-block read, and the distinct `L0004`/`L0001` codes; one
  `tests/e2e.rs` test covers the CLI warning and exit `0`.
