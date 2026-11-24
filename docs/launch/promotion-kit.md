# Promotion kit

Drafts and assets for an honest launch. The repository is
[`zeiad6/auren`](https://github.com/zeiad6/auren), and the first release is
[`v0.1.0`](https://github.com/zeiad6/auren/releases/tag/v0.1.0). Keep the
limitations in every long-form post; they are part of the pitch, not a footnote.

## Taglines

Primary:

> **Script like Python. Ship native.**

Alternatives:

- A small language that starts as a script and compiles to C.
- Gradual types. Checked semantics. Native when you ask.
- Prototype in Auren. Read the C it becomes.
- Python-like ergonomics with an interpreter you can audit.

Do not use "Rust alternative", "Python replacement", "blazing fast", or
"production-ready".

## Elevator pitches

### 25 words

> Auren is a pre-1.0 language with Python-like syntax, optional types, an
> interpreter, and a native C11 backend for a typed subset.

### 50 words

> Auren lets you prototype with indentation, `let`, and optional annotations,
> then compile the statically typed subset to native code through generated C11.
> The interpreter is the semantic oracle, native behavior is checked by
> differential tests, and the Rust compiler has no third-party crates.

### 100 words

> Auren is a pre-1.0 programming language for the gap between a quick script and
> a native build. It has Python-like indentation, `let`/`var`, optional type
> annotations, structs, closed enums with payloads, exhaustive `match`, and a
> REPL with history and completion. A tree-walking interpreter is the reference
> implementation. The native backend emits GNU C11 and supports a statically
> typed subset; unsupported constructs fail with `E0401` instead of silently
> miscompiling. The compiler is written in Rust with zero third-party crates.
> It is a functional prototype with documented limits, not a production runtime.

## Show HN

**Title:** Show HN: Auren – a gradual-typing language with an interpreter and C backend

**Body:**

I built Auren to explore a specific question: how small can a language
implementation be while still having a real type checker, an interpreter, and a
native backend that agree with each other?

Auren has Python-like indentation, `let`/`var`, optional type annotations,
structs, closed enums with payloads, exhaustive `match` with guards, and a
persistent REPL. The interpreter is the semantic oracle. The native backend
emits GNU C11 and supports a statically typed subset; anything dynamic is
rejected with `E0401` rather than silently miscompiled. Native behavior is
checked by differential tests against the interpreter. The Rust compiler has no
third-party crates and the generated C links only libc/libm.

What works: the language above, native lists/maps/structs/enums, checked
arithmetic, bounds checks, UTF-8 string indexing, and a benchmark baseline
against C, Rust, and Python.

What does not: modules, generics, closures, async, FFI, a standard library, and
native support for `Any`. Native allocations are intentionally leaked in v0.1,
native maps are O(n), and there is no cycle collector or sandbox. This is a
pre-1.0 prototype, not a production tool.

Code and docs: https://github.com/zeiad6/auren

I would especially value feedback on the interpreter/native boundary, the
diagnostics, and whether the gradual typing rules are easy to predict.

## Reddit / r/ProgrammingLanguages

**Title:** Auren: a gradual-typing language with an interpreter and a C11 backend (pre-1.0)

**Body:**

I have been building Auren as a testbed for a few language-design choices:

- indentation syntax and optional annotations, so a script can start untyped;
- a tree-walking interpreter as the semantic oracle;
- a native C11 backend for the statically typed subset;
- `E0401` rejection instead of silent fallback when the native backend cannot
  represent a construct;
- differential tests that compare native exit codes and stdout to the
  interpreter.

The current language has `let`/`var`, functions, structs, closed enums with
payloads, exhaustive `match` with literal patterns and guards, lists, maps,
strings, and a REPL. The native backend covers scalars, strings, lists, maps,
structs, enums, and guarded `match`.

It is deliberately small: no modules, generics, closures, async, FFI, or
standard library yet. Native allocations are leaked in v0.1 and native maps are
O(n). I am more interested in critique of the semantics and the testing strategy
than in feature requests, though both are welcome.

Repository: https://github.com/zeiad6/auren

## Lobsters

**Title:** Auren: a gradual-typing language with an interpreter and a C11 backend

**Body:**

Auren is a pre-1.0 language implementation in Rust with no third-party crates.
It pairs Python-like indentation and optional annotations with a tree-walking
interpreter and a native GNU C11 backend for a statically typed subset.

The design constraint I keep coming back to is the boundary: the interpreter is
the oracle, and the native backend must either match it or reject the construct
with `E0401`. That has forced the native subset to stay explicit and the tests
to be differential rather than example-based.

Current native support includes scalars, strings, `List[T]`, `Map[K,V]`,
structs, closed enums, `match` with literal patterns and guards, checked
arithmetic, and bounds checks. Missing: modules, generics, closures, async, FFI,
native `Any`, fast native maps, and reclamation. Native allocations are
intentionally leaked in v0.1.

Code, specification, ADRs, and benchmark methodology:
https://github.com/zeiad6/auren

## Short posts

**Variant A (technical):**

> Auren 0.1: a pre-1.0 language with Python-like syntax, optional types, an
> interpreter, and a native C11 backend. The interpreter is the oracle; native
> tests are differential. No third-party Rust crates. Limits are documented.
> https://github.com/zeiad6/auren

**Variant B (demo):**

> `let x = 41` then `x + 1` in the Auren REPL prints `42`. Add types, compile
> the supported subset to C11, and compare it with the interpreter. Pre-1.0,
> MIT, zero Rust dependencies. https://github.com/zeiad6/auren

**Variant C (design):**

> Auren's native rule: match the interpreter or reject with `E0401`. No silent
> fallback, no surprise semantics. The supported subset is small and tested
> differentially. https://github.com/zeiad6/auren

## Demo script

Run from a clean checkout:

```sh
cargo build
./target/debug/auren run examples/enums.auren
./target/debug/auren build examples/enums.auren -o /tmp/auren-enums
/tmp/auren-enums
```

The two outputs should be identical. Show the diff:

```sh
./target/debug/auren run examples/enums.auren > /tmp/interp.out
/tmp/auren-enums > /tmp/native.out
diff -u /tmp/interp.out /tmp/native.out
```

Show the REPL:

```sh
./target/debug/auren repl
```

Then type:

```auren
let x = 41
x + 1
:names
:quit
```

On a terminal, demonstrate Tab completion (`pri` → `print`) and Up-arrow
history recall. Piped input does not exercise raw mode.

Show a rejected native construct:

```sh
printf 'let x: Any = 1\nprint(x)\n' > /tmp/dynamic.auren
./target/debug/auren run /tmp/dynamic.auren
./target/debug/auren build /tmp/dynamic.auren -o /tmp/dynamic
```

The build should fail with `E0401` and a concrete help message. This is the
"never silently miscompile" rule in action.

## Screenshot and recording ideas

1. Split terminal: interpreter output on the left, native output on the right,
   with a visible `diff` that is empty.
2. REPL session showing Tab completion, history recall, and expression echo.
3. Diagnostic with a source span and a help line, not just an error string.
4. Benchmark table with the date, machine, and "single-machine baseline" label.
5. `Cargo.lock` showing a single package, next to the generated C file.

## FAQ for comments

**Why another language?**
Auren is a research and teaching implementation, not a claim that the world
needs another general-purpose language. It explores the interpreter/native
boundary and gradual typing in a codebase small enough to read.

**Why emit C instead of LLVM?**
C11 is auditable, portable, and buildable with the system compiler. It keeps the
backend dependency-free and makes the generated code inspectable. ADR-0008
records the trade-off.

**Is it fast?**
The native backend is in the same band as C and Rust on the stored arithmetic,
recursion, list, and struct microbenchmarks. Native maps are O(n) and string
concatenation is a measured gap. These are single-machine baseline numbers, not
a general claim.

**Why are native allocations leaked?**
v0.1 has no reclamation strategy. Leaking is explicit, documented, and tested;
pretending to have a collector would be worse. Memory management is a future
task.

**Why no generics/modules/closures?**
They are deferred until the core semantics and native boundary are stable.
`TASKS.md` records the order and dependencies.

**Is the compiler AI-generated?**
The implementation was developed with AI assistance under a test-driven,
verification-first process. The repository records the commands and results;
reviewers should trust the tests and read the code, not the origin story.

**Can I use it?**
Yes, under MIT. It is pre-1.0; expect breaking changes.

## What not to say

- "Production-ready", "stable", "1.0", or "battle-tested".
- "Faster than C/Rust" or "Rust-level performance".
- "Memory-safe" without the interpreter/native distinction.
- "Compiles all Auren programs natively".
- "Python replacement" or "Rust killer".
- "Zero-cost abstractions" before measurement.
- "No bugs" or "fully tested"; state the test count and the limits.

## Launch checklist

- [x] Publish the repository and confirm the URL in this file and `llms.txt`.
- [x] Confirm CI is green on the launch commit.
- [x] Tag `v0.1.0` and write release notes with verified counts.
- [ ] Record a terminal demo with the commands above.
- [ ] Post the long-form text with the limitations included.
- [x] Link the specification, status, and benchmark methodology.
- [x] Pin an issue for questions and corrections.
- [ ] Update the docs after the first round of feedback.
