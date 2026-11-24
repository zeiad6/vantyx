# Agent Assignments

The orchestrator owns integration, shared architecture documents, and the
final gate. Specialized subagents are advisory or scoped to non-overlapping
files. Only one subagent runs at a time until it finishes.

| Role | Agent | Current assignment | Status |
| --- | --- | --- | --- |
| Orchestrator / chief engineer | primary session | T-072 fuzzer expansion verified locally (160/160 on three seeds); commit and CI pending | In progress |
| Compiler architecture review | `compiler-architect` | Native memory reclamation design (read-only) | Blocked on subagent capacity |
| Backend implementation | `backend-engineer` | No active assignment; T-070/T-071 are committed | Idle |
| Language implementation | `language-implementer` | No active assignment; T-074 checker fix is committed | Idle |
| Runtime/memory | `runtime-engineer` / `memory-model-engineer` | No active assignment | Idle |
| Type system | `type-system-engineer` | No active assignment | Idle |
| QA verification | `qa-verifier` | Independent T-062–T-066, T-072 expansion, and T-073–T-074 verification when capacity returns | Blocked: subagent provider usage limit |
| Spec sync | `spec-sync-auditor` | Post-T-072/T-073–T-074 documentation drift audit | Queued |
| Security | `security-auditor` | Recursive-removal boundary review when requested | Idle |
| Tooling | `tooling-engineer` | `vantyx lint` delivered; formatter remains blocked on comment preservation; fuzzer shrinking/sanitizers are ready follow-ups | Idle |
| Benchmarks | `benchmark-engineer` | No performance claim; no benchmark required | Idle |
| Docs | `docs-writer` | No active assignment; fuzzer, lint, and audit docs written by the primary session | Idle |
| Release | `release-engineer` | CI pending for the T-072 expansion commit and the local `806a822` audit commit | Waiting |

File ownership for the T-072 expansion: the primary session owns
`tools/differential_fuzz.py`, `.github/workflows/ci.yml`, and the synchronized
status/research documents. The T-073–T-074 change set is committed in
`5e1092f`; the T-070–T-071 fixes and the original fuzzer are in `a4fe86b`;
the T-058–T-069 change sets remain in `e6ec597`, `4982ab3`, `0491c63`, and
`dfaad9b`. No other agent may modify these files concurrently.
