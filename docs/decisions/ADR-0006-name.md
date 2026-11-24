# ADR-0006: Language name — Auren

- Status: Superseded by [ADR-0014](ADR-0014-rename-vantyx.md)
- Date: 2026-11-21
- Note: The project was renamed to **Vantyx** on 2026-11-23. This ADR is kept
  as history; see ADR-0014 and `RESEARCH/NAMING_AUDIT.md` for the decision and
  the migration mapping.

## Context
The language needs an identity distinct from Python and Rust. Candidate names were checked for collisions with existing programming languages.

## Candidates
- Kestrel — rejected: multiple active languages, including one with the same "memory-safe compiled language" pitch.
- Vesper — rejected: active systems language and other projects.
- Aurel — rejected: existing BASIC dialect.
- Auren — selected.

## Decision
Name the language **Auren**, extension `.auren`, CLI `auren`. Repository codename remains RustZ.

## Rationale
- No known programming-language collision at time of check.
- Short, pronounceable, distinctive.
- Suggests "aurum/aurora" (gold/dawn), matching the goal of a durable, clear language.

## Consequences
- Unrelated companies use the name; uniqueness is not trademark-cleared.
- Renaming later would be costly; do it before public releases if necessary.

## Reconsideration criteria
Before a public 1.0 release, perform a proper trademark and package-registry availability check.
