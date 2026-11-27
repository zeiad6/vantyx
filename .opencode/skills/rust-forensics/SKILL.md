---
name: Rust Repository Forensics
description: Audit an unfamiliar Rust repository before changing it, map architecture and dependencies, investigate Git history, reproduce defects, and produce an evidence-backed forensics report without destructive Git operations.
---

# Rust Repository Forensics

## When to use

- You are entering an unfamiliar Rust repository or one whose docs, task files, or prior agent reports may be stale.
- You need a complete source, history, dependency, or architecture audit before a change.
- A defect must be reproduced and traced to a root cause rather than patched by guesswork.
- You need evidence for a claim about build health, dependency risk, or repository history.

Do not use this skill as a substitute for the domain skill that will make the change. Use it to establish facts, then load the relevant implementation skill.

## Required tools

- `git`, `cargo`, `rustc`, `rg` (ripgrep).
- `cargo metadata`, `cargo tree`, `cargo clippy`, `cargo fmt`.
- Optional and only if already installed or explicitly approved: `cargo audit`, `cargo deny`, `cargo geiger`, `miri`, `gdb`/`lldb`, `strace`, `perf`, `valgrind`.
- A sandbox or container when the repository or its `build.rs` scripts are untrusted.

## Safety constraints

- Never run `git reset --hard`, `git clean`, `git checkout -- .`, force-push, or rewrite history. Treat every uncommitted change as user work.
- Read `build.rs`, proc-macro crates, and CI scripts before executing a build. They can run arbitrary code and access the network.
- Do not install dependencies, toolchains, or crates without checking the environment policy and recording the change.
- Do not print secrets, tokens, private keys, or full environment dumps. Redact evidence before sharing.
- Do not delete files or weaken tests to make a build pass.
- Label every finding `CONFIRMED`, `SUSPECTED`, or `UNKNOWN`; never present source reading as runtime proof.

## Workflow

1. **Freeze the starting point.**
   ```sh
   git status --short
   git branch -vv
   git log --oneline --decorate -20
   git remote -v
   git diff --stat
   git stash list
   git tag --sort=-creatordate | head
   git submodule status
   ```
   Record the exact HEAD, branch, dirty files, and date. Do not change the tree yet.

2. **Read the repository rules and claims.** Read `README*`, `AGENTS.md`, `CONTRIBUTING*`, `SECURITY*`, `rust-toolchain*`, `Cargo.toml`, workspace manifests, CI config, and any task/status/audit files. Separate rules from claims; claims need evidence.

3. **Inventory the workspace.**
   ```sh
   cargo metadata --format-version 1 --no-deps
   cargo tree --depth 1
   cargo tree -d
   cargo tree -e features
   cargo tree --target all
   ```
   List workspace members, binaries, libraries, examples, benchmarks, tests, build scripts, and feature flags. Note any dependency with native code, a build script, or a proc macro.

4. **Map the architecture.** Start at `src/main.rs`, `src/lib.rs`, and workspace entry points. Identify modules, public types, traits, and dispatch paths. Useful searches:
   ```sh
   rg -n '^(pub(\([^)]*\))? )?(struct|enum|trait|fn|mod|type|const|static)\b' src
   rg -n 'impl .* for |match .*\{|Command::new|std::fs|std::net' src
   ```
   For each subsystem record: responsibility, public API, callers, tests, feature gates, and failure modes.

5. **Investigate Git history, not just the current tree.**
   ```sh
   git log --stat --since='90 days ago'
   git log --follow -- path/to/file
   git log -S'pattern' --oneline -- path
   git blame -L 1,80 -- path/to/file
   ```
   Identify hotspots (frequent churn), reverted fixes, stale TODOs, deleted tests, and changes that contradict the docs. Do not rewrite history.

6. **Establish a baseline before edits.**
   ```sh
   cargo check --workspace --all-targets --all-features
   cargo test --workspace --all-targets --all-features
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo fmt --all -- --check
   ```
   Record the toolchain (`rustc -Vv`, `cargo -V`), target, exact commands, exit codes, and relevant output. A timeout or missing tool is not a pass.

7. **Search for risk, not style.**
   ```sh
   rg -n 'unsafe|unwrap\(|expect\(|panic!|todo!|unimplemented!|unreachable!' src
   rg -n 'Command::new|process::|std::fs|remove_|rename|copy|canonicalize' src
   rg -n 'MAX_|limit|cap|budget|timeout|depth' src
   rg -n '#\[cfg\(|target_os|target_arch' src
   ```
   Inspect every `unsafe` block, FFI boundary, process spawn, filesystem mutation, and network path. Distinguish internal invariants from user-reachable input.

8. **Reproduce suspected defects with the smallest input.** Prefer a focused unit or integration test. Capture:
   - exact command and working directory;
   - commit and dirty state;
   - input fixture or minimal program;
   - exit code, stdout, stderr, and backtrace (`RUST_BACKTRACE=1`);
   - responsible `file:line` and the code path that reaches it.
   Do not "fix" a defect before the reproduction is stable.

9. **Audit dependencies and supply chain.**
   ```sh
   cargo tree -i <crate>
   cargo tree --duplicates
   cargo audit
   cargo deny check
   ```
   Record licenses, advisories, unmaintained crates, duplicate major versions, build scripts, and native libraries. A new dependency needs an explicit justification and, in many projects, an ADR.

10. **Write the evidence ledger.** Store it in the repository's existing audit/research directory; do not create a parallel documentation tree. Each finding needs severity, evidence class, reproduction, impact, suspected root cause, and next action.

## Validation and acceptance criteria

- Repository snapshot, toolchain, and baseline commands are recorded with date and observed results.
- Architecture map covers every workspace member and identifies entry points and subsystem boundaries.
- Dependency report covers direct, transitive, duplicate, native, and build-time dependencies.
- Git history was read, not rewritten; no user work was discarded.
- Every defect claim has a reproduction command and observed output, or is explicitly `SUSPECTED`/`UNKNOWN`.
- The report states which tools, platforms, and feature combinations were not tested.
- No source file was modified before its current behavior was understood.

## Expected output

A `FORENSICS.md` or equivalent report containing:

1. Snapshot: commit, branch, dirty state, toolchain, date.
2. Repository map: workspace members, entry points, module tree.
3. Architecture: subsystem responsibilities, data flow, public APIs.
4. Dependency and supply-chain findings.
5. Build/test/clippy/fmt baseline with exact commands and outcomes.
6. Git-history hotspots and relevant prior changes.
7. Defects and root-cause analysis, each with evidence class and reproduction.
8. Risks, unknowns, and recommended next actions.
9. A short "what not to trust" list of stale docs or prior claims.

## Troubleshooting

- **`build.rs` needs network access:** stop, inspect the script, use a sandbox or `--offline`, and record the blocked build instead of granting network access silently.
- **Feature combinations fail:** test `--no-default-features`, `--all-features`, and the minimal feature set separately; report the matrix.
- **Tests hang or time out:** run the narrowest target with a timeout, capture the last output, and check for network, stdin, or infinite-loop dependencies.
- **Proc-macro or generated code hides behavior:** expand with `cargo expand` if available; otherwise inspect the macro source and label the behavior unverified.
- **Docs contradict code:** trust the code and the observed behavior, then update the docs as part of the fix.
- **Dirty tree blocks a clean baseline:** record the diff and test both the dirty tree and a clean worktree without touching the user's changes.

## Examples

```sh
# Establish facts before touching anything
git status --short && git log -1 --oneline
cargo metadata --format-version 1 --no-deps > /tmp/metadata.json
cargo tree --duplicates
cargo test --workspace --all-targets --all-features 2>&1 | tee /tmp/baseline-test.log

# Trace a panic to a minimal case
rg -n 'unwrap\(|panic!' src
RUST_BACKTRACE=1 cargo test failing_test -- --nocapture

# Find when a risky pattern appeared
git log -S'unsafe' --oneline -- src
git blame -L 40,90 -- src/runtime.rs
```

## Related skills and review

- Pair with `performance-benchmarking` for any performance claim and `security-audit-engineering` for unsafe, FFI, process, network, or dependency risk.
- Pair with `runtime-memory-safety` when the defect involves ownership, leaks, aliasing, or reclamation.
- Reviewer checks: baseline evidence exists, every claim is labeled, no destructive Git command ran, and the report names untested surfaces.
