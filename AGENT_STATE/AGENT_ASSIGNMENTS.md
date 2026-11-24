# Agent Assignments

The orchestrator owns integration, shared architecture documents, and the
final gate. Specialized subagents are advisory or scoped to non-overlapping
files. Only one subagent runs at a time until it finishes.

| Role | Agent | Current assignment | Status |
| --- | --- | --- | --- |
| Orchestrator / chief engineer | primary session | T-075 result-enum propagation verified locally (621-test gate, 180/180 on three seeds); commit and CI pending | In progress |
| Compiler architecture review | `compiler-architect` | Native memory reclamation design (read-only) | Blocked on subagent capacity |
| Backend implementation | `backend-engineer` | No active assignment; T-070/T-071 are committed | Idle |
| Language implementation | `language-implementer` | T-075 `?` propagation implemented by the primary session; independent review when capacity returns | Idle |
| Runtime/memory | `runtime-engineer` / `memory-model-engineer` | No active assignment | Idle |
| Type system | `type-system-engineer` | No active assignment | Idle |
| QA verification | `qa-verifier` | Independent T-062–T-066, T-072 expansion, and T-073–T-074 verification when capacity returns | Blocked: subagent provider usage limit |
| Spec sync | `spec-sync-auditor` | Post-T-072/T-073–T-074 documentation drift audit | Queued |
| Security | `security-auditor` | Recursive-removal boundary review when requested | Idle |
| Tooling | `tooling-engineer` | `vantyx lint` delivered; formatter remains blocked on comment preservation; fuzzer shrinking/sanitizers are ready follow-ups | Idle |
| Benchmarks | `benchmark-engineer` | No performance claim; no benchmark required | Idle |
| Docs | `docs-writer` | No active assignment; fuzzer, lint, and audit docs written by the primary session | Idle |
| Release | `release-engineer` | CI pending for the T-075 commit; runs 45499019615, 45496041422, 45493833079, and 45488725812 are green | Waiting |

File ownership for T-075: the primary session owns `src/token.rs`,
`src/lexer.rs`, `src/ast.rs`, `src/parser.rs`, `src/checker.rs`,
`src/interp.rs`, `src/ir.rs`, `src/codegen_c.rs`, `src/lint.rs`,
`tests/e2e.rs`, `tests/native.rs`, `tools/differential_fuzz.py`, the
specification, ADR-0038, and the synchronized status/research documents. The
T-072 expansion is committed in `53a8b82`; the T-073–T-074 change set is
committed in `5e1092f`; the T-070–T-071 fixes and the original fuzzer are in
`a4fe86b`; the T-058–T-069 change sets remain in `e6ec597`, `4982ab3`,
`0491c63`, and `dfaad9b`. No other agent may modify these files concurrently.
