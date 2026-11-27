# Checkpoint — 2026-11-26 — T-098 deterministic train/held-out dataset splits

## Task

- ID: T-098
- Title: Deterministic train/held-out dataset splits
- Status: VERIFIED locally
- Owner: orchestrator (implementation and verification)
- Priority: P3 (AI host tooling)
- Branch: `main`
- HEAD: `1aba7ba` (no commit, push, or history rewrite authorized)
- Working tree: T-078 through T-098 remain uncommitted; many pre-existing
  modifications from earlier tasks are present and must be preserved.

## Problem

`vantyx ai dataset` validated JSONL corpora but did not split them. Training or
evaluation experiments therefore needed an external script, could not be
reproduced from the repository, and risked overwriting an existing corpus.
A split must be deterministic, independent of line order, bounded in memory,
refuse overwrites, and reject invalid or schema-invalid data before writing.

## Design

- Assignment: `SHA-256("vantyx-dataset-split-v1\0" || seed_le64 ||
  record_bytes)`, compared by its top 32 bits against
  `floor(train_ratio * 2^32)`.
- Determinism: same input bytes, seed, and ratio always produce the same split;
  reordering lines does not move a record; identical records share a split.
- Outputs: `<stem>.train.jsonl` and `<stem>.heldout.jsonl`, opened with
  `create_new`. Files created by a failed run are removed.
- Validation: the CLI runs the existing dataset inspection first, including
  `--schema`; `split_with_schema` validates again during the scan. The write
  pass verifies the input digest and record count.
- Normalization: a leading BOM, CRLF endings, and blank lines are removed from
  the outputs with warnings. The input is never modified.
- Non-empty guarantee: with at least two records, if a threshold leaves one
  side empty, the minimum- or maximum-hash record is moved deterministically
  and a warning records it.
- Report: actual counts, bytes, output SHA-256 values, and warnings in text or
  `--json`.

## Changes

- `src/ai/dataset.rs`: `split`, `split_with_schema`, `SplitPart`,
  `SplitSummary`, `DEFAULT_TRAIN_RATIO`, `record_bytes` helper shared with
  inspection, two-pass scan/write, output cleanup, normalization warnings.
- `src/ai/mod.rs`: exports the split API and default ratio.
- `src/ai_cli.rs`: `vantyx ai split` with `--out-dir`, `--train-ratio`,
  `--seed`, `--schema`, and `--json`; shared dataset summary printing and
  validity helpers; usage text.
- `tests/ai.rs`: two CLI tests.
- `docs/ai/DATASET_SPLITTING.md`: new behavior, determinism, limits, and
  non-goals.
- `docs/ai/DATASET_INSPECTION.md`, `docs/ai/AI_ARCHITECTURE.md`,
  `docs/ai/AI_ROADMAP.md`, `docs/PROJECT_BASELINE.md`, `README.md`: cross-links
  and status.
- `docs/decisions/ADR-0046-deterministic-dataset-splits.md`: decision record.
- `TASKS.md`, `PROJECT_STATUS.md`, `AGENTS.md`, and `AGENT_STATE/`: task and
  state records.

## Tests

Five `src/ai/dataset.rs` unit tests:

- `split_is_deterministic_and_partitions_records` — BOM/CRLF/blank input, two
  identical runs, exact partition, LF outputs, warnings, input unchanged.
- `split_seed_changes_assignment_and_rejects_bad_ratios` — seed changes both
  digests; 0, 1, negative, NaN, and infinity rejected; no output directory.
- `split_requires_two_records_and_refuses_overwrite` — one record rejected;
  second run refused; existing file unchanged; file-as-directory rejected.
- `split_rejects_invalid_lines_before_writing` — invalid JSON and non-object
  lines rejected; no files.
- `split_schema_gate_and_non_empty_fallback` — schema violation rejected; high
  and low ratios still produce two non-empty splits with a move warning.

Two `tests/ai.rs` CLI tests:

- `dataset_split_is_deterministic_and_reports_provenance` — JSON report,
  partition, repeatability, seed change, LF outputs.
- `dataset_split_rejects_usage_invalid_data_and_overwrite` — missing
  `--out-dir`, bad ratio, bad seed, unknown option, invalid lines,
  schema-invalid records, overwrite refusal.

## Verification

- `cargo test --lib ai::dataset` — 15 passed.
- `cargo test --test ai dataset_split` — 2 passed.
- `cargo test` — 700 passed (391 library unit + 15 CLI unit + 41 AI integration
  + 113 e2e + 140 native).
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo fmt --check --all` — clean after formatting.
- `cargo build --release` — succeeded.
- `python3 tools/test_differential_fuzz.py` — 7 passed.
- `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100.
- All five `examples/*.vtx` built and ran natively with identical output.
- Manual smoke: 10-record BOM/CRLF dataset, ratio 0.7 seed 7 → 9/1; second run
  refused overwrite; seed 8 changed both digests; `--train-ratio 1` exited 64;
  one record exited 1; invalid JSON and schema-invalid records exited 1 with no
  split files; ratios 0.999999 and 0.000001 each produced two non-empty splits
  with a move warning.

## Files modified by this task

- `src/ai/dataset.rs`
- `src/ai/mod.rs`
- `src/ai_cli.rs`
- `tests/ai.rs`
- `docs/ai/DATASET_SPLITTING.md`
- `docs/ai/DATASET_INSPECTION.md`
- `docs/ai/AI_ARCHITECTURE.md`
- `docs/ai/AI_ROADMAP.md`
- `docs/PROJECT_BASELINE.md`
- `docs/decisions/ADR-0046-deterministic-dataset-splits.md`
- `README.md`
- `TASKS.md`
- `PROJECT_STATUS.md`
- `AGENTS.md`
- `AGENT_STATE/SESSION_STATE.md`
- `AGENT_STATE/ACTIVE_TASK.md`
- `AGENT_STATE/TASK_QUEUE.md`
- `AGENT_STATE/COMPLETED_TASKS.md`
- `AGENT_STATE/DECISIONS.md`
- `AGENT_STATE/RECOVERY.md`
- `AGENT_STATE/AGENT_ASSIGNMENTS.md`
- `AGENT_STATE/CHECKPOINTS/2026-11-26-t098-dataset-splits.md`

## Blockers

- Commit, push, and CI are unauthorized.
- Independent `qa-verifier` review is blocked by subagent provider usage limits.
- T-040 local-model evidence still needs a local inference server/model.
- GPU, tensor, SIMD, autodiff, and training evidence still needs hardware.
- Windows native support remains unverified; CI is Linux-only.

## Next exact steps

1. Continue the AI dataset track with deduplication and tokenizer-aware counts
   as separate bounded tasks, or pivot to a P1 core-language task if the audit
   shows a higher-priority defect. Add a failing regression test before fixing
   any confirmed mismatch.
2. Re-run the full gate after each change.
3. Request independent review when subagent capacity returns.
4. Do not commit or push without explicit user authorization.
