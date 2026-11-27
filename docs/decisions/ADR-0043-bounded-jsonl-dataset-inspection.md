# ADR-0043: Bounded JSONL dataset inspection

- Status: Accepted
- Date: 2026-11-26
- Task: T-092

## Context

The AI host can inspect model files (ADR-0041, ADR-0042) but has no way to
validate the datasets that feed retrieval, extraction evaluation, or future
training. JSONL is the common interchange format for instruction, preference,
and text corpora, and malformed lines are a frequent source of silent training
corruption.

The project has no third-party Rust dependencies. A dataset tool must therefore
parse JSON with the in-tree parser, stream rather than load large files, and
report provenance (size and digest) so a run can be reproduced.

## Decision

Add `src/ai/dataset.rs` and `vantyx ai dataset <file> [--json]`.

- The file is streamed line by line through the in-tree SHA-256; it is never
  loaded whole.
- Each non-blank line must be one JSON object. Invalid JSON, invalid UTF-8,
  and non-object records are counted and the first 20 are reported with line
  numbers.
- A UTF-8 BOM is accepted on the first line; LF and CRLF are accepted.
- Top-level field names are summarized with record counts and a JSON type
  histogram. Output is sorted for determinism.
- Explicit limits: 256 MiB file, 16 MiB line, 10,000 distinct top-level
  fields, 20 reported invalid lines.
- Exit `0` when every non-blank line is a JSON object, `1` when invalid lines
  or an I/O error occurred, and `64` for usage errors. The summary is printed
  before the failure so callers can inspect it.
- The command is read-only and offline.

## Consequences

- Dataset validation no longer requires a Python or Hugging Face tool.
- The digest covers the exact bytes read, including blank lines and line
  endings, so provenance is stable.
- The inspector intentionally does not tokenize, split, shuffle, deduplicate,
  or validate a schema. Those remain separate steps.
- Nested fields are reported only as `object`; a future task can add bounded
  nested summaries if needed.
- Duplicate JSON keys inside one object are counted per occurrence rather than
  merged. This is documented rather than silently normalized.
- A line larger than 16 MiB invalidates that line but does not abort the scan,
  so the digest and total line count remain available.

## Alternatives considered

- **Reuse `vantyx ai inspect`:** rejected. That command's format detection is
  model-oriented and its summaries are tensor-oriented.
- **Add `serde`/`serde_json`:** rejected. It violates the zero-dependency rule
  and would not change the streaming design.
- **Parse the whole file into memory:** rejected. Large corpora would exhaust
  memory and the digest would be computed after allocation.
- **Reject CRLF/BOM:** rejected. Real corpora frequently contain both, and
  accepting them costs no ambiguity.
- **Exit `0` on invalid lines:** rejected. Dataset validation is usually a
  gate; a zero exit would hide corruption in scripts.

## Limits

The check is syntactic and structural, not semantic. It does not know which
fields a trainer requires, does not detect near-duplicates or label leakage,
and does not guarantee that a dataset is safe or licensed for use. The digest
is computed during the same pass as validation and is not an atomic snapshot;
a concurrent writer can change the file mid-scan.
