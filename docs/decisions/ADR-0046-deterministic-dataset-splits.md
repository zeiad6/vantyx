# ADR-0046: Deterministic train/held-out dataset splitting

- Status: Accepted
- Date: 2026-11-26
- Task: T-098

## Context

`vantyx ai dataset` validates JSONL corpora and reports provenance, but it
deliberately does not split them. The AI roadmap lists deterministic
train/held-out splits as the next dataset capability. A split has to satisfy
several constraints at once:

- The same input and options must produce the same files on every machine.
- A record must not move between splits because unrelated lines were inserted
  or reordered.
- The command must not silently overwrite an existing training file.
- Invalid or schema-invalid data must be rejected before any output exists.
- Memory use must stay bounded for the 256 MiB dataset limit.

A conventional shuffle with a seeded PRNG violates the reordering property
unless the whole input is buffered and sorted, and storing every record hash
also grows with the record count. A time-based or process-random seed makes
experiments irreproducible.

## Decision

Assign each record independently with a seeded content hash:

```text
bucket = SHA-256("vantyx-dataset-split-v1\0" || seed_le64 || record_bytes)
train  = top 32 bits of bucket < floor(train_ratio * 2^32)
```

This gives order-independent, machine-independent, O(1)-memory assignment.
Identical records always share a split, which prevents duplicate leakage even
though deduplication itself remains future work.

Supporting decisions:

- Validate first with the existing inspection gate, then revalidate during the
  split scan. The CLI also passes `--schema` to `split_with_schema`, so the
  records written are the records that were checked.
- Normalize a leading BOM, CRLF endings, and blank lines out of the outputs,
  and report each normalization as a warning. The input is never modified.
- Derive output names from the input stem and require `--out-dir`. Use
  `create_new` for both files and remove any file created by a failed run.
- Guarantee both splits are non-empty for datasets with at least two records by
  moving the minimum- or maximum-hash record when the threshold leaves one side
  empty. Report the move. Random retries would make the output depend on
  hidden state.
- Report the actual counts rather than claiming the requested ratio was met.
- Keep shuffling, deduplication, tokenizer-aware counts, and label
  stratification out of scope.

## Consequences

- Splits are reproducible from the report's seed, ratio, and input digest.
- The achieved ratio can differ from the requested ratio, especially for small
  datasets; the report makes this visible.
- A record cannot be moved between splits by reordering the input.
- The fallback can shift one record more than the ratio implies; it only runs
  when a side would otherwise be empty.
- The command reads the input twice, plus one inspection pass through the CLI.
  This is a deliberate trade for validation and bounded memory.
- Output files are normalized to LF and are not byte-identical to the input.
- The pair is not atomic across a process crash, but a failed run does not keep
  a partial pair.
