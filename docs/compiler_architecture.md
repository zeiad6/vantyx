# Compiler Architecture

## Stage contracts

### Lexer
Input: `&str`, file name.
Output: `Vec<Token>`, `Vec<Diagnostic>`.
Guarantees: always terminates, never panics on arbitrary UTF-8 input, emits `Indent`/`Dedent`/`Newline`, rejects tab indentation, reports unterminated strings and invalid numbers.

### Parser
Input: tokens.
Output: `ast::Program`.
Guarantees: bounded expression/block/type depth, synchronizes at newlines after errors, reports expected/found tokens with spans. Enum declarations and both match forms (`match` statement with block arms, `match` expression with one-expression arms) are parsed into explicit AST nodes; patterns include wildcards, integer/string/boolean literals, variant bindings, recursively nested payload patterns (bounded at 64 levels), top-level `|` alternatives, and an optional `if <expr>` guard. Import declarations are parsed as `Item::Import`; relative paths are literal, `.vtx`-suffixed strings (`E0512`), and each selective name may appear once (`E0511`).

### Module loader
Input: entry path.
Output: flattened `ast::Program`, `SourceRegistry`, diagnostics.
Guarantees: every reachable file is canonicalized and parsed once; imports are relative to the importing file and confined to the entry file's directory (`E0502`); cycles (`E0503`), unreadable or missing targets (`E0501`), unknown selected names (`E0504`), executable statements in modules (`E0505`), non-entry `main` (`E0506`), and file/depth/byte limits (`E0507`–`E0509`) are diagnosed. Non-entry files contribute only imports and declarations. Per-file spans and globals are retained, and `ExprId`s are seeded by source file so cross-file expression-type maps cannot collide.

### Checker
Input: AST.
Output: `SemanticModel` (functions, structs, enums, expression types) or diagnostics.
Guarantees: every variable use resolved; annotated types validated; `Any` permitted but explicit; no implicit numeric coercion; control-flow validity checked. Enum names and variant constructor names are globally unique, variant payload types are resolved, pattern bindings are scoped per arm, literal and integer range patterns are type-checked against the scrutinee (empty ranges are `E0318`), guards are checked as `Bool` after bindings enter scope (`E0301`), and closed-enum and `Bool` matches are checked for exhaustiveness (`E0316`) with guarded arms excluded from coverage.

### Runtime
Input: AST + `SemanticModel`.
Output: process output or `RuntimeError` with span and stack trace.
Guarantees: checked arithmetic, bounds checks, call-depth limit, type validation at dynamic boundaries, exhaustive-match enforcement for dynamic values (`R0014`), immutable enum variant/payload slots, structural enum equality/display, literal matching on `Int`/`Str`/`Bool`, guard evaluation after pattern bindings with false guards continuing to the next arm, UTF-8 file I/O with a 64 MiB read cap, read-only metadata probes with follow-symlink type predicates and `lstat` symlink detection, single-level directory operations with sorted listings and `R0015` failures, lexical glob expansion with hidden rules, symlink-safe `**`, byte-sorted deduplicated results, and `R0015`/`R0016` limits, recursive directory creation and removal with `R0015` failures, rename and byte-preserving file copy with `R0015` failures, new-target recursive directory copy with source containment, raw-name copying, inner-link and special-file rejection, and `R0015` caps, UTF-8 text builtins with Unicode `White_Space` trimming and `R0016` invalid-argument failures, and lexical POSIX-style path helpers with no filesystem access.

### IR lowering
Input: checked AST + `SemanticModel`.
Output: `ir::IrProgram` or `IrError`.
Guarantees: only statically representable constructs are lowered; `Any`, `List[Any]`, `Map[Any, *]`, `Any` enum payloads, `match` on `Any`, and unsupported operations fail with `E0401` and a help message. Types are explicit on every IR expression. `List[T]` and `Map[K, V]` are lowered for every statically known type argument, including nested collections and collections of structs. String indexing and iteration are lowered as `StrIndex`/`ForStr`; map iteration as `ForMap`; enum construction and `match` as `ConstructVariant`/`Match` with `IrPattern`; scalar literal patterns as `IrLiteral`; integer range patterns as `IrPattern::Range`; nested payload patterns recursively as `IrPattern::Variant`/`IrPattern::Bind` with absolute `IrFieldPath` bindings; top-level alternatives as `IrPattern::Or` with a validated shared binding signature; match guards as `IrMatchArm::guard`, validated to lower to `Bool`. No dynamic assumptions are inserted silently.

### C backend
Input: `IrProgram` + source name/source for positions.
Output: a self-contained GNU C11 translation unit.
Guarantees: left-to-right evaluation order, checked integer arithmetic, checked division/range operations, list/string bounds checks, missing-map-key errors, a 256-frame call-depth limit, and runtime errors matching interpreter codes. Lists use a stable header with a typed contiguous buffer and a generated element descriptor (ADR-0009). Maps use insertion-ordered parallel key/value buffers with generated key/value descriptors and an optional open-addressing index for scalar keys; structural keys keep linear search (ADR-0010, ADR-0016). Enums use a tagged struct with a union of per-variant payloads and generated display/equality callbacks (ADR-0012). Nested patterns emit short-circuit tag and literal tests; bindings are declared from recorded payload paths, and guards that read bindings use a GNU C statement expression (ADR-0020). Integer ranges emit `>=` plus `<` or `<=` comparisons (ADR-0022). Or-patterns emit a short-circuit alternative condition and initialize one shared binding set from the matched alternative (ADR-0021). Structural equality is cycle-safe. Lexical path helpers are small byte-scanned C functions with no filesystem calls (ADR-0030). Metadata probes use `stat`/`lstat` with POSIX type predicates (ADR-0032). Glob expansion uses `opendir`/`readdir`/`lstat`, a hand-written matcher over Unicode scalar values, byte sorting with `strcmp`, and the same hidden-entry, symlink, and limit rules as the interpreter (ADR-0033). Recursive removal recurses without following symbolic links and trims a trailing root separator before the link check (ADR-0031). Recursive copy resolves source and target parents with `realpath`, rejects a new target inside the source at a component boundary, walks with `opendir`/`readdir`/`lstat`, copies raw names and bytes, rejects inner symbolic links and special files, and enforces 100,000-entry and 128-depth caps (ADR-0034). Uses only libc/libm. Allocation failures terminate with a runtime diagnostic.

## Native subset boundary

Implemented: `Unit`, `Int`, `Float`, `Bool`, `Str`, `List[T]` and `Map[K, V]` for statically known type arguments, nominal structs, nominal enums, globals, functions, `if`/`while`/`for range`/`for list`/`for str`/`for map key`, `match` statements and expressions over closed enums with nested payload patterns, top-level or-patterns, and guards, string indexing, structural equality, UTF-8 file I/O with a 64 MiB read cap, metadata probes, single-level and recursive directory operations, lexical glob expansion, rename and byte-preserving file copy, recursive directory copy, UTF-8 text builtins, lexical path builtins, and the builtins listed in `PROJECT_STATUS.md`.

Rejected with `E0401`: `Any`, `List[Any]`, `Map[Any, *]`, `Any` enum payloads, `match` on `Any`, unsupported `for` iterables, unannotated value-returning functions, and dynamically typed file, directory, glob, recursive-copy, rename/copy, metadata, text, or path arguments. String index assignment is rejected by the checker with `E0306`. Unsupported programs must run with `vantyx run`.

This boundary is normative for v0.1: native builds must fail loudly rather than produce different behavior.

## Diagnostics

Codes are stable and grouped:
- `E0xxx` lexical/syntactic
- `E02xx` names/scopes
- `E03xx` types/declarations
- `E05xx` modules/imports (`E0501`–`E0512`)
- `R0xxx` runtime

Rendering includes file, line, column, source line, caret, and optional help. Multiple errors are collected where recovery is safe.

## Tooling boundary

`src/main.rs` owns CLI dispatch, file compilation, and native builds. `src/repl.rs` owns interactive input: raw-mode editing through the system `stty` with a canonical fallback, history, and completion (ADR-0013). The REPL compiles accumulated entries with `compile_with_options(source, false)` and executes them through the interpreter; it never invokes the native backend. Expression echo lives in `Interpreter::run_repl`, so each new expression is evaluated once and replayed entries stay muted. REPL meta-commands are handled before compilation and are not Vantyx syntax.

## Testing strategy

| Level | Location | Coverage |
| --- | --- | --- |
| Unit | inline `#[cfg(test)]` | lexer, parser, checker, values |
| CLI unit | `src/repl.rs` | completion, history deduplication, history file round-trip, permissions, cap |
| End-to-end | `tests/e2e.rs` | compile + execute + stdout/error assertions, REPL persistence/echo/history |
| Negative | `tests/e2e.rs` | malformed syntax, type errors, runtime errors |
| Regression | same suites | every fixed bug gets a test |
| Differential | `tests/native.rs` | interpreter vs native backend (69 cases, including imported modules) |
| Interactive smoke | manual pty via `script` | raw mode, Tab completion, history recall, Ctrl-C/Ctrl-D |

## Performance path

1. Interpreter with `Rc` values (current, semantic oracle).
2. Typed IR + C backend for the static subset (current, `-O2` via clang).
3. Resolve interpreter variables to slots instead of hash lookups.
4. Bytecode VM with constant pool and typed fast paths.
5. Escape analysis; unboxed Int/Float/Bool/struct locals.
6. Cranelift or LLVM if benchmarks justify the dependency (ADR-0008).

Optimizations are admitted only with correctness tests and measured impact.
