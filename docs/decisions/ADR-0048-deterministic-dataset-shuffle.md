# ADR-0048 — Deterministic JSONL dataset shuffle

Status: Accepted
Date: 2026-11-26
Task: T-101

## Context

`vantyx ai dataset`, `vantyx ai split`, and `vantyx ai dedup` validate and
reshape JSONL corpora, but none changes the order of records. Training often
needs a shuffled corpus, and an ad hoc shell `shuf` is neither reproducible
across machines nor validated against the dataset gate. A shuffle command must
be deterministic for a given seed, must validate before writing, and must not
risk overwriting a corpus.

The split boundary already uses a seeded content hash to assign records without
depending on line order. The same principle can define a global order: hash each
record with the seed and sort by the digest.

## Decision

Add `vantyx ai shuffle <file> --out <file> [--seed <n>]` with these rules:

1. **Order by a domain-separated content hash.**
   `SHA-256("vantyx-dataset-shuffle-v1\0" || seed_le64 || record_bytes)`, with
   the exact record bytes as a tie-breaker. The order depends only on the set of
   records and the seed, not on their input order.
2. **Validate before writing.** Reuse the dataset inspection gate, including the
   optional schema. Parse every record as a top-level JSON object and validate
   schema constraints before creating the output.
3. **Never overwrite.** Require `--out`, refuse an existing path, refuse the
   input path, and use `create_new`. Remove the output if writing fails.
4. **Bound memory.** Hold the whole dataset in memory under a 64 MiB input cap
   and a 1,000,000-record cap. Exceeding either limit is a format error with no
   output kept.
5. **Normalize only line framing.** Remove a first-line BOM, CRLF endings, and
   blank lines; write one LF per record. Do not rewrite JSON content.
6. **Report provenance.** Emit the seed, input/output SHA-256 values and byte
   counts, record count, and normalization warnings in text or JSON.

## Consequences

- The command is deterministic and input-order independent: two corpora with
  the same records in different orders produce the same output for the same
  seed.
- A different seed normally produces a different order; a one-record dataset is
  necessarily unchanged.
- The whole dataset is held in memory, so the input cap is lower than the
  streaming inspection cap. This is an explicit bound, not a claim of unlimited
  scale. An external-memory shuffle is deferred.
- Byte-based ordering does not canonicalize JSON; records remain exactly as
  written, apart from line framing.
- No new dependency, runtime type, or error code is introduced. The command uses
  the existing JSON parser, schema validator, SHA-256 implementation, and
  dataset line reader.

## Alternatives considered

- **Fisher–Yates with a seeded PRNG.** Familiar, but requires a specified PRNG
  and consumes O(n) random state; reproducing the exact order across languages
  or versions is harder than comparing a SHA-256 digest.
- **`shuf` or another external tool.** Not portable, not validated, and not
  reproducible without pinning the tool and its seed semantics.
- **Shuffle inside `split`.** Couples ordering and partitioning, makes each
  step harder to audit, and would change the split contract.
- **Streaming reservoir sampling.** Uses constant memory but needs a specified
  PRNG and does not produce the same permutation as a content-hash sort. Deferred
  with the external-memory design.
- **On-disk external sort.** Avoids the memory cap but adds temporary files,
  cleanup, and a much larger implementation. The 64 MiB/1,000,000-record bounds
  are an honest first step.
