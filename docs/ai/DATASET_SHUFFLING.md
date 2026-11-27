# Deterministic JSONL Dataset Shuffling

`vantyx ai shuffle <file> --out <file>` validates a local JSONL dataset and
writes a deterministic seeded permutation of its records. It is the offline
counterpart of `vantyx ai split` and the companion of `vantyx ai dedup`: the
same validation gate runs first, and the output is a pure function of the set
of records and the seed.

The command is offline: it never downloads data, executes dataset content, or
contacts a provider. It does not modify the input file.

## Usage

```sh
vantyx ai shuffle train.jsonl --out train.shuffled.jsonl
vantyx ai shuffle train.jsonl --out train.shuffled.jsonl --seed 7
vantyx ai shuffle train.jsonl --out train.shuffled.jsonl \
  --schema record.schema.json --json
```

`--out` is required and names the single output file. The parent directory
must already exist. The output is created with `create_new`; the command
refuses to overwrite an existing file and refuses to write over the input.
`--seed` is an unsigned 64-bit integer and defaults to `0`.

Text output:

```text
format: jsonl-shuffle
input: train.jsonl
input size: 310 bytes
input sha256: deaef7...
records: 10
seed: 7
output: train.shuffled.jsonl (310 bytes)
  sha256: 6f3c2a...
warnings:
  1 blank line(s) were not copied
  1 CRLF line ending(s) were normalized to LF
```

`--json` emits the same fields with `format: "jsonl-shuffle"`, including
`records`, `seed`, input/output byte counts and SHA-256 values, and the
warnings array.

## Ordering

Each record is ordered by:

```text
SHA-256("vantyx-dataset-shuffle-v1\0" || seed_le64 || record_bytes)
```

with the record bytes as a tie-breaker. The result depends only on the set of
records and the seed, not on the order in which they appeared in the input.
Two files that contain the same records in different orders produce the same
output for the same seed.

The domain separator prevents a shuffle key from being confused with a split
bucket, a deduplication digest, or another protocol. There is no PRNG state and
no hidden entropy: the same input bytes and seed always produce the same output
bytes, regardless of process, machine, or time.

A different seed normally produces a different order. For a one-record dataset
the order is trivially unchanged.

## Validation and output safety

The command runs the same checks as `vantyx ai dataset`:

- the input is a regular file no larger than 64 MiB;
- every non-blank line is one UTF-8 JSON object;
- each line is no larger than 16 MiB;
- when `--schema` is supplied, every record satisfies the supported JSON Schema
  subset.

Invalid content, a schema violation, or an I/O failure aborts before any output
is kept. The input is read once into memory, validated, and hashed; the output
is then written in sorted order. If writing fails, the output is removed. The
output record count is checked against the validated count before success is
reported. Through the CLI the inspection pass reads the input once more, so the
validation gate and the shuffle pass cannot disagree about validity.

## Determinism and memory

The whole dataset is held in memory because a global permutation needs every
record before the first output line can be chosen. The command accepts at most
**64 MiB** of input and **1,000,000 records**; exceeding either limit is a
format error and no output is kept. These bounds are lower than the 256 MiB
inspection cap on purpose, and they are explicit memory bounds rather than a
claim of unlimited scale. The 16 MiB line cap still applies.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | The output was written; every input record was valid and, when supplied, schema-valid. |
| `1` | Invalid input, schema-invalid records, a malformed schema, an I/O failure, an existing output, or a shuffle limit. |
| `64` | Usage error (missing path or `--out`, unknown option, missing or duplicate option value, invalid seed). |

## What it does not do

- It does not use a conventional PRNG or reservoir sampling.
- It does not deduplicate, split, tokenize, or stratify records.
- It does not preserve BOM, CRLF, or blank lines; records are written with one
  LF terminator each.
- It does not modify, move, or delete the input.
- It does not append to or overwrite an existing output.
- It does not create parent directories.
- It does not provide a streaming/external-memory shuffle.
- It does not guarantee that a small dataset changes order; with one record the
  output is necessarily identical.

## Related

- `vantyx ai dataset` — bounded JSONL inspection and schema validation.
- `vantyx ai split` — deterministic train/held-out splits.
- `vantyx ai dedup` — exact duplicate-record removal.
- `vantyx ai shuffle` — this command.
- [DATASET_INSPECTION.md](DATASET_INSPECTION.md),
  [DATASET_SPLITTING.md](DATASET_SPLITTING.md),
  [DATASET_DEDUPLICATION.md](DATASET_DEDUPLICATION.md).
- [ADR-0048](../decisions/ADR-0048-deterministic-dataset-shuffle.md) — decision
  record for content-hash shuffle ordering.
