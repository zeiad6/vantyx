# Agent Assignments

The orchestrator owns integration, shared architecture documents, and the
final gate. Specialized subagents are advisory or scoped to non-overlapping
files. Only one subagent runs at a time until it finishes.

| Role | Agent | Current assignment | Status |
| --- | --- | --- | --- |
| Orchestrator / chief engineer | primary session | T-070–T-072 verified locally (584-test gate); commit and CI pending | In progress |
| Compiler architecture review | `compiler-architect` | Native memory reclamation design (read-only) | Blocked on subagent capacity |
| Backend implementation | `backend-engineer` | No active assignment; T-070/T-071 reviewed by the primary session | Idle |
| Language implementation | `language-implementer` | No active assignment | Idle |
| Runtime/memory | `runtime-engineer` / `memory-model-engineer` | No active assignment | Idle |
| Type system | `type-system-engineer` | No active assignment | Idle |
| QA verification | `qa-verifier` | Independent T-062–T-066 and T-070–T-072 verification when capacity returns | Blocked: subagent provider usage limit |
| Spec sync | `spec-sync-auditor` | Post-T-070–T-072 documentation drift audit | Queued |
| Security | `security-auditor` | Recursive-removal boundary review when requested | Idle |
| Tooling | `tooling-engineer` | `vantyx lint` (T-019 first half) is the next ready task | Queued |
| Benchmarks | `benchmark-engineer` | No performance claim; no benchmark required | Idle |
| Docs | `docs-writer` | No active assignment | Idle |
| Release | `release-engineer` | CI pending for the T-070–T-072 commit | Waiting |

File ownership for the T-070–T-072 change set: the primary session owns
`src/codegen_c.rs`, `tests/native.rs`, `tools/differential_fuzz.py`,
`.github/workflows/ci.yml`, `docs/decisions/ADR-0036-*`, and the synchronized
status documents. The T-058–T-069 change sets remain committed in `e6ec597`,
`4982ab3`, `0491c63`, and `dfaad9b`. No other agent may modify these files
concurrently.
