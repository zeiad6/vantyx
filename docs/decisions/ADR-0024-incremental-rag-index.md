# ADR-0024 — Incremental RAG re-indexing and vector reuse

- Status: Accepted
- Date: 2026-11-24
- Supersedes: none
- Related: ADR-0007 (zero dependencies), ADR-0015 (AI host foundation),
  ADR-0017 (embedding providers and cache), ADR-0018 (local RAG index),
  `docs/ai/AI_ARCHITECTURE.md`, `docs/ai/AI_ROADMAP.md`

## Context

The local RAG index rebuilds every chunk and re-embeds the complete corpus on
every `vantyx ai index` run. With the offline hash provider that is cheap, but
with a trained embedding model it is the dominant cost and repeats work for
documents that did not change. The index already stores stable chunk ids and one
vector per chunk, so unchanged chunks can be identified without a new manifest.

The available alternatives were:

1. Keep full rebuilds only. Simple, but wasteful for real embedding providers.
2. Add a separate mtime/size manifest or an embedded database. This introduces
   new persistent state, clock/portability issues, and more failure modes before
   the basic reuse path is proven.
3. Reuse vectors by stable chunk id inside the existing version-1 JSON index,
   with strict provider/model/dimension continuity checks.

## Decision

Add an explicit incremental update path over the existing index format.

1. **Reuse by stable chunk id.** A chunk id is an FNV-1a value over source,
   line range, and text. A matching id therefore commits the same source, line
   range, and text. When the provider, model, and dimensions also match, the
   stored vector is reused without an embedding call.
2. **Strict compatibility.** Incremental updates require an existing semantic
   index with the same provider, model, and dimensions. Lexical indexes,
   missing indexes, and configuration changes are rejected with a message that
   points to a full rebuild. The CLI uses the existing index's provider, model,
   and dimensions unless the user explicitly requests a different one, in which
   case the update is refused.
3. **Embed only pending chunks.** `RagIndex::plan_incremental` returns an
   `IncrementalPlan` containing every current chunk, an optional reused vector
   per chunk, and `ReuseStats`. The caller embeds only
   `IncrementalPlan::pending_chunks()` and completes the plan with
   `IncrementalPlan::finish`.
4. **Prune removed chunks.** The current corpus scan is authoritative. Chunks
   present in the previous index but absent from the current scan are dropped.
   A changed chunk is therefore counted as one removed chunk and one pending
   (re-embedded) chunk.
5. **No provider call for a no-op.** When every current chunk has a reusable
   vector, the provider is not constructed and no network or model call occurs.
6. **Empty semantic indexes are representable.** Removing every document writes
   a valid version-1 index with the original dimensions and zero chunks. It
   round-trips through the loader and searches to an empty result set. A vector
   index with chunks but missing vectors is still rejected.
7. **CLI surface.** `vantyx ai index --incremental` requires `--out` to name an
   existing semantic index. JSON output reports `previous_chunks`,
   `reused_chunks`, `embedded_chunks`, `removed_chunks`, `removed_sources`, and
   `added_sources`; human output reports the same counts.
8. **No hidden ingestion shortcut.** The command still walks and chunks the
   complete corpus. The measured saving is embedding work for unchanged chunks,
   not ingestion time. The current `--root` is the complete corpus; partial
   prefix updates and multi-root merging are not supported.

## Consequences

Positive:

- Unchanged chunks avoid embedding calls, which matters most with trained or
  remote embedding models.
- Deletions and edits are reflected without manual index surgery.
- The behavior is deterministic, offline-testable, and adds no dependency or
  second persistent format.
- Reuse is conservative: any mismatch falls back to an explicit error or a
  full rebuild rather than silently mixing vectors from different models.

Negative:

- Every update still pays ingestion, chunking, JSON load, and atomic save costs.
- A changed chunk is re-embedded in full even if only one line changed; there is
  no sub-chunk diffing.
- The current corpus scan defines the whole index, so partial-prefix updates
  are not available.
- The index remains a flat JSON file with linear search; this is not a vector
  database and does not add metadata filtering, deduplication, or reranking.
- The hash provider remains a lexical baseline, not evidence of semantic
  quality or live model performance.

## Alternatives considered

- **Mtime/size manifest:** rejected for now because it adds a second state file
  and cross-platform timestamp assumptions before chunk-id reuse is measured.
- **Database upserts:** rejected because ADR-0018 deliberately keeps the first
  local index dependency-free and inspectable.
- **Silent provider fallback:** rejected because mixing vectors from different
  models or dimensions would produce invalid similarity scores.
