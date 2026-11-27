# Deterministic JSONL Dataset Splits

`vantyx ai split <file> --out-dir <dir>` validates a local JSONL dataset and
writes deterministic train and held-out files. It is the offline counterpart of
`vantyx ai dataset`: the same validation gate runs first, then every record is
assigned by a seeded content hash.

The command is offline: it never downloads data, executes dataset content, or
contacts a provider. It does not modify the input file.

## Usage

```sh
vantyx ai split train.jsonl --out-dir splits
vantyx ai split train.jsonl --out-dir splits --train-ratio 0.8 --seed 7
vantyx ai split train.jsonl --out-dir splits --schema record.schema.json
vantyx ai split train.jsonl --out-dir splits --json
```

For an input named `train.jsonl`, the outputs are:

```text
splits/train.train.jsonl
splits/train.heldout.jsonl
```

Both files are created with `create_new`; the command refuses to overwrite an
existing file. If writing fails after one file was created, that file is
removed again, so a failed run does not leave a partial pair behind.

Text output:

```text
format: jsonl-split
input: train.jsonl
input size: 310 bytes
input sha256: deaef7...
records: 10
seed: 7
train ratio: 0.7
train: splits/train.train.jsonl (9 records, 270 bytes)
  sha256: 60f3e9...
held-out: splits/train.heldout.jsonl (1 records, 30 bytes)
  sha256: fd5cca...
warnings:
  1 blank line(s) were not copied
  6 CRLF line ending(s) were normalized to LF
  the UTF-8 byte-order mark was removed
```

`--json` prints one compact object with stable keys: `format`, `input`,
`input_sha256`, `input_bytes`, `records`, `seed`, `train_ratio`, `train`,
`held_out`, and `warnings`. Each part contains `path`, `records`, `bytes`, and
`sha256`.

## Determinism

Assignment is a function of the record bytes and the seed, not of the line
number or wall-clock state:

```text
bucket = SHA-256("vantyx-dataset-split-v1\0" || seed_le64 || record_bytes)
train  = top 32 bits of bucket < floor(train_ratio * 2^32)
```

Consequences:

- The same input bytes, seed, and ratio always produce the same split, on any
  machine and in any process.
- Reordering the input lines does not move a record between splits.
- Identical records always land in the same split. This is not deduplication:
  duplicates are still copied, but they cannot leak across the boundary. Run
  `vantyx ai dedup` first when the corpus must not contain exact duplicates.
- Changing `--seed` or `--train-ratio` changes the assignment. There is no
  random state and no hidden entropy.

The achieved ratio can differ from the requested ratio because whole records
are assigned independently. The report always states the actual counts.

## Validation gate

Before writing anything, the CLI runs the same inspection as
`vantyx ai dataset`:

- The input must be a regular file no larger than 256 MiB.
- Each non-blank line must be valid UTF-8 and one JSON object.
- With `--schema`, every record must satisfy the supported JSON Schema subset.

Invalid lines or schema-invalid records exit `1` and no split files are
created. `split` revalidates records while scanning and validates the schema a
second time in the scan pass, so a file that changes between inspection and
writing is rejected rather than silently split.

## Record bytes

The output is a normalized training corpus, not a byte-for-byte copy of the
input:

- A leading UTF-8 byte-order mark is removed.
- CRLF line endings are normalized to LF.
- Blank lines are not copied.
- Each record is written as its original bytes followed by one LF.

Warnings report each normalization that occurred. The output SHA-256 values
cover exactly the bytes written.

## Non-empty guarantee

A split needs at least two records. If the hash threshold would put every
record on one side, one record is moved deterministically to the other side:
the minimum-hash record for an empty train split, or the maximum-hash record
for an empty held-out split. A warning records the move. This keeps both files
usable for training and evaluation without introducing randomness.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Both split files were written. |
| `1` | The dataset has invalid lines or schema-invalid records, the schema or dataset cannot be read, an output file exists, or writing failed. |
| `64` | Usage error (missing path or `--out-dir`, bad `--train-ratio`, bad `--seed`, unknown option, missing or duplicate option value). |

## Limits

| Limit | Value |
| --- | --- |
| Dataset file size | 256 MiB |
| Bytes per line | 16 MiB |
| Schema file size | 16 MiB |
| Minimum records | 2 |

Memory use is constant with respect to the number of records. The input is read
twice: once to validate, hash, count, and choose the fallback record, and once
to write both outputs. Through the CLI the inspection pass reads it once more,
so the gate and the split cannot disagree about validity.

## What it does not do

- It does not shuffle either output; records keep their relative input order.
- It does not deduplicate records, count tokens, or stratify by label. Use
  `vantyx ai dedup` for exact byte-identical duplicate removal.
- It does not create a validation or test split beyond the single held-out set.
- It does not append to or overwrite existing files.
- It does not make the pair atomic across a crash; a failed run removes the
  files it created in that run.

## Related

- `vantyx ai dataset` — bounded JSONL inspection and schema validation.
- `vantyx ai dedup` — exact duplicate-record removal before splitting.
- `docs/ai/DATASET_INSPECTION.md` — the inspection format and limits.
- `docs/ai/DATASET_DEDUPLICATION.md` — exact record identity and limits.
- ADR-0043 — bounded JSONL dataset inspection.
- ADR-0046 — deterministic train/held-out splitting.
- ADR-0047 — exact dataset deduplication.
