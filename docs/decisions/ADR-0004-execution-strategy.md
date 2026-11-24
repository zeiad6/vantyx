# ADR-0004: Reference interpreter first, backend-agnostic IR later

- Status: Accepted
- Date: 2026-11-21

## Context
A working language is needed early, but the first execution engine must not become a dead end.

## Options
1. Tree-walking interpreter only.
2. Bytecode VM first.
3. Native code generation first.
4. Reference interpreter now, then IR and a native backend sharing the checked AST.

## Decision
Option 4. The interpreter is the semantic oracle. The AST and checker are backend-neutral. A typed IR and native backend follow once the language core is verified.

## Rationale
- Fastest route to a real, testable language.
- Native codegen before semantics stabilize would multiply rework.
- Differential testing (interpreter vs native) is a strong correctness tool.
- The interpreter is also useful for REPL, scripting, and debugging.

## Consequences
- Performance is not competitive initially; this must be stated honestly.
- Two execution paths eventually exist; differential tests are mandatory.
- The interpreter must avoid leaking backend assumptions into the AST/checker.

## Reversibility
High for execution strategy; the front end is unaffected.

## Reconsideration criteria
If interpreter performance blocks real use cases before the native backend is ready, add a bytecode VM; keep the interpreter as the oracle.
