# Architecture

## Goals

Vantyx must be simple to start and progressively optimizable. The architecture separates a reusable semantic core from execution backends so interpreted and native execution share one language definition.

## Pipeline

```
source (.vtx)
  │
  ▼
lexer ──► tokens + INDENT/DEDENT + spans
  │
  ▼
parser ──► AST (ExprId-annotated)
  │
  ▼
checker ──► resolved names, function signatures, struct/enum layouts, expression types
  │            │
  │            └── diagnostics (E-codes)
  ▼
execution
  ├── tree-walking interpreter (semantic oracle, full language)
  ├── typed IR → C11 backend (native subset, current)
  └── bytecode VM (planned)
```

## Crate layout

| Module | Responsibility |
| --- | --- |
| `span` | Byte spans, line/column mapping |
| `diagnostic` | Error/warning records and source rendering |
| `token` | Token kinds and lexer output |
| `lexer` | Indentation-aware tokenizer |
| `ast` | Typed AST data structures |
| `parser` | Recursive descent + Pratt expressions |
| `types` | Type representation and display |
| `checker` | Scopes, signatures, gradual type checking |
| `value` | Runtime values |
| `interp` | Evaluation, environments, builtins |
| `ir` | Typed, backend-neutral intermediate representation |
| `codegen_c` | GNU C11 emission for the native backend |
| `lib` | Stable API: parse, check, run, compile |
| `main` | CLI |

No compiler module depends on the interpreter. The interpreter consumes only the AST plus the checker's semantic model.

## Key design decisions

- **One AST, multiple consumers.** The checker annotates `ExprId → Type`; a future IR lowering pass consumes the same AST. No second parser.
- **Fail-stop runtime.** Dynamic operations are checked; a failed check is a diagnostic, never undefined behavior.
- **Explicit spans everywhere.** Every token, expression, statement, and declaration carries a span for diagnostics and future debug info.
- **No third-party dependencies in the compiler core.** Fast builds, small supply chain, easy auditing.
- **Reference interpreter first.** It is the semantic oracle. Native/VM backends must match it; `tests/native.rs` performs differential interpreter-vs-native checks for the supported subset.
- **Explicit native subset.** The IR lowers only statically typed constructs it can represent. Anything else is rejected with `E0401`, never silently miscompiled (ADR-0008).

## Non-goals for v0.1

- Maximum interpretation speed.
- Complete Rust-style ownership syntax.
- GPU/AI runtime.
- Sandboxing untrusted code.

## Extension points

1. **IR**: the current typed IR is intentionally small and non-SSA. It can evolve toward explicit locals/control-flow blocks without changing the front end.
2. **Backends**: C emission is implemented (small, portable, uses system clang); Cranelift or LLVM will be evaluated later with benchmarks.
3. **Ownership analysis**: a separate pass over the checked AST; opt-in annotations; escape analysis for stack allocation.
4. **Concurrency**: design requires shared-state rules before implementation; no data races by construction.
5. **Tooling**: formatter operates on tokens/AST; LSP reuses parser/checker with incremental source maps.
