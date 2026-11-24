# Active Task

## T-054 — Incremental RAG re-indexing with vector reuse and pruning

- Status: VERIFIED locally; commit pending
- Priority: P1
- Dependencies: T-045, T-046, ADR-0018, ADR-0024

### Implemented

- `src/ai/rag.rs`: `ReuseStats`, `IncrementalPlan`,
  `RagIndex::plan_incremental`, `validate_chunk_set`, empty semantic index
  loading/search, and four unit tests.
- `src/ai_cli.rs`: `--incremental` parsing and compatibility checks, lazy
  provider construction for no-op updates, atomic save, and JSON/human reuse
  reporting; one parser test.
- `src/ai/mod.rs`: exports `IncrementalPlan` and `ReuseStats`.
- `tests/ai.rs`: offline CLI integration test that edits one document, deletes
  another, checks reuse/prune counts, and searches after the update.
- `benchmarks/rag.py`: no-op and one-changed-chunk incremental phases with
  consistency assertions.
- `docs/decisions/ADR-0024-incremental-rag-index.md` and synchronized status,
  architecture, roadmap, README, `llms.txt`, and audit documents.

### Evidence

- 463-test gate, clippy clean, release build, examples identical.
- Manual 3-chunk reproduction: `previous_chunks:3`, `reused_chunks:1`,
  `embedded_chunks:2`, `removed_chunks:2`, `removed_sources:1`,
  `added_sources:1`.
- Benchmark: full build 42.746 ms, search 27.659 ms, no-op update 47.377 ms,
  one-chunk update 48.049 ms. The incremental path still scans and chunks the
  whole corpus, so the measured saving is avoided embedding calls, not wall
  time with the near-free hash provider.

### Remaining

- Commit, push, and confirm CI.
- No code work remains for the acceptance criteria.
