# Agent Assignments

The orchestrator owns integration, shared architecture documents, and the
final gate. Specialized subagents are advisory or scoped to non-overlapping
files. Parallel delegation was attempted on 2026-11-27 but every child session
failed with `Free usage limit reached`; the primary session executes the
selected tasks directly and does not claim independent review.

| Role | Agent | Current assignment | Status |
| --- | --- | --- | --- |
| Orchestrator / chief engineer | primary session | T-118 linter reachability, T-119 sanitized differential fuzz in CI, and T-120 native reclamation design after T-117 list reversal; T-114 through T-120 commit and CI pending | In progress |
| Compiler architecture review | `compiler-architect` | T-120 native memory reclamation design written by the primary session; independent review when capacity returns | Blocked on subagent capacity |
| Backend implementation | `backend-engineer` | No active assignment; T-070/T-071 are committed | Idle |
| Language implementation | `language-implementer` | T-075 `?` propagation implemented by the primary session; independent review when capacity returns | Idle |
| Runtime/memory | `runtime-engineer` / `memory-model-engineer` | No active assignment | Idle |
| Type system | `type-system-engineer` | No active assignment | Idle |
| QA verification | `qa-verifier` | Independent T-062–T-066 and T-072–T-117 verification when capacity returns | Blocked: subagent provider usage limit |
| Spec sync | `spec-sync-auditor` | Post-T-117 documentation drift audit | Blocked: subagent provider usage limit |
| Security | `security-auditor` | T-117 `reverse` and native list audit | Blocked: subagent provider usage limit |
| Tooling | `tooling-engineer` | T-118 `L0003` all-terminating `if`/`match`; delegated child failed, primary session executing | In progress |
| Benchmarks | `benchmark-engineer` | No performance claim; no benchmark required | Idle |
| Docs | `docs-writer` | No active assignment; fuzzer, lint, and audit docs written by the primary session | Idle |
| Release | `release-engineer` | T-119 sanitized differential fuzz in CI; delegated child failed, primary session executing | In progress |

File ownership for T-095: the primary session owns `src/ai/schema.rs`,
`src/ai/json.rs`, `src/ai/extract.rs`, `src/ai/tools.rs`, `tests/ai.rs`,
`docs/ai/DATASET_INSPECTION.md`,
`docs/decisions/ADR-0045-json-schema-conformance-and-preflight.md`, and the
synchronized status/research/state documents. File ownership for T-094: the
primary session owns `src/ai/dataset.rs`, `src/ai/mod.rs`, `src/ai_cli.rs`,
`tests/ai.rs`, `docs/ai/DATASET_INSPECTION.md`,
`docs/decisions/ADR-0044-bounded-dataset-schema-validation.md`, and the
synchronized status/research/state documents. File ownership for T-093: the primary session owns `.opencode/skills/`,
`docs/AGENT_SKILLS.md`, `AGENTS.md`, `RESEARCH/TOOL_INVENTORY.md`, and the
synchronized status/state documents. File ownership for T-092: the primary
session owns `src/ai/dataset.rs`, `src/ai/mod.rs`, `src/ai_cli.rs`,
`tests/ai.rs`, `docs/ai/DATASET_INSPECTION.md`, `docs/ai/AI_ARCHITECTURE.md`,
`docs/ai/AI_ROADMAP.md`, `examples/ai/train.jsonl`, `examples/ai/README.md`,
`README.md`, `docs/decisions/ADR-0043-bounded-jsonl-dataset-inspection.md`, and
the synchronized status/research/state documents. File ownership for T-091: the primary session owns `src/ai/model.rs`,
`src/ai/mod.rs`, `src/ai_cli.rs`, `tests/ai.rs`,
`docs/ai/MODEL_INSPECTION.md`, `docs/ai/AI_ARCHITECTURE.md`,
`docs/ai/AI_ROADMAP.md`,
`docs/decisions/ADR-0042-safetensors-shard-index-aggregation.md`, and the
synchronized status/research/state documents. File ownership for T-090: the primary session owns `src/ai/model.rs`,
`src/ai/mod.rs`, `src/ai_cli.rs`, `tests/ai.rs`,
`docs/ai/MODEL_INSPECTION.md`, `docs/ai/AI_ARCHITECTURE.md`,
`docs/ai/AI_ROADMAP.md`, `docs/decisions/ADR-0041-offline-model-inspection.md`,
and the synchronized status/research/state documents. File ownership for
T-089: the primary session owns `src/fs_safety.rs`, `src/interp.rs`,
`src/codegen_c.rs`, `tests/e2e.rs`, `tests/native.rs`,
`docs/language_specification.md`, `docs/decisions/ADR-0029-rename-and-copy.md`,
and the synchronized status/research/state documents. File ownership for T-087: the primary session owns `src/interp.rs`,
`tests/e2e.rs`, `tests/native.rs`, `tools/differential_fuzz.py`,
`docs/language_specification.md`, `docs/decisions/ADR-0040-lexical-function-scope.md`,
and the synchronized status/research/state documents. File ownership for
T-086: the primary session owns `src/codegen_c.rs`,
`tests/native.rs`, `tests/e2e.rs`, `docs/language_specification.md`,
`docs/decisions/ADR-0030-lexical-path-builtins.md`, and the synchronized
status/research/state documents. File ownership for T-098: the primary session
owns `src/ai/dataset.rs`, `src/ai/mod.rs`, `src/ai_cli.rs`, `tests/ai.rs`,
`docs/ai/DATASET_SPLITTING.md`, `docs/ai/DATASET_INSPECTION.md`,
`docs/ai/AI_ARCHITECTURE.md`, `docs/ai/AI_ROADMAP.md`,
`docs/decisions/ADR-0046-deterministic-dataset-splits.md`, and the synchronized
status/state documents. File ownership for T-097: the primary session
owns `src/codegen_c.rs`, `tests/native.rs`, and the synchronized status/state
documents. File ownership for T-096: the primary session
owns `src/value.rs`, `src/interp.rs`, `src/codegen_c.rs`, `tests/native.rs`, and
the synchronized status/state documents. File ownership for T-085: the primary session
owns `src/lint.rs`,
`tests/e2e.rs`, `docs/linting.md`, `docs/language_specification.md`,
`docs/decisions/ADR-0037-static-lints.md`, and the synchronized
status/research/state documents. T-082/T-083/T-084 own
`src/codegen_c.rs`, `tests/native.rs`, `tests/e2e.rs`,
`tools/differential_fuzz.py`, `docs/language_specification.md`, and the
synchronized status/research/state documents. T-081 owns the `contains`
lowering and cross-compiler test; T-080 owns `type` lowering in
`src/codegen_c.rs`, its e2e/native tests, and fuzzer cases; T-079 owns the
earlier `print`/float changes in `src/interp.rs` and `src/codegen_c.rs` and
ADR-0039; T-078 owns the earlier numeric-conversion changes in the same files.
T-077 is committed in
`1aba7ba`; T-076 in `de0bac7`; T-075 in `66b4efc`; the T-072 expansion is
committed in `53a8b82`; the T-073–T-074 change set is committed in `5e1092f`;
the T-070–T-071 fixes and the original fuzzer are in `a4fe86b`; the
T-058–T-069 change sets remain in `e6ec597`, `4982ab3`, `0491c63`, and
`dfaad9b`. No other agent may modify these files concurrently.
