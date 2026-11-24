# Auren

**Script like Python. Ship native.**

Auren is a pre-1.0 programming language with Python-like indentation, optional
type annotations, a tree-walking interpreter, and a native GNU C11 backend for
the statically typed subset.

![status](https://img.shields.io/badge/status-pre--1.0-orange)
![license](https://img.shields.io/badge/license-MIT-blue)
[![CI](https://github.com/zeiad6/auren/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/zeiad6/auren/actions/workflows/ci.yml)

```auren
fn main():
    let name = "World"
    print("Hello, {name}!")
```

> **Status: functional prototype.** Auren is not production-ready. The native
> backend supports a subset, native allocations are intentionally leaked in
> v0.1, native maps are O(n), and there is no cycle collector or sandbox. See
> [PROJECT_STATUS.md](PROJECT_STATUS.md) for the verified state and exact limits.

## Why Auren

1. **Simple by default** — indentation blocks, `let`/`var`, optional type
   annotations, no required `main`.
2. **Safe by default** — checked arithmetic, bounds checks, no null, no
   uninitialized values, no user-visible `unsafe`.
3. **Gradual typing** — prototype without annotations; add them for
   compile-time guarantees and a native build.
4. **One semantics** — the interpreter is the oracle; native behavior is
   checked by differential tests. Unsupported constructs fail with `E0401`,
   never silently miscompile.
5. **Small and auditable** — the Rust compiler has zero third-party crates and
   the generated C links only libc/libm.

## Status

Auren is a **functional prototype**. The lexer, parser, gradual type checker,
tree-walking interpreter, typed IR, and native C11 backend are implemented and
tested. Closed enums with payloads, exhaustive `match`, scalar literal patterns
for `Int`/`Str`/`Bool`, and `Bool`-checked match guards work in both engines.
Native lists, insertion-ordered maps, structs, enums, and UTF-8-aware string
indexing/iteration are supported for statically typed code. The REPL persists
bindings and declarations and provides line editing, history, Tab completion,
and expression echo.

| Need | Auren today |
| --- | --- |
| A quick script with no type annotations | Yes, through the interpreter |
| Optional static types and clear diagnostics | Yes |
| A native executable | Yes, for the statically typed subset |
| Modules, generics, closures, async, FFI | Not yet |
| Production reliability or a stable ABI | No |
| Fast native maps or automatic reclamation | No |

A reproducible benchmark baseline guides optimization. Memory reclamation,
native map indexing, pattern-matching extensions, and optimization are the next
major gaps.

## Quick start

```sh
cargo build
./target/debug/auren run examples/hello.auren
./target/debug/auren check examples/hello.auren
./target/debug/auren build examples/hello.auren -o /tmp/hello
/tmp/hello
./target/debug/auren repl
```

Native builds require `clang` (override with `AUREN_CC`). Programs using `Any`,
`List[Any]`, maps whose key/value types cannot be inferred statically, `Any`
enum payloads, or `match` on `Any` must currently run with `auren run`. A match
expression must be the final element of its statement or arm; bind it to a
variable before using it inside a call or literal.

## REPL

```sh
./target/debug/auren repl
```

The REPL keeps bindings, functions, structs, enums, and mutable globals across
entries. End a multi-line block with a blank line. Expression statements print
their value once; `print` output is not duplicated.

- **Tab** completes keywords, builtins, and names declared at the top level.
- **Up/Down** recall history; **Ctrl-A/E**, **Ctrl-U/K/W**, and **Ctrl-L** edit.
- **Ctrl-C** cancels the current entry; **Ctrl-D** exits.
- **`:help`**, **`:history`**, **`:names`**, and **`:quit`** are REPL commands.

History is stored in `~/.auren_history` with mode 0600 on Unix. Set
`AUREN_HISTORY` to another path, or to `off` to disable persistence. Multi-line
blocks are kept for the session but are not recalled. Piped input and systems
without `stty` fall back to canonical line input. See
[ADR-0013](docs/decisions/ADR-0013-repl-line-editing.md).

## Benchmarks

```sh
cargo build --release
python3 benchmarks/run.py --reps 5
```

The runner checks that all implementations print identical output, then times
Auren (interpreter and native), Python, C, and Rust on six equivalent workloads.
On the 2026-11-22 single-machine baseline, the native backend was in the same
performance band as C/Rust on arithmetic, recursion, list, and struct workloads;
map operations and immutable string concatenation were the measured gaps. See
[docs/benchmarks.md](docs/benchmarks.md) for methodology, caveats, and raw
results under `benchmarks/results/`. These are baseline measurements, not a
general performance claim.

## Documentation

- [Language specification](docs/language_specification.md)
- [Architecture](docs/architecture.md)
- [Compiler architecture](docs/compiler_architecture.md)
- [Type system](docs/type_system.md)
- [Memory model](docs/memory_model.md)
- [Benchmarks](docs/benchmarks.md)
- [Roadmap](docs/roadmap.md)
- [Tasks](TASKS.md)
- [Project status](PROJECT_STATUS.md)
- [Launch and AI-discovery kit](docs/launch/)
- [`llms.txt`](llms.txt) — compact facts for AI assistants
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)

## Repository map

| Path | Purpose |
| --- | --- |
| `src/lexer.rs` | Indentation-aware lexer |
| `src/parser.rs` | Recursive-descent / Pratt parser |
| `src/checker.rs` | Name resolution and gradual type checking |
| `src/interp.rs` | Reference tree-walking runtime |
| `src/ir.rs` | Typed, backend-neutral IR |
| `src/codegen_c.rs` | GNU C11 native backend |
| `src/repl.rs` | Zero-dependency REPL editor, history, and completion |
| `src/diagnostic.rs` | Source spans and rendered diagnostics |
| `tests/e2e.rs` | End-to-end language tests |
| `tests/native.rs` | Differential interpreter vs native tests |
| `examples/` | Runnable programs, including enums and structs |
| `tools/fuzz_smoke.py` | Reproducible malformed-input and depth-limit fuzzing |
| `benchmarks/` | Equivalent Auren/Python/C/Rust workloads and the measurement runner |
| `docs/` | Specification, architecture, ADRs, research, launch kit |
| `.opencode/agents/` | Specialized engineering subagents |

## License

MIT. See [LICENSE](LICENSE).
