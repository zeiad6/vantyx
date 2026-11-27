# ADR-0042: Safetensors shard-index aggregation

- Status: Accepted
- Date: 2026-11-26
- Related: T-091, ADR-0041 (offline model inspection), ADR-0007 (zero
  dependencies), ADR-0015 (AI host boundary), `docs/ai/MODEL_INSPECTION.md`

## Context

ADR-0041 added bounded, offline inspection of one GGUF or Safetensors file.
Large Safetensors models are normally distributed as several shard files plus a
`model.safetensors.index.json` that maps every tensor name to its shard. A user
who inspects one shard at a time cannot answer the questions that matter for
provenance: how many tensors and parameters the complete model has, which
shards belong to it, whether the index and shard headers agree, and whether the
index silently references a missing or escaping path.

The obvious alternatives were:

1. **Leave aggregation to an external tool.** Adds a dependency, may load
   weights, and breaks the offline provenance path established by ADR-0041.
2. **Add a Safetensors or Hugging Face crate.** Violates the zero-dependency
   rule in ADR-0007 for a read-only summary.
3. **Parse the index and each shard header in-tree.** Reuses the existing
   bounded JSON parser, SHA-256, and Safetensors validation, and keeps the
   command offline and dependency-free.

## Decision

Extend `vantyx ai inspect` to recognize a Safetensors shard index and aggregate
its referenced shards.

The inspector:

- Recognizes an index when the file is not a plausible GGUF or Safetensors
  binary and either its first non-whitespace byte is `{` or its path ends in
  `.json`. The JSON object must contain a `weight_map` object; otherwise the
  input is a format error.
- Reads at most 64 MiB of index JSON and accepts at most 1,000,000 tensor
  entries and 10,000 distinct shards.
- Requires every shard path to be relative, to use `/` separators, to contain
  no `..` component, and to end in `.safetensors`. It resolves the index
  directory and each shard with `canonicalize`, then rejects a shard whose
  canonical path does not stay under the index directory.
- Inspects every referenced shard with the ADR-0041 Safetensors path, so shard
  ranges, dtypes, shapes, offsets, and byte lengths are validated exactly.
- Reads each shard header a second time to compare its tensor names with the
  index. A shard tensor missing from the index, an indexed tensor missing from
  its shard, or a tensor appearing in more than one shard is a format error.
- Aggregates tensor count, parameter count, data bytes, and the dtype
  histogram. The summary's own `file_bytes` and `sha256` describe the index
  JSON; each `ShardSummary` carries the shard's size, digest, tensor count,
  parameter count, and data bytes.
- Reports a warning when `metadata.total_size` is not a non-negative integer or
  does not equal the summed shard data bytes. It does not reject the model for
  this mismatch because index conventions and padding can differ.
- Never downloads a shard, loads weights, executes model code, or modifies any
  file. It does not scan the directory for unlisted shards.

A shard file that is itself an index is rejected instead of being recursively
expanded, so a hostile pair of files cannot cause unbounded recursion.

## Consequences

Positive:

- One command validates and summarizes a complete sharded Safetensors model,
  including cross-checks between the index and every shard header.
- The index digest and per-shard digests are available in `--json` for
  reproducible experiments and future pull/remove integrity checks.
- The implementation reuses the existing parser, validator, and SHA-256; no
  dependency or native-backend change is introduced.
- Lexical and canonical path checks prevent an index from silently reading
  files outside its own directory.

Negative and limits:

- Inspection time and I/O grow with the total size of all shards because every
  shard is hashed.
- The inspector reads each shard header twice: once during validation and once
  for the index cross-check. This is intentional and bounded, but it is not a
  single-pass parser.
- Validation is best-effort and not atomic. A shard can change between its
  digest and its header check, and a symlink can be retargeted concurrently.
- The index cannot prove that the directory contains no extra shards, and it
  does not compare against a remote repository or verify licenses.
- `metadata.total_size` is treated as advisory. A mismatch produces a warning,
  not a format error, because not every producer defines it identically.
- GGUF files remain single-file; only Safetensors indexes are aggregated.

## Tests

- `src/ai/model.rs` unit tests cover a two-shard aggregate, per-shard digests,
  dtype merging, JSON shard output, a `total_size` mismatch warning, a missing
  `weight_map`, escaping and absolute paths, a wrong extension, an extra shard
  tensor, an indexed tensor missing from its shard, a missing shard file, a
  non-Safetensors shard, a non-string shard value, a nested index, and a
  Unix-only symlink escape.
- `tests/ai.rs` runs the CLI end to end on a two-shard index in text and JSON
  modes and checks the missing-shard `AI_E_IO` path.
- The full repository gate is recorded in `TASKS.md` and `PROJECT_STATUS.md`.

## Alternatives considered

1. **Add a separate `vantyx ai inspect-index` command.** Rejected: content
   detection keeps one command and one output schema, and the file type is
   unambiguous.
2. **Trust `weight_map` without reading shard headers.** Rejected: the index
   could list tensors that do not exist or omit tensors that do, which is
   exactly the integrity failure the command should catch.
3. **Scan the directory for every `.safetensors` file.** Rejected: unlisted
   files may be unrelated, and a directory scan adds unbounded candidate work.
   The index is the declared model boundary.
4. **Reject a `total_size` mismatch.** Rejected: producers differ on whether
   the value counts payload bytes, tensor bytes, or padded bytes; a warning is
   more honest than a false rejection.
