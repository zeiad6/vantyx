# Checkpoint — 2026-11-26 — T-100 exact JSONL dataset deduplication

## Task

- ID: T-100
- Title: Exact JSONL duplicate-record removal
- Status: VERIFIED locally
- Owner: orchestrator (implementation and verification)
- Priority: P3 (AI host tooling)
- Branch: `main`
- HEAD: `1aba7ba` (commit and push now authorized by the user)
- Working tree: T-078 through T-100 are pending in one working tree.

## Problem

`vantyx ai dataset` validated JSONL corpora and `vantyx ai split` partitioned
them, but neither removed duplicate records. Duplicates inflate training cost
and bias evaluation. Deduplication must be deterministic, must not rewrite
record content, must validate before writing, must never overwrite a file or
the input, and must keep memory bounded.

## Design

- Identity: exact normalized record bytes (first-line BOM, CRLF `\r`, and the
  line terminator excluded; blank lines are not records). JSON key order,
  whitespace, escapes, and numeric spelling are not canonicalized.
- Keep the first occurrence in input order.
- Digest: `SHA-256("vantyx-dataset-dedup-v1\0" || record_bytes)`.
- Two passes: scan/validate/count and compute the input SHA-256; then revalidate
  and write the first occurrence of each digest. Remove the output if the input
  changes or the counts disagree.
- Refuse an existing output and the input path; create the output with
  `create_new`.
- Cap the in-memory digest set at 1,000,000 distinct records.
- Report input/output bytes and SHA-256 values, total/unique/duplicate counts,
  and BOM/CRLF/blank-line warnings in text or `--json`.

## Changes

- `src/ai/dataset.rs`: `DedupSummary`, `MAX_DEDUP_UNIQUE_RECORDS`,
  `DEDUP_HASH_DOMAIN`, `deduplicate`, `deduplicate_with_schema`, `dedup_inner`,
  `DedupScan`, `scan_dedup_input`, `write_dedup`, `dedup_record_hash`; the
  split writer was renamed to `RecordWriter` and is shared by both features;
  four unit tests.
- `src/ai/mod.rs`: exports the dedup API.
- `src/ai_cli.rs`: `vantyx ai dedup` with `--out`, `--schema`, and `--json`;
  usage text and text/JSON reporting.
- `tests/ai.rs`: two CLI tests.
- `docs/ai/DATASET_DEDUPLICATION.md`: behavior, identity, limits, exit codes,
  and non-goals.
- `docs/decisions/ADR-0047-exact-dataset-deduplication.md`: decision record.
- `docs/ai/DATASET_INSPECTION.md`, `docs/ai/DATASET_SPLITTING.md`,
  `docs/ai/AI_ARCHITECTURE.md`, `docs/ai/AI_ROADMAP.md`, `README.md`,
  `docs/PROJECT_BASELINE.md`: cross-links and status.
- `TASKS.md`, `PROJECT_STATUS.md`, `AGENTS.md`, and `AGENT_STATE/`: task and
  state records.
- `.opencode/skills/`: the thirteen harness skills copied into the repository,
  making the documented project-local location real.
- `docs/AGENT_SKILLS.md`, `docs/PROJECT_BASELINE.md`: inventory corrected to
  thirteen skills with the exact `security-audit-engineering` ID and the
  `engineering-skill-orchestration` entry.
- `.github/workflows/ci.yml`: validates skill frontmatter, required sections,
  size, and unique IDs on every CI run.

## Tests

Four `src/ai/dataset.rs` unit tests:

- `dedup_keeps_first_occurrence_and_normalizes` — BOM/CRLF/blank input,
  duplicate records, first-occurrence output, exact output digest, warnings,
  input unchanged, JSON report.
- `dedup_rejects_invalid_input_and_refuses_overwrite` — invalid JSON leaves no
  output; an existing output is refused and preserved; the input path is
  refused; a missing input is an I/O error.
- `dedup_schema_gate_and_unique_record_limit` — schema violation leaves no
  output; a 2-record cap fails through `dedup_inner`.
- `dedup_rejects_empty_and_all_blank_inputs` — empty and all-blank datasets
  fail with no output.

Two `tests/ai.rs` CLI tests:

- `dataset_dedup_keeps_first_occurrence_and_reports_provenance` — JSON report,
  exact output, repeatability, input unchanged, overwrite refusal.
- `dataset_dedup_rejects_usage_invalid_data_and_schema_errors` — missing path,
  missing `--out`, unknown option, duplicate `--out`, same input/output,
  invalid data, and schema-invalid records.

## Verification

- `cargo test --lib ai::dataset` — 19 passed.
- `cargo test --test ai dataset_dedup` — 2 passed.
- `cargo test` — 707 passed (395 library unit + 15 CLI unit + 43 AI integration
  + 113 e2e + 141 native).
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo fmt --check --all` — clean.
- `cargo build --release` — succeeded.
- `python3 tools/test_differential_fuzz.py` — 7 passed.
- `python3 tools/differential_fuzz.py --iterations 500 --seed 7` — 500/500.
- All five `examples/*.vtx` built and ran natively with identical output.
- `/tmp/opencode/probe/diff6.py` — 31/31.
- `.opencode/skills/engineering-skill-orchestration/scripts/validate_skills.py
  --root .opencode/skills` — 13 skills valid.

## Files modified by this task

- `src/ai/dataset.rs`
- `src/ai/mod.rs`
- `src/ai_cli.rs`
- `tests/ai.rs`
- `docs/ai/DATASET_DEDUPLICATION.md`
- `docs/ai/DATASET_INSPECTION.md`
- `docs/ai/DATASET_SPLITTING.md`
- `docs/ai/AI_ARCHITECTURE.md`
- `docs/ai/AI_ROADMAP.md`
- `docs/decisions/ADR-0047-exact-dataset-deduplication.md`
- `docs/PROJECT_BASELINE.md`
- `README.md`
- `TASKS.md`
- `PROJECT_STATUS.md`
- `AGENTS.md`
- `AGENT_STATE/SESSION_STATE.md`
- `AGENT_STATE/ACTIVE_TASK.md`
- `AGENT_STATE/TASK_QUEUE.md`
- `AGENT_STATE/COMPLETED_TASKS.md`
- `AGENT_STATE/DECISIONS.md`
- `AGENT_STATE/AGENT_ASSIGNMENTS.md`
- `AGENT_STATE/CHECKPOINTS/2026-11-26-t100-dataset-dedup.md`

## Blockers

- Independent `qa-verifier` review is blocked by subagent provider usage limits.
- T-040 local-model evidence still needs a local inference server/model.
- GPU, tensor, SIMD, autodiff, and training evidence still needs hardware.
- Windows native support remains unverified; CI is Linux-only.

## Next exact steps

1. Run the final full gate once more after the documentation and state edits.
2. Review the working-tree diff for accidental or generated files.
3. Commit T-078 through T-100 together, push `main`, watch CI, and record the
   run ID.
4. Continue with tokenizer-aware dataset counts, shuffle, or label
   stratification as separate bounded tasks, or another P1/P2 candidate from
   the audit.
