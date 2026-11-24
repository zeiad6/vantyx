# Agent Assignments

The orchestrator owns integration, shared architecture documents, and the
final gate. Specialized subagents are advisory or scoped to non-overlapping
files. Only one subagent runs at a time until it finishes.

| Role | Agent | Current assignment | Status |
| --- | --- | --- | --- |
| Orchestrator / chief engineer | primary session | T-058–T-069 integration, documentation, gate, commit | Active |
| Compiler architecture review | `compiler-architect` | Native memory reclamation design (read-only) | Blocked on subagent capacity |
| Backend implementation | `backend-engineer` | No active assignment | Idle |
| Language implementation | `language-implementer` | No active assignment | Idle |
| Runtime/memory | `runtime-engineer` / `memory-model-engineer` | No active assignment | Idle |
| Type system | `type-system-engineer` | No active assignment | Idle |
| QA verification | `qa-verifier` | Independent T-062–T-066 verification when capacity returns | Blocked: subagent provider usage limit |
| Spec sync | `spec-sync-auditor` | Post-T-069 documentation drift audit | Queued |
| Security | `security-auditor` | Recursive-removal boundary review when requested | Idle |
| Tooling | `tooling-engineer` | No active assignment | Idle |
| Benchmarks | `benchmark-engineer` | No performance claim; no benchmark required | Idle |
| Docs | `docs-writer` | No active assignment | Idle |
| Release | `release-engineer` | CI watch after push | Queued |

File ownership for the T-058–T-069 change sets: the primary session owns all
modified files. No other agent may modify them concurrently.
