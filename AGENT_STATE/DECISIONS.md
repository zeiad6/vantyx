# Decisions

Append-only record of engineering decisions made in autonomous sessions. Design
rationale that affects semantics belongs in `docs/decisions/` as an ADR; this
file records operational choices.

## 2026-11-24 — T-054 incremental RAG scope

- Reuse vectors only when the stable chunk id, provider, model, and dimensions
  all match. Any mismatch is an explicit error or a full rebuild, never a silent
  mix.
- Treat the current `--root` scan as the complete corpus and prune chunks from
  deleted files. Partial-prefix updates and multi-root merging are out of scope.
- Require an existing semantic index for `--incremental`; reject lexical indexes
  and provider/model/dimension changes with actionable help.
- Skip provider construction entirely when no chunk is pending, so a no-op
  update performs no network or model work.
- Allow an empty semantic index (`dimensions > 0`, zero chunks) so deleting every
  document is representable; empty search returns no hits.
- Keep ingestion and chunking full-corpus. Document that the measured saving is
  avoided embedding calls, not scan time.
- Defer streaming tool calls because the CLI has no tool-execution surface;
  implementing them now would be library-only.

## 2026-11-24 — Enum benchmark and representation

- Add an `enum_ops` workload before changing the heap-allocated native enum
  representation. The measured native median is in the same band as C and Rust
  (3.646 ms vs 3.766 ms and 4.529 ms in the same run; a targeted run measured
  4.208 ms vs 3.557 ms and 3.370 ms), so the evidence does not justify a
  representation rewrite.
- Do not claim enum performance from a single run; absolute medians shift with
  machine load, so compare implementations within the same run.

## 2026-11-24 — Native string character buffers

- Single-byte ASCII characters returned by iteration or indexing share an
  immutable static table instead of allocating a heap string per character.
  Native strings are never mutated in place, and the runtime is single-threaded,
  so the shared pointers preserve semantics.
- Multibyte UTF-8 characters still allocate. Concatenation still copies into a
  fresh exact-size buffer; no ownership or reclamation change is implied.
- The measured `string_ops` improvement is a runtime optimization, not a claim
  about general string performance.

## 2026-11-24 — Verification and status discipline

- `VERIFIED` requires an observed command, date, and recorded evidence.
- Benchmarks remain labelled plumbing-only when using the offline hash provider.
- Do not claim live AI, grounding, GPU, training, or production readiness without
  direct evidence.
