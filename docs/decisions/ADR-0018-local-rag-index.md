# ADR-0018 — Local RAG index format and retrieval boundary

- Status: Accepted
- Date: 2026-11-24
- Supersedes: none
- Related: ADR-0007 (zero dependencies), ADR-0015 (AI host foundation),
  ADR-0017 (embedding providers and cache), `docs/ai/AI_ARCHITECTURE.md`,
  `docs/ai/AI_SECURITY.md`

## Context

Embeddings and BM25 existed, but there was no reproducible way to ingest a local
corpus, persist it, retrieve cited chunks, or measure retrieval quality. The
next AI-4 step needed a RAG path that stayed local-first, used the existing
provider boundary, survived process restarts, and did not pretend to be a
production vector database or a grounded-answer system.

The available alternatives were:

1. Adopt Qdrant, LanceDB, sqlite-vec, or fastembed. Each adds a service, native
   library, or model files, conflicting with ADR-0007 for the first local path.
2. Build a small flat index over the existing embedding and BM25 components,
   persist it as strict JSON, and evaluate retrieval with local fixtures.

## Decision

Use a small, strict, local-first RAG pipeline.

1. **Bounded ingestion.** Ingestion accepts one canonical root, repository
   relative `/`-separated source paths, and an extension allowlist (text,
   Markdown, and code). Symlinks are rejected, hidden/VCS/build directories are
   skipped, and limits are 4,096 files, 1 MiB per file, 32 MiB total source,
   10,000 chunks, 4,096 chunk characters, 2,048 overlap characters, 48 MiB total
   chunk text, 8 KiB queries, and top-k 100.
2. **Stable line-cited chunks.** Chunk ids are FNV-1a values over source, line
   range, and text; they are stable for unchanged content and are not
   cryptographic. Every chunk carries `source`, `start_line`, and `end_line`.
   Long single lines are split without losing bytes; short lines are grouped up
   to the configured chunk size with bounded overlap.
3. **Index format version 1.** The persisted object is
   `{version, provider, model, dimensions, chunks:[{id, source, start_line,
   end_line, text, vector?}]}`. Lexical indexes store no vectors
   (`dimensions = 0`); semantic and hybrid indexes store one finite vector per
   chunk with consistent dimensions.
4. **Validation before use.** Loading rejects unknown versions, duplicate chunk
   ids, invalid line ranges, non-finite or malformed vectors, a mix of present
   and absent vectors, dimension/count mismatches, duplicate or missing fields,
   oversized text, and files above 64 MiB. Lexical state is rebuilt only after
   validation succeeds.
5. **Atomic, owner-only persistence.** Saves serialize to a temporary file in
   the destination directory, sync where supported, and rename over the target.
   On Unix the file mode is `0600`. The index is not encrypted; it contains full
   chunk text and vectors and must be treated as sensitive local data.
6. **Three explicit search modes.** `lexical` uses BM25 and never calls an
   embedding provider. `semantic` requires vectors and uses cosine similarity.
   `hybrid` requires vectors and fuses both ranked lists with reciprocal-rank
   fusion (`k = 60`) and deterministic chunk-id tie breaking. Results expose
   lexical score, semantic score, both ranks, fusion score, and citations. An
   optional relative source-prefix filter is applied before top-k and is not an
   authorization boundary.
7. **Retrieval evaluation only.** `vantyx ai eval` scores a JSON fixture with
   recall@k, precision@k, MRR, and per-query hits against exact chunk ids or
   source paths. It does not measure answer accuracy, grounding, hallucination
   rate, latency, or memory, and no answer is generated.
8. **CLI surface.** `vantyx ai index`, `search`, and `eval` share provider,
   endpoint, network, timeout, JSON, and metrics policy with the rest of
   `vantyx ai`. `--metrics` is explicitly rejected for these commands rather
   than silently ignored.

## Consequences

Positive:

- The full ingest → embed → persist → retrieve → evaluate loop runs offline and
  is reproducible in tests.
- Citations are part of the data model, not an afterthought.
- The index can be inspected, diffed, and validated with ordinary tools.
- No new dependency, service, or bundled model is required.
- Unsupported semantic search on a lexical index fails clearly.

Negative and remaining work:

- Search is linear over chunks; this is not a production vector database.
- The format is version 1 with no migration path.
- Indexes are full rewrites; there is no incremental update, deletion,
  deduplication, arbitrary metadata fields, or reranking. Only a relative
  source-prefix filter is supported.
- Only text, Markdown, and code are ingested; PDF/HTML/Office extraction is not
  implemented.
- The hash provider is lexical, not a trained semantic model; live embedding
  models remain unverified.
- Evaluation labels are exact ids or source paths; graded relevance, answer
  grounding, and hallucination measurement are absent.
- The index is not encrypted and has no tenant isolation beyond file mode.

## Alternatives rejected

- **Adopt a vector database now.** Rejected: service/native dependency and no
  measured need at current corpus sizes.
- **Store vectors in a binary format first.** Rejected for v1: strict JSON is
  easier to validate, inspect, and test; a binary format needs its own version,
  checksums, and tooling.
- **Persist only vectors and re-read documents at search time.** Rejected:
  source files change and disappear; the index must remain self-contained and
  citations must match the indexed text.
- **Claim grounded generation.** Rejected: retrieval alone does not prove that a
  generated answer uses the sources correctly.
- **Silently ignore an unsupported `--metrics` or search mode.** Rejected:
  explicit errors preserve the no-silent-miscompile/configuration principle.

## Security notes

- Ingestion is confined to a canonical root; path traversal and symlinks are
  rejected, and hidden/VCS/build directories are skipped.
- Limits bound file count, bytes, chunks, query size, and top-k.
- Lexical search is offline. Semantic and hybrid search use the same explicit
  provider, network, timeout, and response-size policy as other AI commands.
- Index loading treats the file as untrusted: strict types, finite vectors,
  range checks, and duplicate-id checks happen before search.
- Saved indexes are atomic and `0600` on Unix, but not encrypted. Protect the
  directory and delete indexes that contain sensitive text.
- Retrieved text is untrusted input and carries no trust label yet.
- No generation step exists, so no prompt-injection path through generated
  answers is claimed to be solved.

## Evidence

- `src/ai/rag.rs`: ingestion, chunking, index persistence, search, fusion,
  evaluation, and 14 unit tests.
- `src/ai_cli.rs`: `vantyx ai index`, `search`, and `eval` plus argument and
  mode validation.
- `tests/ai.rs`: `rag_index_search_and_eval_are_offline_and_cited` (including a
  source-prefix filter) and
  `lexical_rag_index_rejects_semantic_search_and_bad_limits`.
- `examples/ai/retrieval-eval.json`: reproducible evaluation fixture.
- Verification: 2026-11-24 full gate, 413 tests; manual three-document hash run
  returned `rust.md:1-4` first for "memory safety" and recall@2 = 1.0 / MRR =
  1.0 on the fixture. `benchmarks/rag.py` measured 200 documents / 600 chunks:
  index median 41.639 ms, hybrid search median 26.720 ms, 1,247,300-byte index,
  and 10 cited hits per query. No network or live model was used.
