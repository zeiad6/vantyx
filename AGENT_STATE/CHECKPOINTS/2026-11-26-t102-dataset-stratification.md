# Checkpoint — T-102 label-stratified JSONL dataset splits

Date: 2026-11-26
Status: VERIFIED; pushed in `a331f2d`; CI run 45925745650 green

## What changed

- `src/ai/dataset.rs`: `StratumSummary`, `MAX_STRATIFIED_*` constants,
  `split_stratified`, `split_stratified_with_schema`, `stratified_split_inner`,
  `read_stratified_records`, `assign_strata`, `extreme_split_ordinal`,
  `write_stratified`, and `StratifiedLimits`. Labels are the compact JSON
  spelling of a top-level string, number, boolean, or null field. Each group is
  ordered by the first 64 bits of
  `SHA-256("vantyx-dataset-split-v1\0" || seed_le64 || record_bytes)` with
  record bytes as a tie-breaker; hashes are precomputed once and strata counted
  in one pass. Train count is `floor(n * ratio)`, clamped to `[1, n-1]` for
  `n >= 2`; singleton labels go to the majority split; one extreme-hash record
  moves only when a global split would be empty. Outputs preserve input order.
  `SplitSummary` gains `stratify` and `strata`, always serialized.
- Bounds: 64 MiB input, 1,000,000 records, 1,000 labels, 1,024-byte compact
  label, existing 16 MiB line and schema caps. Validation and the optional
  schema gate run before output creation; existing outputs and the input path
  are refused; partial outputs are removed.
- Shared split setup extracted into `validate_train_ratio`,
  `prepare_split_paths`, `create_split_files`, and `framing_warnings` without
  changing unstratified behavior.
- `src/ai/mod.rs`: exports `StratumSummary`, `split_dataset_stratified`, and
  `split_dataset_stratified_with_schema`.
- `src/ai_cli.rs`: `vantyx ai split ... --stratify <field>`, text and JSON
  strata output, usage text, and a corrected `AI_E_FORMAT` help line that no
  longer claims every input is a model file.
- `tests/ai.rs`: two stratified CLI tests; existing split tests assert
  `stratify: null` and `strata: []` for the unstratified path.
- Docs: `docs/ai/DATASET_STRATIFICATION.md`, ADR-0049,
  `docs/ai/DATASET_SPLITTING.md`, `docs/ai/AI_ARCHITECTURE.md`,
  `docs/ai/AI_ROADMAP.md`, README, TASKS.md T-102.

## Verified commands (2026-11-26)

- `cargo test --lib stratified` — 3 passed.
- `cargo test --test ai dataset_split` — 4 passed.
- `cargo test` — 718 passed (402 library unit + 15 CLI unit + 47 AI integration
  + 113 e2e + 141 native).
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check --all`
  clean; `cargo build --release` succeeded.
- All five examples built natively and matched `vantyx run` output with `cmp`.
- `python3 tools/test_differential_fuzz.py` — 7 passed;
  `python3 tools/differential_fuzz.py --iterations 100 --seed 1` — 100/100;
  supplemental `--iterations 500 --seed 11` audit — 500/500.
- Skill validator: 13 skills valid; link checker: 117 links OK.

## Blockers and limits

- Independent `qa-verifier` review was unavailable (subagent provider
  free-usage limit), so this is local verification, not independent review.
- One stratification field only; no label weights, multi-field keys, or
  tokenizer-aware balancing; no output shuffle or deduplication; in-memory
  grouping; global ratio can differ from the requested ratio; `1.0` and `1`
  share a spelling while `"1"` is distinct; labels are not Unicode-normalized;
  the pair is not atomic across a crash.

## Next

T-102 is committed as `a331f2d`, pushed, and CI run 45925745650 passed; the
T-101 status commit `5d5655b` also passed CI run 45923860569. Continue with
tokenizer-aware dataset counts or another queued P1/P2 task.
