# ADR-0025 — Deterministic MMR diversity reranking for local RAG search

- Status: Accepted
- Date: 2026-11-24
- Supersedes: none
- Related: ADR-0007 (zero dependencies), ADR-0015 (AI host foundation),
  ADR-0017 (embedding providers and cache), ADR-0018 (local RAG index),
  ADR-0024 (incremental RAG index), `docs/ai/AI_ARCHITECTURE.md`,
  `docs/ai/AI_ROADMAP.md`

## Context

`vantyx ai search` ranks chunks by BM25, cosine similarity, or reciprocal-rank
fusion and returns the top-k. A flat top-k can return several near-identical
chunks: duplicated documents, repeated boilerplate, or adjacent windows whose
text differs by a line or two. The index already stores chunk text and (for
semantic and hybrid indexes) one vector per chunk, so a reranking pass can
reduce redundancy without adding persistent state.

The available alternatives were:

1. Keep flat ranking only. Simple, but redundant results waste prompt context
   and can mislead retrieval evaluation.
2. Threshold-based suppression (drop hits below a similarity cutoff). Requires a
   dataset-specific threshold, fails silently on small corpora, and gives no
   control over the relevance/diversity trade-off.
3. Deterministic maximal marginal relevance (MMR) over a bounded candidate
   pool, exposed as an explicit opt-in flag.

## Decision

Add an opt-in MMR diversity reranker to the existing search path.

1. **Opt-in only.** `RagIndex::search_with_source_diverse` takes an explicit
   `lambda` in `[0.0, 1.0]`; `search` and `search_with_source` are unchanged.
   `lambda` is finite and validated before any work. The CLI exposes
   `--diverse` with a fixed default weight of `DEFAULT_MMR_LAMBDA = 0.7`.
2. **Wider candidate pool.** Diversity reranking selects from
   `limit * 4` candidates clamped to `20..=MAX_TOP_K`, never fewer than
   `limit`. When `source_prefix` is set, the already-existing behavior ranks the
   whole index before filtering, so the pool is the full filtered set.
3. **Exact duplicate suppression.** Chunks whose trimmed text is identical are
   collapsed before MMR, keeping the first (highest-ranked) copy. The removed
   copy is not cited.
4. **Similarity source.** If every candidate has a stored vector, pairwise
   similarity is cosine similarity clamped to `[0, 1]`. A zero-norm vector
   contributes `0.0`. Otherwise (lexical-only indexes, or a mixed history),
   similarity is token Jaccard similarity over `bm25::tokenize` output. This
   keeps the reranker usable without an embedding provider.
5. **Normalized relevance.** Relevance is `hit.score / max(candidate score)`,
   clamped to `[0, 1]`, so `lambda` means the same thing for lexical, semantic,
   and hybrid modes even though their raw scores have different ranges. If the
   best score is non-positive or non-finite, relevance is treated as `1.0`.
6. **Deterministic selection.** Greedy MMR selects
   `lambda * relevance - (1 - lambda) * max_similarity_to_selected`. Ties keep
   the earlier candidate, and candidate order is the underlying ranking with id
   tie-break, so repeated runs over the same index produce identical output.
   The selected hits are returned in selection order; their scores are the
   original ranking scores, not the MMR objective.
7. **No silent mode change.** Reranking does not alter lexical, semantic, or
   hybrid scoring, does not re-embed, and does not persist anything. It runs
   after filtering and before final truncation.
8. **CLI surface.** `vantyx ai search --diverse` applies reranking; JSON output
   adds `"rerank": "mmr"` (or `"none"`) and human output prints `rerank: mmr`.
   `--diverse` is rejected for `index` and `eval`, which do not rerank.

## Consequences

Positive:

- Duplicate and near-duplicate chunks no longer occupy top-k slots when
  diversity is requested.
- The same implementation works with lexical-only indexes through Jaccard
  similarity.
- Fully deterministic output with no new dependencies or persisted fields.

Negative:

- Diversity reranking can move a more relevant but redundant chunk below a
  less relevant chunk. This is the intended trade-off and is opt-in.
- Pairwise similarity is O(n²) over the candidate pool (bounded by
  `MAX_TOP_K`), so reranking adds measurable work; the benchmark records it.
- Exact-duplicate suppression is textual, not semantic: paraphrases are only
  penalized through similarity, not collapsed.
- Reranking has no measured retrieval-quality improvement yet; it is a
  deterministic context-efficiency control.

## Verification

- Unit tests in `src/ai/rag.rs`: duplicate trimming, Jaccard bounds, MMR
  ordering versus relevance-only ordering, deterministic repeated runs, and
  rejection of out-of-range or non-finite `lambda`.
- `src/ai_cli.rs` parser test for `--diverse` and rejection of `--diverse=...`.
- Offline CLI integration test in `tests/ai.rs`: index with two identical
  documents and two distinct ones, baseline search returns the duplicate,
  diverse search suppresses it, both report the expected `rerank` field, and
  `ai index --diverse` exits 64.
- `benchmarks/rag.py` measures hybrid search with and without MMR over the
  generated corpus, verifies the same top-1 citation, and records the result
  under `benchmarks/results/`.
