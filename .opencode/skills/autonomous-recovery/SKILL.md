---
name: Autonomous Recovery
description: Survive interruptions, context resets, provider outages, quota limits, and long-running tasks by saving verifiable state, validating checkpoints, backing off safely, resuming without duplicate work, and handing off exact next steps.
---

# Autonomous Recovery

## When to use

- Starting or resuming a long task, especially after a context reset or model switch.
- A provider, network, tool, or quota failure interrupts execution.
- Before ending a session, compacting context, or handing work to another agent.
- A checkpoint or status file may be stale, incomplete, or inconsistent with the tree.

Do not use this skill to bypass quotas, rate limits, authentication, or authorization. Recovery must respect the same safety boundaries as the original task.

## Required tools

- Git and a state directory (`AGENT_STATE/`, `.agent/`, or the project's existing equivalent).
- A task board with IDs, status, owner, and next action.
- Command logs or captured output for verification.
- A clock and backoff implementation, or manual scheduling with recorded times.
- The project's narrow and broad validation commands.
- A handoff template that a fresh agent can follow without the original conversation.

## Safety constraints

- Do not trust a checkpoint blindly. Compare it with the actual tree, commit, and test output.
- Do not redo destructive operations or repeat side effects whose completion is uncertain.
- Do not bypass quotas, rotate credentials, or create new accounts to continue.
- Do not discard user work to make the state match the checkpoint.
- Redact secrets, tokens, prompts, and personal data from state files.
- Stop retrying when the failure is deterministic or the retry budget is exhausted.
- Record what was not verified; never convert an assumption into a completed status.

## Workflow

1. **Recover the source of truth.** Read the project rules, task board, active-task file, last checkpoint, and recent logs. Then run:
   ```sh
   git status --short
   git log -1 --oneline
   git diff --stat
   git stash list
   ```
   Record the actual tree state before trusting any note.

2. **Validate the checkpoint.** Compare its recorded commit, branch, dirty files, test results, and next step with reality. Classify:
   - `MATCH`: checkpoint is consistent;
   - `STALE`: tree advanced or changed;
   - `INCOMPLETE`: missing evidence or next step;
   - `CONFLICT`: checkpoint and tree disagree.
   Trust the tree and observed commands over the note.

3. **Re-run the narrow verification.** Execute the smallest command that proves the active task's current state. Then run the broader gate only if the narrow check passes and the tree is coherent.

4. **Reconstruct missing state from evidence.** Use `git diff`, test logs, shell history if permitted, task-board entries, and file timestamps. Write the reconstructed state immediately; do not rely on memory.

5. **Resume from the exact next step.** Use the checkpoint's command, file, and acceptance criterion. If the next step is ambiguous, stop and write a precise one before continuing.

6. **Prevent duplicate work.** Check completed-task records, open branches, worktrees, and file ownership before starting. Search for the task ID and the intended file changes. If duplicate work exists, reconcile instead of racing.

7. **Handle network and provider failures with bounded backoff.**
   - Honor `Retry-After` when present.
   - Otherwise use exponential backoff with jitter: 15s, 30s, 60s, 120s, 240s, 480s, then stop or schedule a later attempt.
   - Cap attempts and total elapsed time.
   - Save state before each retry.
   - Continue offline work while waiting; do not busy-loop.

8. **Handle quota exhaustion.** Record the exact error, reset time if known, and remaining offline work. Do not create new accounts, rotate keys, or circumvent limits. Resume after the reset with a fresh checkpoint.

9. **Checkpoint before risky boundaries.** Save before context compaction, long builds, destructive commands, external side effects, or handoff. Include:
   - task ID, goal, status, and owner;
   - branch/commit and dirty files;
   - files changed and why;
   - exact commands and observed results;
   - blockers and missing prerequisites;
   - next exact command, not a general intention.

10. **Hand off cleanly.** A fresh agent should be able to continue from the checkpoint alone. State what is verified, what is suspected, and what must not be repeated.

## Validation and acceptance criteria

- The checkpoint matches the actual tree, or the discrepancy is documented.
- The narrow verification was re-run after resume; broader validation is recorded if run.
- Completed work is not duplicated; task IDs and file ownership are checked.
- Network retries respect backoff, `Retry-After`, and a maximum attempt budget.
- Quota limits are not bypassed; offline work continues safely.
- The checkpoint contains exact commands and observed results, not intentions.
- A fresh agent can identify the next action without the original conversation.
- Blocked work has a concrete prerequisite, owner, and resume condition.

## Expected output

1. Recovery report: checkpoint status, tree state, discrepancies.
2. Re-verification log with exact commands and results.
3. Updated state files and task board.
4. Checkpoint with next exact step and blockers.
5. Backoff/retry log when applicable.
6. Handoff note for the next agent.

## Troubleshooting

- **Checkpoint is missing:** reconstruct from `git diff`, test logs, and task-board entries; write a new checkpoint immediately.
- **Checkpoint and tree disagree:** trust the tree; record the difference and update the state rather than reverting user work.
- **Tests fail after resume:** determine whether the tree changed, the environment differs, or a dependency is missing. Record the first failing command and output.
- **Provider remains down:** stop retrying after the budget, keep the blocker, and finish offline tasks. Do not claim the online step happened.
- **Quota exhausted:** record the reset time and continue offline; never bypass the limit.
- **Context nearly exhausted:** write the checkpoint before continuing; include the next command and the files to read first.
- **Duplicate work suspected:** compare task IDs, branches, worktrees, and diffs; reconcile with the owner before editing.
- **State file contains secrets:** redact it, rotate if necessary, and record the incident according to policy.

## Examples

```sh
# Resume safely
git status --short && git log -1 --oneline
cat AGENT_STATE/ACTIVE_TASK.md
cat AGENT_STATE/CHECKPOINTS/2026-11-26-T-042.md
cargo test --test focused_test

# Check for duplicate work
git branch --all --list '*T-042*'
git worktree list
rg -n 'T-042' AGENT_STATE TASKS.md
```

```markdown
# Checkpoint - 2026-11-26 - T-042

## State
- Branch: task-042; HEAD: abc1234; dirty: src/parser.rs, tests/parser.rs
- Status: implementation complete, full suite not yet run

## Verification
- `cargo test --test parser` -> 18 passed, 0 failed
- `cargo clippy --workspace --all-targets -- -D warnings` -> not run

## Blockers
- None

## Next exact steps
1. Run `cargo test --workspace`.
2. If green, update TASKS.md with the command and result.
```

## Related skills and review

- Pair with `multi-agent-orchestration` for ownership, handoff, and independent review.
- Pair with the domain skill for the active task so recovery does not skip required validation.
- Reviewer checks: checkpoint matches tree, no duplicate work, retries bounded, state redacted, and next step executable.
