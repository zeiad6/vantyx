# ADR-0002: Indentation-based syntax with explicit keywords

- Status: Accepted
- Date: 2026-11-21

## Context
Vantyx targets Python-like approachability but must remain unambiguous and tool-friendly.

## Options
1. Indentation blocks (Python style).
2. Brace blocks (Rust/C style).
3. `end`-delimited blocks (Lua/Ruby style).
4. Significant indentation with optional braces (hybrid).

## Decision
Use indentation blocks with `:` headers. Reject tabs in indentation. Allow a single simple statement after `:` on the same line. Use `and`/`or`/`not` keywords rather than symbolic logical operators.

## Rationale
- Lowest visual and syntactic overhead for beginners.
- Enforced formatting reduces style disputes.
- Single canonical spelling simplifies the parser, formatter, and diagnostics.
- Tabs rejected avoids the largest class of indentation bugs.

## Consequences
- The lexer must emit INDENT/DEDENT and handle continuations inside brackets.
- Copy-pasted code with mixed indentation can fail; diagnostics must be precise.
- A formatter becomes important.

## Reversibility
Low. Changing block syntax would break every program. The decision is recorded in the specification and enforced by tests.

## Reconsideration criteria
Only if real user studies show indentation is a major adoption barrier, or if tooling cannot represent blocks reliably.
