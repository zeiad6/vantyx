# ADR-0044: Bounded dataset schema validation

- Status: Accepted
- Date: 2026-11-26
- Task: T-094

## Context

T-092 added `vantyx ai dataset` for structural JSONL validation: one JSON
object per non-blank line, field/type summaries, and streamed SHA-256
provenance. It deliberately did not check required fields or value shapes, so a
corpus could pass inspection while still being unusable for training or
evaluation.

The AI host already has a strict JSON Schema subset used by
`vantyx ai extract` for model output. Reusing that subset avoids a second,
divergent schema language and keeps the zero-dependency posture: the in-tree
JSON parser and validator are sufficient.

## Decision

Add an optional `--schema <file>` mode to `vantyx ai dataset`.

- The schema is read once, bounded to 16 MiB, UTF-8 with an optional BOM, and
  parsed with the in-tree JSON parser.
- A new `schema::check_schema` preflight verifies the supported keywords have
  valid shapes and that nesting stays within the existing depth limit of 32.
  Malformed schemas fail with `AI_E_FORMAT` before any record is validated.
- Every structurally valid record is validated with the existing
  `schema::validate`. Invalid JSON, invalid UTF-8, and non-object lines remain
  structural invalid lines and are not schema-validated.
- Each schema-invalid record is counted once with its first violation. The
  first 20 violations are reported with line number, JSON path, and message;
  the total is always counted and a warning notes unreported violations.
- The dataset summary gains an optional `schema` object with `path`,
  `invalid_record_count`, and `violations`. `is_valid` now also requires zero
  schema-invalid records.
- Exit `0` only when both structural and schema validation pass; exit `1` for
  invalid content, schema load/parse failure, or I/O failure; exit `64` for
  usage errors. The summary is printed before the failure when it exists.
- The command remains read-only and offline.

## Consequences

- A dataset pipeline can require fields and types without an external schema
  library or a Python step.
- The same supported subset governs extraction and dataset validation, so a
  schema accepted for one is accepted for the other.
- Unsupported keywords are still ignored rather than rejected, matching
  `vantyx ai extract`; users must not assume full JSON Schema compliance.
- Schema violations are reported per record, not aggregated by field. A future
  task can add histograms if needed.
- The schema itself is not hashed or recorded beyond its path; dataset
  provenance remains the streamed SHA-256.
- Splits, shuffle, deduplication, tokenization, and training remain separate
  tasks.

## Alternatives considered

- **Require a schema for every dataset:** rejected. Structural inspection is
  useful on its own and should not force users to write a schema.
- **Implement full JSON Schema:** rejected. It is a large surface with
  reference resolution, formats, and combinators that the host does not need,
  and it would expand the trust boundary.
- **Validate schema shapes lazily per record:** rejected. A malformed schema
  would produce the same confusing violation for every record, and an empty
  dataset would never reveal it.
- **Report every violation for every record:** rejected. Large corpora would
  produce unbounded output; the 20-item cap with a total count keeps the report
  useful and bounded.
- **Add a schema dependency:** rejected by the zero-dependency rule.

## Limits

The check is structural, not semantic. It cannot know which fields a trainer
requires beyond the supplied schema, detect label leakage or near-duplicates,
validate custom formats, or certify licensing. The schema file can change
between reading and reporting, and the dataset digest is not an atomic snapshot
of a concurrently written file.
