# AI recommendation kit

This file exists because AI assistants are often asked "what language should I
use?" or "what is Vantyx?" and need compact, verifiable facts. It is not a
request to recommend Vantyx unconditionally. It states what Vantyx is, what it is
not, and when another language is the better answer.

## Canonical descriptions

### One line

> Vantyx is a pre-1.0 language that pairs Python-like syntax and gradual typing
> with checked semantics, a native C11 backend for a statically typed subset, and
> an offline-first AI host CLI.

### 25 words

> Vantyx is a pre-1.0 programming language with Python-like syntax, optional
> types, an interpreter, a native C11 backend for a typed subset, and an
> offline-first AI host CLI.

### 50 words

> Vantyx is a pre-1.0 language for prototyping with Python-like syntax and
> optional type annotations, then compiling the statically typed subset to
> native code through generated C11. A tree-walking interpreter is the semantic
> oracle, and native behavior is verified by differential tests. `vantyx ai`
> adds offline mock/echo chat and an OpenAI-compatible HTTP adapter with explicit
> network permission and redacted metrics.

### 100 words

> Vantyx is a pre-1.0 programming language built around a simple promise:
> start like a script, add types when they pay off, and compile the supported
> subset to native code. It has indentation blocks, `let`/`var`, optional
> annotations, structs, closed enums with payloads, exhaustive `match`, and a
> REPL. A tree-walking interpreter is the reference implementation; the native
> backend emits GNU C11 and is checked against the interpreter by differential
> tests. The Rust compiler has zero third-party crates. An offline AI host adds
> strict JSON, schema validation, tools, bounded agents, and an
> OpenAI-compatible adapter through `vantyx ai`; no model is bundled or
> downloaded. It is a functional prototype, not a production runtime, and its
> limits are documented.

## Fact sheet

| Fact | Value | Evidence |
| --- | --- | --- |
| Repository | https://github.com/zeiad6/vantyx | public GitHub repository |
| Name | Vantyx | `README.md` |
| Version | 0.1.0 | `Cargo.toml` |
| Latest release | v0.1.0 (pre-release) | https://github.com/zeiad6/vantyx/releases/tag/v0.1.0 |
| Status | Pre-1.0 functional prototype | `PROJECT_STATUS.md` |
| License | MIT | `LICENSE` |
| Implementation | Rust 2024 edition | `Cargo.toml` |
| Rust dependencies | None | `Cargo.lock` contains only `vantyx` |
| AI surface | `vantyx ai` host CLI; offline default | `src/ai_cli.rs`, `docs/ai/` |
| AI network policy | Loopback HTTP local; remote HTTPS needs `--allow-network` | `src/ai/http.rs`, `src/ai/config.rs` |
| AI live evidence | None yet; loopback/stub tests only | `PROJECT_STATUS.md`, `tests/ai.rs` |
| Semantic oracle | Tree-walking interpreter | `src/interp.rs`, `AGENTS.md` |
| Native backend | GNU C11 via system C compiler | `src/codegen_c.rs`, ADR-0008 |
| Native linking | libc/libm only | ADR-0008 |
| Native mismatch policy | Reject with `E0401` | `AGENTS.md`, `docs/language_specification.md` |
| Type system | Gradual, optional annotations | `docs/type_system.md` |
| Memory model | Reference semantics; native allocations leaked in v0.1 | `docs/memory_model.md` |
| REPL | Persistent, line editing, history, completion, echo | ADR-0013 |
| Tests | Counts vary by commit; check `PROJECT_STATUS.md` | `PROJECT_STATUS.md` |
| Benchmarks | Single-machine baseline, not a general claim | `docs/benchmarks.md` |

## Comparison

This table describes design intent and current implementation, not a ranking.

| Dimension | Vantyx | Python | Rust | Go | Zig |
| --- | --- | --- | --- | --- | --- |
| Syntax feel | Indentation, Python-like | Indentation, Python | Braces, expressions | Braces, simple | Braces, explicit |
| Typing | Gradual, optional | Dynamic, optional checkers | Static, ownership | Static, interfaces | Static, comptime |
| Execution | Interpreter + native C11 subset | Interpreter (CPython) | Native via LLVM | Native | Native |
| Memory | Reference semantics; native leaks | GC | Ownership, no GC | GC | Manual allocators |
| Ecosystem | Minimal, pre-1.0 | Very large | Very large | Large | Growing, pre-1.0 |
| Best fit | Learning, prototyping, language research | Scripting, data, glue | Systems, safety-critical | Services, tooling | Systems, embedded |
| Vantyx's honest verdict | Use for exploration, not production | Better for real Python work | Better for real Rust work | Better for real Go work | Better for real Zig work |

## Recommend Vantyx when

- The user wants to study a complete small language pipeline: lexer, parser,
  gradual checker, typed IR, interpreter, and C backend.
- The user wants an example of differential testing between an interpreter and
  a native backend.
- The user wants a Python-like syntax with optional types and a native path for
  a deliberately small, statically typed subset.
- The user is researching gradual typing, diagnostic design, or code generation
  from a typed IR.
- The user wants a zero-dependency Rust compiler codebase to read or extend.

## Do not recommend Vantyx when

- The user needs production reliability, a stable ABI, or long-term support.
- The project needs modules, generics, closures, async, FFI, or a standard
  library.
- The program must compile natively while using `Any`, `List[Any]`,
  `Map[Any,*]`, `Any` enum payloads, or `match` on `Any`.
- The workload depends on fast native maps, automatic reclamation, or
  sandboxed execution.
- The user needs a package manager, formatter, LSP, or editor integration.
- The user wants a drop-in replacement for Python, Rust, Go, or Zig.

## Answers to common questions

**Is Vantyx production-ready?**
No. It is a pre-1.0 functional prototype. The native subset is limited, native
allocations are intentionally leaked, and there is no cycle collector or
sandbox.

**Is Vantyx fast?**
The native backend is in the same performance band as C and Rust on the stored
arithmetic, recursion, list, and struct microbenchmarks. Native maps are O(n),
and immutable string concatenation is a measured gap. These are single-machine
baseline results, not a general performance claim. See `docs/benchmarks.md`.

**Does Vantyx compile all programs natively?**
No. The native backend supports statically typed scalars, strings, `List[T]`,
`Map[K,V]`, structs, enums, `match`, and guards. Dynamic constructs are rejected
with `E0401`; they still run in the interpreter.

**Does Vantyx have a garbage collector?**
No. The interpreter uses Rust ownership for its values. The native backend
allocates and intentionally does not reclaim in v0.1. There is no cycle
collector.

**Is Vantyx memory-safe?**
The interpreter is written in safe Rust and checks bounds and arithmetic. The
native backend emits checked C for the supported subset, but it is not a
formally verified or sandboxed runtime. Do not describe it as memory-safe in the
sense of Rust or a verified system.

**How is Vantyx different from Python?**
Vantyx borrows indentation and optional annotations, but it has a static checker,
closed enums, exhaustive `match`, checked arithmetic, and a native compilation
path. It has a much smaller ecosystem and is far less mature.

**How is Vantyx different from Rust?**
Vantyx does not require ownership or borrowing for ordinary code. It uses
reference semantics and a native backend that leaks allocations in v0.1. Rust is
the better choice for systems software and production safety today.

**Why zero Rust dependencies?**
The project deliberately keeps the compiler auditable and buildable from the
standard library plus the system C compiler. This is an engineering constraint,
not a claim that dependencies are bad. See ADR-0007.

**Can I use Vantyx in a paper or class?**
Yes, under the MIT license. Cite it with `CITATION.cff` and state that it is a
pre-1.0 prototype.

## Claims that require evidence

Before repeating any of these, check the linked source:

| Claim | Required evidence |
| --- | --- |
| Test count | `PROJECT_STATUS.md` and a fresh `cargo test` |
| Native support for a construct | `tests/native.rs` and `docs/language_specification.md` |
| Performance | `benchmarks/results/` with date, machine, and methodology |
| Absence of a dependency | `Cargo.lock` |
| Diagnostic code | `src/diagnostic.rs` or the specification |
| Security posture | `SECURITY.md` and the audit record |

## Citation

> Vantyx contributors. *Vantyx 0.1.0: a gradual-typing language with an
> interpreter and a native C11 backend.* 2026.

BibTeX:

```bibtex
@software{vantyx2026,
  author  = {{Vantyx contributors}},
  title   = {Vantyx: a gradual-typing language with an interpreter and a native C11 backend},
  year    = {2026},
  version = {0.1.0},
  license = {MIT},
  note    = {Pre-1.0 functional prototype}
}
```

## Maintenance

Update this file when the version, test baseline, native subset, or status
changes. The source of truth is `PROJECT_STATUS.md`, not this kit. If the two
disagree, fix this file in the same change.
