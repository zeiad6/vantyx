# Checkpoint — T-101 deterministic JSONL dataset shuffle

Date: 2026-11-26
Status: VERIFIED; pushed in `c54c61c`; CI run 45923211883 green

## What changed

- `src/ai/dataset.rs`: `ShuffleSummary`, `shuffle`, `shuffle_with_schema`,
  `shuffle_inner`, `read_shuffle_records`, `write_shuffle`, and
  `shuffle_record_digest`. Ordering key:
  `SHA-256("vantyx-dataset-shuffle-v1\0" || seed_le64 || record_bytes)` with
  record bytes as tie-breaker. Bounds: 64 MiB input, 1,000,000 records,
  existing 16 MiB line cap. Validation and optional schema gate run before the
  output is created; existing output and input paths are refused; failed output
  is removed; record count is verified.
- `src/ai/mod.rs`: exports `ShuffleSummary`, `shuffle_dataset`, and
  `shuffle_dataset_with_schema`.
- `src/ai_cli.rs`: `vantyx ai shuffle <file> --out <file> [--seed <n>]
  [--schema <file>] [--json]`, with `--out`, `--seed`, and `--schema`
  duplicate/usage checks.
- `tests/ai.rs`: two CLI integration tests.
- Docs: `docs/ai/DATASET_SHUFFLING.md`, ADR-0048, `docs/ai/AI_ARCHITECTURE.md`,
  `docs/ai/AI_ROADMAP.md`, README, TASKS.md T-101.

## Verified commands

- `cargo test --lib ai::dataset` — 23 passed.
- `cargo test --test ai dataset_shuffle` — 2 passed.
- `cargo test` — 713 passed (399 library unit + 15 CLI unit + 45 AI integration
  + 113 e2e + 141 native).
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check --all`
  clean; `cargo build --release` succeeded.
- `python3 tools/test_differential_fuzz.py` — 7 passed;
  `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100.
- All five examples built and ran natively with identical output.
- Manual smoke: two `--seed 7` shuffles produced byte-identical files and
  SHA-256 values; JSON report fields were present.
- Skill validator: 13 skills valid.

## Next

T-101 is committed as `c54c61c`, pushed, and CI run 45923211883 passed. Continue
with the next bounded dataset task (tokenizer-aware counts or label
stratification) or another queued P1/P2 task.
