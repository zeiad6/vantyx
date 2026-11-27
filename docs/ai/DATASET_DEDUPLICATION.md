# Exact JSONL Dataset Deduplication

`vantyx ai dedup <file> --out <file>` validates a local JSONL dataset and writes
a copy with exact duplicate records removed. It is the offline counterpart of
`vantyx ai dataset` and the natural companion of `vantyx ai split`: the same
validation gate runs first, and the first occurrence of each record is kept in
input order.

The command is offline: it never downloads data, executes dataset content, or
contacts a provider. It does not modify the input file.

## Usage

```sh
vantyx ai dedup train.jsonl --out train.unique.jsonl
vantyx ai dedup train.jsonl --out train.unique.jsonl --schema record.schema.json
vantyx ai dedup train.jsonl --out train.unique.jsonl --json
```

`--out` is required and names the single output file. The parent directory
must already exist. The output is created with `create_new`; the command
refuses to overwrite an existing file and refuses to write over the input.

Text output:

```text
format: jsonl-dedup
input: train.jsonl
input size: 310 bytes
input sha256: deaef7...
records: 10 (unique: 8, duplicates removed: 2)
output: train.unique.jsonl (248 bytes)
  sha256: 6f3c2a...
warnings:
  1 blank line(s) were not copied
  1 CRLF line ending(s) were normalized to LF
```

`--json` emits the same fields with `format: "jsonl-dedup"`, including
`records`, `unique_records`, `duplicates_removed`, input/output byte counts and
SHA-256 values, and the warnings array.

## Record identity

Two records are duplicates only when their normalized bytes are identical:

- the first-line UTF-8 BOM is removed;
- a trailing `\r` from a CRLF ending is removed;
- blank lines are not records;
- JSON key order, whitespace, escapes, and numeric spelling are **not**
  normalized.

For example, `{"a":1,"b":2}` and `{"b":2,"a":1}` are different records, and
`{"n":1}` and `{"n":1.0}` are different records. This keeps the operation
predictable and makes it agree with the split boundary: records that are
byte-identical are exactly the records that `vantyx ai split` always assigns to
the same side.

The digest is `SHA-256("vantyx-dataset-dedup-v1\0" || record_bytes)`. The domain
separator prevents a digest from being confused with a split bucket or another
protocol.

## Validation and atomicity

The command runs the same checks as `vantyx ai dataset`:

- the input is a regular file no larger than 256 MiB;
- every non-blank line is one UTF-8 JSON object;
- each line is no larger than 16 MiB;
- when `--schema` is supplied, every record satisfies the supported JSON Schema
  subset.

Invalid content, a schema violation, or an I/O failure aborts before any output
is kept. The input is read twice: once to validate, count unique records, and
compute its SHA-256, and once to write the output. If the input changes between
the passes, or the written record count differs, the output is removed and the
command fails. Through the CLI the inspection pass reads the input once more, so
the validation gate and the deduplication pass cannot disagree about validity.

## Determinism and memory

The output is a pure function of the input bytes and schema: the same input
always produces the same output bytes, regardless of process, machine, or time.
There is no random state and no hidden entropy.

The digest set is held in memory so the input can be streamed. The command
accepts at most **1,000,000 distinct records**; exceeding that limit is a
format error and no output is kept. This bound keeps memory predictable while
covering typical training corpora. The 256 MiB input cap and the 16 MiB line cap
still apply.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | The output was written; every input record was valid and, when supplied, schema-valid. |
| `1` | Invalid input, schema-invalid records, a malformed schema, an I/O failure, an existing output, or the distinct-record limit. |
| `64` | Usage error (missing path or `--out`, unknown option, missing or duplicate option value). |

## What it does not do

- It does not canonicalize JSON; key order, whitespace, escapes, and numeric
  spelling differences are preserved as distinct records.
- It does not shuffle, split, tokenize, or stratify records.
- It does not modify, move, or delete the input.
- It does not append to or overwrite an existing output.
- It does not create parent directories.
- It does not detect duplicate JSON keys inside one object; the JSON parser
  keeps the last occurrence, and record identity is still byte-based.
- It does not provide a semantic near-duplicate detector or MinHash/SimHash.

## Related

- `vantyx ai dataset` — bounded JSONL inspection and schema validation.
- `vantyx ai split` — deterministic train/held-out splits.
- `vantyx ai dedup` — this command.
- [DATASET_INSPECTION.md](DATASET_INSPECTION.md),
  [DATASET_SPLITTING.md](DATASET_SPLITTING.md).
- [ADR-0047](../decisions/ADR-0047-exact-dataset-deduplication.md) — decision
  record for exact byte-based deduplication.
