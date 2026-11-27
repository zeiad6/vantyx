# Static lints

`vantyx lint <file.vtx>` compiles a program with the same lexer, parser, and
checker as `vantyx check`, then runs non-fatal lint passes over the checked
AST. It never executes the program.

```sh
vantyx lint examples/hello.vtx
vantyx lint - < program.vtx
```

Lint findings are warnings. They do not change compilation, interpretation, or
native output. `vantyx lint` exits `0` when compilation succeeds even if it
reports findings, `1` when compilation fails, and `64` for invalid usage. Use
`vantyx check` when only compiler diagnostics matter.

## Rules

### `L0001` — unused binding

A local or top-level `let`/`var` binding is declared but never read.

```vtx
fn main():
    let unused = 1      # L0001
    print("ok")
```

The initializer is evaluated before the new binding enters scope, so
`let value = value + 1` reads an outer `value`; the inner binding is still
reported if nothing else reads it. A plain assignment (`value = 1`) is a write,
not a read. A compound assignment (`value += 1`) reads and writes.

Bindings whose names start with `_` are treated as intentionally unused.
Loop variables and `match` payload bindings are not reported in this first lint
pass. An unused function parameter is reported as `L0004` instead, so a dead
local and a dead interface parameter are distinguishable.

### `L0002` — `var` never reassigned

A `var` binding is read but never written after its declaration. `let` would
express the same program.

```vtx
fn main():
    var label = "ready" # L0002
    print(label)
```

A binding that is never read is reported as `L0001` instead. Writes from any
nested block count, including compound assignments.

### `L0003` — unreachable statement

A statement follows a direct `return`, `break`, or `continue` in the same
block.

```vtx
fn f() -> Int:
    return 1
    print("dead")       # L0003
```

Only the first unreachable statement in a block is reported, and dead code is
not linted further. A statement after an `if` or `match` whose branches all
terminate is not reported; the pass deliberately avoids full control-flow
analysis.

### `L0004` — unused parameter

A function parameter is never read. The parameter still participates in name
resolution and keeps its position in the signature.

```vtx
fn constant(value: Int) -> Int: # L0004
    return 1
```

A parameter named with a leading underscore (`_value`) is treated as
intentionally unused. Reading the parameter anywhere in the function, including
a guard, a nested block, or a match arm block, suppresses the warning. Because
parameters are immutable, `L0002` never applies to them.

## Scope and limits

- The pass is conservative: when reachability or a read is ambiguous, it stays
  silent rather than guessing.
- Imported modules are part of the checked program, so their declarations are
  linted and findings render against the imported file.
- There are no suppression attributes, configuration files, severity levels,
  machine-readable output, or automatic fixes yet.
- `L0003` does not perform interprocedural analysis and does not report
  unreachable code after an all-terminating `if`/`match`.
- `L0004` is purely lexical: any read suppresses it, including passing the
  parameter to another call. There is no interface or trait system yet, so no
  conformance reason to keep an unread name is recognized.
- The formatter remains future work. The current lexer discards comments, so a
  formatter needs a separate design that preserves them.

See [ADR-0037](decisions/ADR-0037-static-lints.md).
