# ADR-0047 — Exact JSONL dataset deduplication

Status: Accepted
Date: 2026-11-26
Task: T-100

## Context

`vantyx ai dataset` validates JSONL corpora and `vantyx ai split` writes
deterministic train/held-out files, but neither removes duplicate records.
Duplicates inflate training cost, bias evaluation, and make a corpus harder to
audit. A deduplication command must be reproducible, must not silently rewrite
records, and must not risk overwriting a corpus.

The split boundary already assigns byte-identical records to the same side. That
makes exact record bytes the natural identity for a first deduplication
primitive: it is deterministic, needs no JSON canonicalization, and composes
with splits.

## Decision

Add `vantyx ai dedup <file> --out <file>` with these rules:

1. **Identity is exact normalized record bytes.** Strip the first-line BOM, a
   trailing CR from CRLF, and the line terminator; blank lines are not records.
   Do not reorder object keys, normalize whitespace, unify escapes, or rewrite
   numbers. `{"a":1,"b":2}` and `{"b":2,"a":1}` are distinct.
2. **Keep the first occurrence in input order.** The output is stable and easy
   to diff against the input.
3. **Digest with a domain separator.**
   `SHA-256("vantyx-dataset-dedup-v1\0" || record_bytes)`.
4. **Validate before writing.** Reuse the dataset inspection gate, including
   the optional schema, and revalidate during the write pass. Abort and remove
   the output if the input changes or the counts disagree.
5. **Never overwrite.** Require `--out`, refuse an existing path, refuse the
   input path, and use `create_new`.
6. **Bound memory.** Retain at most 1,000,000 distinct digests in memory. Fail
   with a format error when the limit is exceeded rather than silently growing
   without bound.
7. **Report provenance.** Emit input/output SHA-256 values and byte counts,
   record counts, duplicates removed, and normalization warnings in text or
   JSON.

## Consequences

- The command is deterministic and order-preserving; the same input always
  produces the same output bytes.
- Byte-based identity is conservative: semantically equal records with
  different formatting are not merged. This is documented and can be extended
  later with an explicit canonicalization mode.
- The in-memory digest set makes memory O(unique records) with a hard cap. The
  input remains streamed; a corpus with more than 1,000,000 distinct records is
  rejected rather than risking unbounded memory.
- The output is a single file, not a pair, so a failed run only has one file to
  remove.
- No new dependency, runtime type, or error code is introduced. The command
  uses the existing JSON parser, schema validator, SHA-256 implementation, and
  dataset line reader.

## Alternatives considered

- **Canonical JSON identity (sorted keys, normalized numbers).** More semantic
  deduplication, but it rewrites records and needs a canonical encoder plus a
  documented number/escape policy. Deferred.
- **Near-duplicate detection (MinHash/SimHash).** Useful for web-scale corpora,
  but probabilistic and much larger in scope. Deferred.
- **Deduplicate inside `split`.** Convenient, but couples two independent
  policies and makes it harder to audit which records were removed. A separate
  command keeps each step reproducible.
- **On-disk external sort.** Avoids the digest-set cap but adds temporary files,
  cleanup, and a much larger implementation. The 1,000,000-record cap is an
  honest first bound.
