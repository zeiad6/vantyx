# Active Task

## T-057 — Deterministic MMR diversity reranking for local RAG search

- Status: VERIFIED locally; commit pending
- Priority: P1
- Dependencies: T-045, T-046, T-054, ADR-0018, ADR-0025

### Implemented

- `src/ai/rag.rs`: `DEFAULT_MMR_LAMBDA` (0.7), `search_with_source_diverse`,
  `diversify_hits`, `deduplicate_hits`, `jaccard_similarity`, `mmr_select`, and
  `diversity_candidates`; shared `search_inner` path.
- `src/ai_cli.rs`: `--diverse` parsing, rejection for `index`/`eval`, and
  `rerank: mmr` in JSON/human search output.
- `src/ai/mod.rs`: `DEFAULT_MMR_LAMBDA` export.
- Tests: three RAG unit tests, one CLI parser test, one offline CLI integration
  test with duplicate and distinct documents.
- `benchmarks/rag.py`: measured standard vs diverse hybrid search with a top-1
  citation invariant and duplicate count.
- `docs/decisions/ADR-0025-mmr-diversity-reranking.md` plus synchronized
  status, baseline, benchmark, README, roadmap, AI, research, and state docs.

### Evidence

- Full gate: 469 tests (304 + 15 + 32 + 44 + 74), clippy clean, debug/release
  builds, all five examples identical under interpreter and native.
- RAG benchmark: index 43.482 ms, standard hybrid search 27.175 ms, MMR diverse
  search 28.292 ms (+1.117 ms), 10 hits, 0 duplicate texts, same top-1 citation
  `doc-0007.md:1`.

### Remaining

- Commit, push, and confirm CI.
- No code work remains for the acceptance criteria.
