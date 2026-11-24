# Completed Tasks

Only tasks with an observed verification command and date are listed here.
See `TASKS.md` for full acceptance criteria and evidence.

- T-056 — Native enum construction and match benchmark (2026-11-24;
  seven-workload suite; `enum_ops` native 3.646 ms vs C 3.766 ms and Rust
  4.529 ms; representation rewrite not justified).
- T-055 — Shared ASCII character buffers for native string iteration and
  indexing (2026-11-24; 464-test gate; `string_ops` 8.318 ms → 1.565 ms in the
  latest run).
- T-054 — Incremental RAG re-indexing with vector reuse and deletion pruning
  (2026-11-24; 463-test gate; ADR-0024; CI run 45376386743).
- T-053 — Placeholder attribution correction and history rewrite
  (2026-11-24; CI run 45365926013, 457 tests; REST endpoint now lists only
  `zeiad6`).
- T-052 — Streamed OpenAI-compatible text chat (2026-11-24; 457-test gate;
  ADR-0023).
- T-051 — Integer range patterns (2026-11-24; 437-test gate; ADR-0022).
- T-050 — Native top-level or-pattern lowering (2026-11-24; 430-test gate;
  ADR-0021).
- T-049 — Native nested-pattern lowering (2026-11-24; 429-test gate;
  ADR-0020).
- T-048 — Nested payload patterns and matrix exhaustiveness (2026-11-24;
  429-test gate; ADR-0019).
- T-047 — Top-level or-patterns (2026-11-24; 422-test gate).
- T-046 — Local RAG indexing, hybrid retrieval, citations, and evaluation
  (2026-11-24; 413-test gate; ADR-0018).
- T-045 — Embedding provider interface, bounded cache, and `vantyx ai embed`
  (2026-11-24; CI runs 45330680289 and 45330898922; ADR-0017).
- T-044 — Native scalar-key map hash index (2026-11-24; CI run 45321718008;
  ADR-0016).
- T-013 — Modules and imports (2026-11-24; 413-test gate).
- T-037–T-039, T-041–T-043 — AI host foundation, CLI, OpenAI-compatible
  adapter, discovery sync, and validated structured extraction (2026-11-24).
- T-029–T-033 — REPL upgrade, native enums, bracketed-match diagnostic, scalar
  literal patterns, and match guards (2026-11-24).
- T-035 — Public repository, CI, and v0.1.0 pre-release (2026-11-24).
- T-057 — Deterministic MMR diversity reranking for local RAG search
  (2026-11-24; 469-test gate; `--diverse`, duplicate suppression,
  cosine/Jaccard similarity; ADR-0025; RAG diverse search 28.292 ms vs
  27.175 ms standard over 600 chunks).
- T-056 — Native enum construction and match benchmark (2026-11-24; 464-test
  gate; `enum_ops` native 3.646 ms vs C 3.766 ms and Rust 4.529 ms; CI run
  45380623770).
