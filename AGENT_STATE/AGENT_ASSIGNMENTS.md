# Agent Assignments

The orchestrator owns integration, shared architecture documents, and the
final gate. Specialized subagents are advisory or scoped to non-overlapping
files. Only one subagent runs at a time until it finishes.

| Role | Agent | Current assignment | Status |
| --- | --- | --- | --- |
| Orchestrator / chief engineer | primary session | T-073–T-074 verified locally (613-test gate); commit and CI pending | In progress |
| Compiler architecture review | `compiler-architect` | Native memory reclamation design (read-only) | Blocked on subagent capacity |
| Backend implementation | `backend-engineer` | No active assignment; T-070/T-071 are committed | Idle |
| Language implementation | `language-implementer` | No active assignment; T-074 checker fix reviewed by the primary session | Idle |
| Runtime/memory | `runtime-engineer` / `memory-model-engineer` | No active assignment | Idle |
| Type system | `type-system-engineer` | No active assignment | Idle |
| QA verification | `qa-verifier` | Independent T-062–T-066 and T-073–T-074 verification when capacity returns | Blocked: subagent provider usage limit |
| Spec sync | `spec-sync-auditor` | Post-T-073–T-074 documentation drift audit | Queued |
| Security | `security-auditor` | Recursive-removal boundary review when requested | Idle |
| Tooling | `tooling-engineer` | `vantyx lint` delivered; formatter remains blocked on comment preservation | Idle |
| Benchmarks | `benchmark-engineer` | No performance claim; no benchmark required | Idle |
| Docs | `docs-writer` | No active assignment; lint docs written by the primary session | Idle |
| Release | `release-engineer` | CI pending for the T-073–T-074 commit | Waiting |

File ownership for the T-073–T-074 change set: the primary session owns
`src/lint.rs`, `src/checker.rs`, `src/lib.rs`, `src/main.rs`, `tests/e2e.rs`,
`tests/native.rs`, `docs/linting.md`, `docs/decisions/ADR-0037-*`, and the
synchronized status documents. The T-070–T-072 change set is committed in
`a4fe86b`; the T-058–T-069 change sets remain in `e6ec597`, `4982ab3`,
`0491c63`, and `dfaad9b`. No other agent may modify these files concurrently.
