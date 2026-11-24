# Agent Assignments

The orchestrator owns integration, shared architecture documents, and the
final gate. Specialized subagents are advisory or scoped to non-overlapping
files. Only one subagent runs at a time until it finishes.

| Role | Agent | Current assignment | Status |
| --- | --- | --- | --- |
| Orchestrator / chief engineer | primary session | T-054 integration, documentation, gate, commit | Active |
| Compiler architecture review | `compiler-architect` | Native memory reclamation design (read-only) | Blocked on subagent capacity |
| Backend implementation | `backend-engineer` | No active assignment | Idle |
| Language implementation | `language-implementer` | No active assignment | Idle |
| Runtime/memory | `runtime-engineer` / `memory-model-engineer` | No active assignment | Idle |
| Type system | `type-system-engineer` | No active assignment | Idle |
| QA verification | `qa-verifier` | Independent T-054 verification when requested | Idle |
| Spec sync | `spec-sync-auditor` | Post-T-054 documentation drift audit | Queued |
| Security | `security-auditor` | No active assignment | Idle |
| Tooling | `tooling-engineer` | No active assignment | Idle |
| Benchmarks | `benchmark-engineer` | RAG incremental plumbing baseline recorded | Idle |
| Docs | `docs-writer` | No active assignment | Idle |
| Release | `release-engineer` | CI watch after push | Queued |

File ownership for the T-054 change set: the primary session owns all modified
files. No other agent may modify them concurrently.
