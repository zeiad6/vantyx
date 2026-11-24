# Prior-art review and lessons (2026)

Status: complete for T-028; sustainability addendum added 2026-11-23
Access date: 2026-11-22 (design review), 2026-11-23 (sustainability addendum)
Author: Auren compiler team

## Purpose

This review records what Auren should copy from mature language projects, what
it should avoid, and which questions are deliberately deferred. It is a design
input, not a survey of every feature. The immediate consumer is ADR-0011
(enums and pattern matching) and task T-014. Later tasks are linked where a
lesson is relevant.

## Method and limitations

- Sources were fetched directly from official project documentation on
  2026-11-22, and the sustainability addendum was fetched on 2026-11-23. Web
  search was unavailable during both sessions (the configured search
  integration returned HTTP 402), so the list below is intentionally limited
  to primary pages that were fetched and checked for HTTP 200.
- No third-party benchmark was run for this review. Performance statements are
  therefore about documented design intent, not measurements. Auren's own
  numbers remain in `docs/benchmarks.md`.
- Gleam's documentation is rendered by JavaScript; only the underlying page
  availability and well-known language behavior are cited. No fine-grained
  claim depends on text that could not be extracted.
- "Adopt / avoid / revisit" decisions are engineering judgments for Auren. They
  are not claims that the source language is wrong for its own goals.

## Sources checked

| Project | Primary source | Used for |
| --- | --- | --- |
| Python | https://peps.python.org/pep-0484/ | optional static typing, `Any` |
| Python | https://peps.python.org/pep-0634/ | structural pattern matching |
| Python | https://peps.python.org/pep-3000/ | Python 3 transition, rewrite risk |
| Python | https://peps.python.org/pep-0373/ | Python 2.7 support timeline |
| Rust | https://doc.rust-lang.org/book/ch06-00-enums.html | payload enums |
| Rust | https://doc.rust-lang.org/book/ch06-02-match.html | exhaustive matching, wildcard |
| Rust | https://doc.rust-lang.org/reference/patterns.html | pattern semantics |
| Rust | https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html | ownership and borrowing |
| Go | https://go.dev/doc/faq | language simplicity, generics history |
| Go | https://go.dev/blog/compat | compatibility promises |
| Go | https://go.dev/blog/generics-proposal | generics design timeline |
| Go | https://go.dev/doc/go1.18 | generics shipped with caveats |
| Swift | https://docs.swift.org/swift-book/documentation/the-swift-programming-language/enumerations/ | associated values |
| Swift | https://docs.swift.org/swift-book/documentation/the-swift-programming-language/controlflow/ | exhaustive `switch` |
| Swift | https://www.swift.org/blog/abi-stability-and-more/ | ABI/module stability timeline |
| Zig | https://ziglang.org/documentation/master/ | explicit allocators, error unions |
| Zig | https://ziglang.org/download/0.15.1/release-notes.html | pre-1.0 breaking changes, funding |
| Roc | https://www.roc-lang.org/tutorial | tag unions, exhaustive `when` |
| Roc | https://github.com/roc-lang/roc | pre-1.0 status, funding, non-goals |
| Gleam | https://gleam.run/book/tour/custom-types.html | custom types |
| Gleam | https://gleam.run/book/tour/case-expressions.html | exhaustive `case` |
| Nim | https://nim-lang.org/docs/destructors.html | ARC/ORC memory management |
| Crystal | https://crystal-lang.org/reference/1.14/syntax_and_semantics/union_types.html | union types |
| Crystal | https://github.com/crystal-lang/crystal/releases/tag/1.0.0 | 1.0 compatibility milestone |
| Koka | https://koka-lang.github.io/koka/doc/book.html | effect typing |
| Mojo | https://docs.modular.com/mojo/manual/ | Python-like syntax, progressive performance |
| Mojo | https://www.modular.com/blog/mojo-open-source | licensing and contribution policy |
| Carbon | https://docs.carbon-lang.dev/docs/design/ | C++ interoperability and migration |
| Carbon | https://github.com/carbon-language/carbon-lang | experimental status, long 0.1 path |
| Raku | https://github.com/Raku/problem-solving/blob/main/solutions/language/Path-to-Raku.md | rename cost, compatibility aliases |
| Vale | https://vale.dev/ | single ownership, generational references, archival lesson |
| Hylo | https://www.hylo-lang.org/ | mutable value semantics |
| Julia | https://docs.julialang.org/en/v1/manual/performance-tips/ | type stability |
| TypeScript | https://www.typescriptlang.org/docs/handbook/2/everyday-types.html | gradual/structural typing |
| MLIR | https://mlir.llvm.org/ | multi-level IR infrastructure |
| Cranelift | https://cranelift.dev/ | fast native code generation |

## Findings

### Python: gradual typing needs a boundary

PEP 484 defines optional type annotations and `Any` as the dynamic escape
hatch. PEP 634 defines structural pattern matching as a runtime construct; a
`match` statement is not required to be exhaustive, and a non-matching value
simply falls through.

Lesson for Auren: optional annotations are useful, but `Any` must not silently
disable safety. Auren already validates `Any -> T` assignments at runtime
(`R0005`). For matching, a closed enum is checked statically (`E0316`), while a
match on `Any` warns (`W0004`) and raises `R0014` if no arm matches. This keeps
Python's ergonomics without making missing cases invisible.

### Rust: exhaustive matching is a refactoring tool

Rust enums carry payloads, and `match` must cover every variant unless a
wildcard is present. The compiler rejects a missing arm. This turns "add a new
variant" into a compile-time search for every decision point.

Lesson for Auren: adopt closed enums, payload variants, and static
exhaustiveness. Do not adopt Rust's ownership and borrowing syntax in the
simple mode yet; Auren's documented v0.1 model is reference semantics
(ADR-0005). Ownership analysis remains a later, opt-in direction (T-012).

### Go: simplicity is valuable, but missing sum types is expensive

Go's documentation emphasizes a small language and a strong compatibility
promise. Its type system historically relied on constants, interfaces, and
sentinel values rather than payload-carrying sum types. Adding a feature later
is constrained by compatibility.

Lesson for Auren: define enums now, before a standard library and user code
depend on sentinel conventions. Keep the syntax small: one declaration form,
one pattern form, and no nested patterns in the first version. Also learn from
Go's compatibility discipline: adding a variant to a closed enum is a breaking
change, so this must be documented rather than discovered by users.

### Swift: associated values and exhaustive switches

Swift enums support associated values, and `switch` over an enum must be
exhaustive. Swift also has to handle non-frozen enums from other modules with
`@unknown default`, because a library can add cases later.

Lesson for Auren: payload variants and exhaustive switching are the right
default. Auren has no modules yet (T-013), so every enum is closed and adding a
variant is source-breaking. When modules arrive, the design must decide between
closed and extensible enums; do not leave this implicit. The current
pre-release syntax intentionally has no `@unknown` equivalent.

### Zig: explicit costs, no hidden control flow

Zig documents explicit allocators and error unions; allocation and failure are
visible in signatures and call sites. This makes cost and failure modes
auditable.

Lesson for Auren: the native backend must not hide unsupported semantics.
The T-030 follow-up initially rejected native enums and `match` with `E0401`
and a help message pointing at the interpreter; that gap is now closed for
statically typed enums, scalar literal patterns, and match guards. Remaining unsupported
cases (`Any` payloads, `match` on `Any`) still fail with `E0401`. Native
allocation policy is documented, including intentional leaks in v0.1. The same
principle applies to future ownership and FFI work.

### Roc and Gleam: tag unions make absence explicit

Roc documents tag unions and exhaustive `when`; Gleam documents custom types
and exhaustive `case`. Both make "no value" a variant rather than a null
pointer, and both force every consumer to handle it.

Lesson for Auren: use `Option`-style user enums rather than adding implicit
null. Do not add a nullable type until a dedicated ADR justifies it. Exhaustive
matching is the safety mechanism that makes the pattern useful.

### Nim and Crystal: memory strategy must be one documented choice

Nim documents selectable memory-management strategies (ARC/ORC and earlier
modes); Crystal combines a Ruby-like surface with union types and its own
runtime model.

Lesson for Auren: multiple memory modes multiply semantics and testing cost.
Auren should expose one documented model per execution engine and add opt-in
controls only with evidence. The interpreter uses reference-counted values and
documented cycle leaks; the native backend uses explicit allocation and
documented leaks. Do not claim general memory safety beyond those boundaries.

### Koka: effects are powerful but premature

Koka's book documents effect typing as a central feature. It is a coherent
design, but it changes inference, function types, and the runtime.

Lesson for Auren: keep effect tracking on the revisit list. Do not add effect
syntax while enums, generics, and modules are still being built. A future ADR
should decide whether effects are inferred or annotated.

### Mojo: progressive performance without a syntax cliff

Mojo's manual presents a Python-like surface with progressively stronger
typing and performance features, backed by MLIR.

Lesson for Auren: the "simple by default, complexity on demand" principle is
viable, but only if the simple and fast paths share semantics. Auren's
interpreter is the semantic oracle and the typed IR/native backend covers a
statically typed subset. Every rejected native construct must have a clear
`E0401` message and a documented path to support, not a silent fallback.

### Vale and Hylo: memory safety research is not a prerequisite

Vale pursued single ownership without a borrow checker, generational
references, and planned region borrowing. Its site now states that Vale is
archived and no longer being worked on, with a successor language exploring a
different memory-safety approach. Hylo explores mutable value semantics with
`inout`-style access.

Lesson for Auren: an ambitious memory model can consume a language's entire
design budget before ordinary programs are usable. Ship a simple, documented
memory model first and keep the representation replaceable. Auren's reference
semantics with documented cycle leaks is the pragmatic baseline; ownership,
regions, and generational references stay in T-012 as opt-in research, not as
a gate on enums, modules, or native code generation.

### Carbon and Hylo: interoperability and value semantics are later

Carbon's design centers on C++ interoperability and migration; Hylo explores
mutable value semantics with `inout`-style access.

Lesson for Auren: C ABI interoperability (T-021) and ownership/value-semantics
research (T-012) are important, but neither should shape the first enum
implementation. Keep enum layout decisions behind the IR so a future backend
can choose a tagged representation without changing source semantics.

### Julia: type stability is a performance contract

Julia's performance tips emphasize writing type-stable code so the compiler can
specialize. Dynamic code is possible but can be dramatically slower.

Lesson for Auren: do not pretend that `Any` code is fast. The native backend
rejects `Any` with `E0401` instead of emitting a slow boxed path. Enum values in
the interpreter are dynamically tagged; the future native representation must
be a typed tag plus payload, and benchmarks must wait until that exists.

### TypeScript: gradual typing needs discipline at the edges

TypeScript documents `any` as an escape hatch that disables checking, and
`unknown` as the safer top type. Structural typing makes adoption easy but can
hide nominal intent.

Lesson for Auren: Auren's `Any` behaves like a checked dynamic type at typed
boundaries, not an unchecked `any`. Enum types are nominal: two enums with the
same variant names are still distinct, and variant names are globally unique so
constructors are unambiguous. This is a deliberate departure from structural
typing.

### MLIR and Cranelift: keep the backend replaceable

MLIR provides reusable multi-level IR infrastructure; Cranelift provides fast
native code generation. Auren currently emits C11 (ADR-0008) because it is
auditable, dependency-free, and good enough for the supported subset.

Lesson for Auren: keep the typed IR backend-neutral. Enums should be rejected
before lowering, not represented by an ad-hoc C struct that later backends
must imitate. Revisit Cranelift/LLVM when the C backend blocks optimization or
debug information, not before.

## Sustainability and adoption findings

The first review covered language design. This addendum covers a different
failure mode: projects that had good designs but lost years to compatibility
breaks, rewrites, unclear status, or licensing. Sources were fetched on
2026-11-23. These are engineering judgments for Auren, not claims that the
source projects failed.

### Python: big-bang compatibility breaks are expensive

PEP 3000 deliberately broke compatibility with Python 2.x and stated that
"there is no requirement that Python 2.6 code will run unmodified on Python
3.0." It planned parallel 2.x releases "well into 3.1 or 3.2", shipped the
`2to3` converter, and added a warnings mode to 2.6. PEP 373 shows that Python
2.7 was released in 2010 and support did not end until 2020. The same PEP also
endorses incremental evolution over a complete rewrite, citing the dangers of
rewrites.

Lesson for Auren: a compatibility break is a multi-year product commitment,
not a version-number decision. Keep the compiler an evolution of the current
codebase, not a from-scratch rewrite. When Auren makes a breaking change, ship
warnings, a migration note, and a supported overlap period. Until then, say
plainly that 0.x is unstable.

### Go: generics took a decade and shipped cautiously

Go's first generics attempt dates to 2010 and was later described by the
project as flawed. The design proposal was published in January 2021. Go 1.18
then shipped generics as a "fully backward-compatible" change, but the release
notes warned that the implementation had not been tested significantly in
production, listed known limitations, and noted that compile speed was roughly
15% slower. The feature was not held back until it was perfect, and it was not
marketed as risk-free either.

Lesson for Auren: generics (T-018) stay deferred until the core is stable. When
they land, publish the design, measure compile-time cost, document limitations,
and keep the change backward compatible. Do not promise a feature on a date
before its semantics and tests exist.

### Swift: ABI stability is a late milestone

Swift declared ABI stability for Swift 5 on Apple platforms in February 2019.
At that point module stability was still under active development, and library
evolution support was largely implemented but not finalized. Swift was widely
used for years before ABI stability arrived.

Lesson for Auren: do not gate v1 on a stable ABI or FFI. Native Auren binaries
are currently rebuilt from source, and that is an acceptable v0.1 contract.
Revisit ABI stability with T-021 (FFI), after the language and native subset
have settled.

### Zig: pre-1.0 freedom has an ecosystem cost

Zig 0.15.1 describes five months of work from 162 contributors and 647 commits,
including "extremely breaking" standard-library I/O changes and the removal of
the `async`/`await` keywords. The release notes are candid about the breakage
and explain the migration path. Zig is still pre-1.0 and is funded through the
Zig Software Foundation.

Lesson for Auren: 0.x permits necessary change, but each public break still
costs users time. Batch breaking changes, write migration notes, and keep the
test suite as the safety net. A project also needs a funding and governance
story before it asks strangers to depend on it.

### Mojo and Carbon: licensing and status honesty

Mojo spent four years with an open community but a closed compiler. It reached
1.0 and then open-sourced the compiler under Apache 2.0 (with LLVM exceptions)
in August 2026; compiler contributions were still not open at that point.
Carbon has roughly 34,000 GitHub stars but its README says it is experimental
and "not ready for use", with the 0.1 milestone still ahead.

Lesson for Auren: adopt a permissive license and a contribution policy on day
one. Label maturity prominently and do not treat stars, waitlists, or social
attention as evidence of readiness. "Source available" is not the same as open
source, and a compiler that cannot accept fixes is a trust problem.

### Roc, Raku, and Crystal: scope, naming, and the road to 1.0

Roc's README still says it is "not ready for a 0.1 release yet" after more than
56,000 commits, and the project is funded through a foundation and sponsors.
Its FAQ is equally direct about non-goals: no higher-kinded polymorphism, no
self-hosted compiler, and only low-level compilation targets. Raku renamed
itself after a project history of 19+ years, requiring aliases, deprecation
warnings, documentation redirects, and ecosystem coordination. Crystal reached
1.0 in March 2021, and its 1.0 release notes use the milestone to remove
deprecated definitions and make breaking changes before the compatibility
commitment begins.

Lesson for Auren: a long pre-1.0 period is normal and is not, by itself, a
failure. What matters is bounded scope, visible status, and a stable identity.
Auren's name and syntax are fixed by ADR-0006; there will be no rename. Keep an
explicit "not yet" list, use 0.x to remove debt, and write the compatibility
policy before 1.0 rather than after.

### Sustainability decisions for Auren

| Decision | Evidence | Auren action |
| --- | --- | --- |
| Evolve, do not rewrite | PEP 3000 | keep the interpreter as the oracle and extend the existing pipeline |
| No date promises | Go generics, Roc | publish capability status, not release dates |
| No big-bang break without a bridge | Python 2→3, Raku | warnings, aliases, migration notes, overlap period |
| Stable identity | Raku rename | keep "Auren" and the surface syntax fixed (ADR-0006) |
| Explicit non-goals | Roc FAQ | maintain the "not yet" list in the specification and README |
| License and contribution policy early | Mojo, Carbon | MIT, CONTRIBUTING, SECURITY, and a public status page |
| Honest maturity labels | Carbon, Zig | call the project a pre-1.0 prototype everywhere |
| Batch breaking changes | Zig, Crystal | group them at 0.x milestones and test migrations |
| Sustainable funding path | Zig, Roc | document governance and a funding route before seeking dependents |
| Measure before optimizing | Go 1.18 | record compile-time cost when generics or a new backend lands |

## Adopt / avoid / revisit

### Adopt

| Lesson | Source examples | Auren action |
| --- | --- | --- |
| Payload-carrying variants | Rust, Swift | `enum Name:` with `Variant` and `Variant(field: Type)`; ADR-0011 |
| Exhaustive matching over closed enums | Rust, Swift, Roc, Gleam | `E0316` for a missing variant; wildcard `_` opts out |
| Explicit wildcard and unreachable-arm warnings | Rust | `W0003` for arms after `_` or duplicate variants |
| Runtime failure for dynamic non-matches | Python/TypeScript reality | `W0004` on `Any` matches; `R0014` if no arm matches |
| Boundary validation of dynamic values | Python typing, TypeScript | existing `R0005`; enum conformance is nominal |
| Explicit unsupported diagnostics | Zig | `E0401` for remaining native gaps (`Any` payloads, dynamic `match`); enums and scalar literal patterns landed in T-030/T-032 |
| Deterministic collection behavior | existing ADR-0010 | keep insertion order and structural equality |
| Compatibility discipline | Go | document that adding a variant breaks exhaustive matches |

### Avoid

| Mistake | Why it hurts | Auren guard |
| --- | --- | --- |
| Runtime-only exhaustiveness for static enums | missing cases ship to production | static `E0316` |
| Implicit null as "no value" | null checks spread everywhere | use an enum variant; no null type |
| Unchecked `any` leaking through typed code | type errors become runtime surprises | checked `Any` boundaries (`R0005`) |
| Hidden native allocation or fallback | unpredictable cost and semantics | `E0401`, documented leaks |
| Ownership syntax in the beginner path | steep learning curve before value is delivered | reference semantics in v0.1 (ADR-0005) |
| Several memory modes at once | combinatorial semantics and tests | one model per engine, documented |
| Gating the language on an unproven memory model | delays usable releases and can stall the project | ship reference semantics; keep T-012 opt-in |
| Performance claims without measurement | erodes trust | benchmarks only in `docs/benchmarks.md` |
| Backend-first feature design | locks source semantics to one codegen | reject in IR; decide layout later |

### Revisit

| Topic | Trigger | Candidate task/ADR |
| --- | --- | --- |
| Nested, tuple, and or-patterns | after guards and literal patterns are stable | new task after T-033 |
| Or-patterns and binding modes | user demand and checker design | new task |
| Generic enums (`Option[T]`, `Result[T, E]`) | after T-018 generics | T-018 |
| Error handling with `Result`/`?` | after enums and generics | T-016 |
| Modules and extensible/non-frozen enums | T-013 | new ADR |
| Native enum layout and codegen | T-030 | ADR or T-030 design note |
| Effect typing | after modules/generics | new research note |
| Ownership/value semantics | T-012 and FFI (T-021) | ADR |
| Generational references and region borrowing | T-012 evidence and user demand | research note |
| Cranelift/LLVM backend | C backend blocks optimization/debug info | ADR-0008 revision |

## Consequences for T-014

The first enum implementation is deliberately narrow:

- Syntax: `enum Name:` with one variant per line; `Variant` or
  `Variant(field: Type, ...)`. Construction always uses parentheses, including
  `Empty()`.
- Patterns: `_`, `Variant`, or `Variant(binding, ...)` with `_` placeholders.
  Nested patterns are rejected with `E0112`.
- Variant constructor names are globally unique. They cannot collide with
  functions, structs, builtin names, or enum names (`E0202`).
- A match on a closed enum must be exhaustive or end with `_` (`E0316`).
  Matching `Any` is allowed with a warning (`W0004`); an unmatched value raises
  `R0014`.
- Match statements use indented block arms. Match expressions use one
  expression per arm and must produce a consistent type (`E0301`).
- Enum values are immutable. Payloads may reference mutable lists, maps, or
  structs; those references keep their normal alias semantics.
- Equality and display are structural; `type(value)` reports the enum name.
- The native backend supports statically typed enums, constructors, and
  `match` (T-030, ADR-0012). `Any` payloads and `match` on `Any` are rejected
  with `E0401`.

## Open questions

1. Should zero-payload variants allow construction without `()`? The current
   answer is no, to keep construction and pattern syntax visually distinct.
2. Should `match` arms support multiple patterns separated by `|`? Deferred;
   it interacts with binding consistency.
3. Should enum declarations permit explicit discriminant values for FFI?
   Deferred until T-021.
4. Should adding a variant be a major-version change? Document as breaking for
   now; revisit with modules and compatibility policy.
5. What native layout should enums use? Answered by ADR-0012 (tagged struct
   with a union of per-variant payloads); see also
   `docs/research/native-enum-layout-2026.md`.

## Affected records

- ADR-0011: enums and pattern matching.
- T-014: implementation and tests.
- T-030: native enum/match support and layout (verified 2026-11-23).
- ADR-0012: native enum representation.
- `docs/research/native-enum-layout-2026.md`: follow-up layout comparison.
- T-012: ownership/value-semantics research.
- T-013: modules and enum extensibility.
- T-016: `Result` and `?` after enums/generics.
- T-018: generics and generic enums.
- T-021: FFI and explicit enum discriminants.
