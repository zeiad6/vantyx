---
name: Multi-Agent Orchestration
description: Decompose large engineering tasks into owned, non-overlapping work; run parallel agents safely; prevent file conflicts; integrate in dependency order; require independent review; and checkpoint every handoff.
---

# Multi-Agent Orchestration

## When to use

- A task spans multiple files, subsystems, or disciplines and can be split without overlapping edits.
- You are coordinating subagents, worktrees, or specialized reviewers.
- A previous parallel attempt produced conflicts, duplicate work, or unverified integration.
- You need a clear ownership and review trail for a long-running change.

Do not use multiple agents for a small task that one agent can complete coherently. Coordination overhead is real.

## Required tools

- The agent/subagent tool and a way to give each worker a fresh, complete brief.
- Git worktrees or isolated branches for parallel file changes.
- A task board or state files with IDs, owners, status, and dependencies.
- `git diff`, test runners, linters, and the project's validation commands.
- A reviewer who is independent from the implementing agent.
- Checkpoint storage with timestamps and exact next steps.

## Safety constraints

- One task, one owner, one set of files. Never let two agents edit the same file concurrently.
- The orchestrator owns shared files: task board, status, architecture docs, ADRs, and checkpoints.
- Do not commit, push, rebase, or delete branches without explicit authorization.
- Treat subagent reports as claims. Verify with diffs, commands, and tests.
- Do not give subagents broader permissions or secrets than they need.
- Stop parallel work if the integration point becomes ambiguous or the tree is inconsistent.
- Preserve user work; inspect uncommitted changes before assigning or reverting anything.

## Workflow

1. **Recover state and define the goal.** Read the task board, `AGENTS.md`, status files, and `git status`. Write the goal, acceptance criteria, and non-goals. Do not restart completed work.

2. **Decompose by deliverable and file ownership.** Build a dependency DAG. Split by disjoint files or by read-only research versus write tasks. Shared files are serialized through the orchestrator.

3. **Write a complete brief for each task.** Include:
   - task ID, goal, and acceptance criteria;
   - exact files/directories owned;
   - constraints and relevant skills to load;
   - commands to run and evidence to return;
   - what must not be touched;
   - handoff format and checkpoint location.
   A fresh subagent has no conversation context.

4. **Choose the execution model.**
   - Parallel: disjoint files, independent verification.
   - Serial: shared files, dependent semantics, or high conflict risk.
   - Worktree isolation: risky or long-running changes that need a clean tree.
   Record the choice and why.

5. **Launch and monitor.** Check that each agent stays within ownership. Inspect diffs as they appear. If an agent edits an unowned file, stop it, preserve the diff, and re-brief before continuing.

6. **Integrate in dependency order.** Compile and test after each integration. Do not stack unverified changes. If two changes conflict semantically, resolve with a written decision, not by accepting both.

7. **Require independent review.** The reviewer must:
   - inspect the actual diff, not the summary;
   - run the acceptance commands independently;
   - verify the selected skill workflows were followed;
   - check tests, error paths, security, and performance claims;
   - report findings with evidence and a clear verdict.
   The implementing agent cannot approve its own work.

8. **Checkpoint every integration.** Record commit/tree state, files changed, commands run, results, blockers, and the next exact step. Use a timestamp and task ID.

9. **Close the task.** Update the task board and status with observed evidence. Move completed, blocked, and deferred items explicitly. Record residual risks and follow-ups.

10. **Recover from interruption.** Read the last checkpoint, compare it with the tree, re-run the narrow verification, and continue from the recorded next step. Do not redo completed work.

## Validation and acceptance criteria

- The task DAG and file ownership map exist before parallel work starts.
- No two agents edited the same file concurrently.
- Every task brief includes acceptance criteria and verification commands.
- Integration is tested after each merge into the working tree.
- An independent reviewer inspected the diff and ran the commands.
- Every completed task has a checkpoint and evidence-backed status.
- Blocked tasks record the exact missing prerequisite and owner.
- No destructive Git operation ran without authorization.

## Expected output

1. Task DAG and ownership map.
2. Briefs with scope, constraints, skills, and acceptance criteria.
3. Integration log in dependency order.
4. Independent review report with verdict and evidence.
5. Checkpoints with tree state and next steps.
6. Updated task board/status with residual risks.

## Troubleshooting

- **Agent edits an unowned file:** stop, inspect the diff, revert only that agent's unintended change if safe, and re-brief with explicit ownership.
- **Two agents conflict:** serialize the work; keep the first verified change, then have the second rebase manually. Never auto-merge semantic conflicts.
- **Duplicate work:** compare task IDs and file ownership; cancel the duplicate and record why.
- **Review disagrees with the report:** trust the observed commands and source; reopen the task.
- **Agent loses context:** provide the checkpoint, exact files, and next command; do not restart from scratch.
- **Provider capacity unavailable:** continue offline work, save state, and do not claim the review happened.
- **Integration breaks tests:** bisect by task, revert the smallest unverified change, and re-run the gate.

## Examples

```sh
# Ownership and state
git status --short
git worktree add ../wt-task-a -b task-a
git diff --stat main...task-a

# Independent verification
cargo test --workspace
git diff main...task-a -- src/module.rs tests/module.rs
```

```markdown
## Task T-042
Owner: agent-a
Files: src/parser.rs, tests/parser.rs
Skills: compiler-engineering
Acceptance: parser rejects malformed input with E0102; full suite passes
Handoff: diff, commands, raw output, checkpoint path
```

## Related skills and review

- Pair with `autonomous-recovery` for state and interruption handling.
- Pair with the domain skill for each task (`compiler-engineering`, `security-audit-engineering`, etc.).
- Reviewer checks: ownership respected, skill workflow followed, tests run, diff scoped, and claims evidenced.
