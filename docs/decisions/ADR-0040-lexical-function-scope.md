# ADR-0040: Lexical function scope

- Status: Accepted
- Date: 2026-11-25
- Related: T-087, T-072

## Context

Vantyx bindings are lexically scoped: the checker resolves each name against
the function's parameters and body, then the program's top-level bindings, and
shadowing is allowed only in an inner block. The native backend follows that
model because C locals are lexical.

The interpreter kept one flat stack of scopes and searched it from the newest
scope backward. A user function pushed its frame onto the same stack, so name
resolution continued into every caller frame below it. A caller local with the
same name as a global therefore shadowed the global inside the callee.

A differential probe found two observable failures:

```vantyx
var total = 10
var other = 20

fn read_total() -> Int:
    return total

fn bump_total() -> Int:
    total = total + 1
    return total

fn bump_other() -> Int:
    other = other + 1
    return other

fn main():
    var total = 100
    let other = "caller-immutable"
    print(read_total())
    print(bump_total())
    print(total)
    print(bump_other())
    print(other)

main()
```

- The interpreter printed `100` for `read_total()` because it read `main`'s
  local `total`; native printed the global `10`.
- `bump_total()` silently changed `main`'s local `total` instead of the
  global, so `main` printed `101`; native left the caller local at `100`.
- `bump_other()` found `main`'s immutable local `other` and raised `R0010`
  ("cannot assign to immutable binding"); native updated the global `20` to
  `21`.

The checker accepted all three calls and resolved each name to the global,
which confirms that the interpreter, not the source program, was wrong. The
same confusion could read or mutate caller state in any valid program that
happens to reuse a global's name as a local, so this is a correctness and
state-isolation defect rather than a cosmetic mismatch.

## Decision

A function call establishes a lexical frame boundary.

- Name lookup searches the active function frame first: its parameters,
  locals, and nested block scopes, from innermost outward.
- If the name is not in that frame, lookup continues in the program's root
  (top-level) scope only.
- Scopes belonging to callers, including top-level block scopes outside the
  current call, are never searched.
- Assignment resolves its target by the same rule, so a callee either writes
  its own binding or the global; it cannot write a caller local.
- The previous frame boundary is restored when the call returns, including on
  the error path, so recursion and nested calls keep independent frames.

The interpreter stores the first scope index of the active call and leaves the
root scope at index zero; `lookup` and `find_scope` apply the search rule. No
syntax, type rule, runtime helper, or native backend change is required.

## Consequences

- The interpreter and native backend now agree on the probe: `10`, `11`,
  `100`, `21`, `caller-immutable`.
- A caller local can no longer shadow a global for a callee, and a callee can
  no longer trigger a false immutable-binding error from a caller local.
- A callee's own local still shadows the global inside that callee, as
  lexical block scoping requires.
- Recursive calls keep separate frames; the boundary is saved and restored
  around each invocation.
- The interpreter remains the reference implementation for this rule, and the
  native backend already matched it. This change makes the oracle correct
  rather than changing native behavior.
- Lookup cost is proportional to the active frame's block depth plus at most
  one root-scope probe. No performance claim is made without measurement.

## Tests

- `tests/e2e.rs::functions_resolve_globals_not_caller_locals` pins the
  interpreter output for a mutable caller local, reads, writes, and an
  immutable caller local with the same name as a global.
- `tests/native.rs::functions_use_lexical_globals_matching` runs the same
  program through both engines and compares stdout and exit code.
- The `functions_program` shape in `tools/differential_fuzz.py` now declares
  two globals and callers that shadow them, so every generated functions case
  exercises the boundary.

## Alternatives considered

1. **Adopt dynamic scoping.** Rejected. It contradicts the checker's static
   resolution, makes every function depend on its caller's local names, and
   turns an accidental name collision into silent state mutation.
2. **Copy globals into each call frame.** Rejected. Assignments to a global
   would update the copy, breaking the existing global-mutation behavior that
   both engines already support.
3. **Resolve names statically in the interpreter.** A larger change that
   would duplicate checker resolution at runtime. The frame boundary fixes the
   defect with the existing scope representation and keeps the runtime
   independent of the checker's internal tables.
