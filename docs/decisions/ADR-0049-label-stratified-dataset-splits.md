# ADR-0049 — Label-stratified JSONL dataset splits

Status: Accepted
Date: 2026-11-26
Task: T-102

## Context

`vantyx ai split` assigns records independently by a seeded hash. That is
deterministic and order-independent, but it does not control label
proportions: a small or imbalanced label can land mostly or entirely in one
split, which makes evaluation noisy and sometimes impossible for that label.
Training and evaluation corpora commonly need every label represented in both
splits in approximately the requested ratio.

A stratified split must stay compatible with the existing contract:

- deterministic for the same input, seed, and ratio;
- validated before any output exists;
- never overwrite an existing file;
- report actual counts and provenance;
- bounded memory and explicit limits;
- no new dependency.

The existing split uses a per-record hash and a global threshold. Keeping that
assignment for the unstratified path avoids changing established behavior.

## Decision

Add `--stratify <field>` to `vantyx ai split`, backed by
`split_stratified` / `split_stratified_with_schema`, with these rules:

1. **Label source.** The named field must exist in every record and hold a JSON
   string, number, boolean, or null. The label is the compact JSON spelling of
   that value. Arrays, objects, missing fields, labels over 1,024 bytes, and
   more than 1,000 distinct labels are format errors. The parser already rejects
   duplicate object keys.
2. **Per-label exact counts.** Within each label group, records are ordered by
   the first 64 bits of
   `SHA-256("vantyx-dataset-split-v1\0" || seed_le64 || record_bytes)` with the
   record bytes as a tie-breaker. For `n` records and train ratio `r`, the train
   count is `floor(n * r)`, clamped to `[1, n - 1]` when `n >= 2`.
3. **Singleton labels.** A one-record label cannot be divided. It goes to train
   when `r >= 0.5` and to held-out otherwise.
4. **Global non-empty fallback.** If the per-label rules leave one split empty,
   move the minimum-hash record to train or the maximum-hash record to held-out,
   and report a warning. This mirrors the unstratified split's guarantee.
5. **Input order preserved.** Records are written in their original relative
   order within each output. Stratification changes membership, not ordering.
   Use `vantyx ai shuffle` first when a shuffled corpus is required.
6. **Validation before writing.** Reuse the inspection gate and optional schema
   validation. The stratified pass re-reads and revalidates every record, then
   checks the label. No output is kept if any record or limit fails.
7. **Bounded in-memory grouping.** Hold the dataset in memory under a 64 MiB
   input cap, 1,000,000 records, 1,000 labels, and the existing 16 MiB line cap.
   The whole record set is needed to group and order labels; the unstratified
   path remains streaming and keeps its 256 MiB cap.
8. **Report the strata.** Add `stratify` and a label-sorted `strata` array
   (`label`, `records`, `train`, `held_out`) to the existing split report.

The domain separator remains `vantyx-dataset-split-v1\0`, so a record's hash
position within its label group is the same hash used by the unstratified
assignment.

## Consequences

- Every multi-record label contributes at least one record to each split, and
  singleton labels are assigned by the majority rule.
- The achieved train fraction can differ slightly from `r`: every label rounds
  down, and multi-record labels are clamped. The report states actual counts.
- Assignment is a function of the record set, seed, ratio, labels, and field
  name, not of input line order. Adding records can change a label group's count
  and therefore move records within that group.
- Integral floats and integers share a compact label spelling (`1.0` labels as
  `1`); string `"1"` is a distinct label.
- The lower 64 MiB cap is an explicit consequence of holding records in memory
  plus one label id and one 8-byte split hash per record. External-memory
  grouping is deferred.
- The unstratified path is behaviorally unchanged; only shared setup helpers
  were extracted.
- No new dependency, error code, or runtime type is introduced.

## Alternatives considered

- **Apply the global hash threshold inside each label.** Simple, but a label
  with few records can still produce zero train or zero held-out records, which
  is the problem stratification is meant to solve.
- **Exact counts by random assignment.** Requires a specified PRNG and makes
  reproducibility depend on generator details. The seeded content hash already
  provides a stable order without new state.
- **Round to nearest instead of floor.** Produces a closer global ratio for some
  inputs but makes per-label counts less predictable and can exceed `n` without
  extra clamping.
- **Assign singletons randomly.** Adds entropy and can make a label absent from
  the split that the ratio would favor. The majority rule is deterministic and
  explainable.
- **Multiple stratification fields or weights.** More expressive, but requires a
  composite-key contract and a clear precedence rule. Deferred.
- **Token-aware balancing.** Requires a model tokenizer and a different cost
  model. The project does not ship a tokenizer; token counts remain a separate
  future task.
- **Streaming external-memory grouping.** Avoids the 64 MiB cap but adds
  temporary files, cleanup, and a larger implementation. The bounded in-memory
  design is an honest first step.
