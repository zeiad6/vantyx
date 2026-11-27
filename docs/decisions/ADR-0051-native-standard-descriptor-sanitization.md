# ADR-0051: Native standard-descriptor sanitization

- Status: Accepted
- Date: 2026-11-27
- Related: T-108, T-107, ADR-0008, ADR-0039

## Context

The interpreter runs on Rust, whose runtime opens `/dev/null` for any of the
standard descriptors 0, 1, or 2 that are closed before `main`. A Vantyx
program started with `1>&-` therefore writes `print` output to `/dev/null`
and exits normally.

The native C backend inherited the host process state instead. With stdout
closed:

- `print` failed with `error[R0013]: failed to write output: Bad file
  descriptor (os error 9)` and exited `2`, while the interpreter exited `0`
  and discarded the output.
- A later `open` could receive descriptor 1. A file opened by `write_file`,
  `read_file`, or another builtin could then share the number still cached by
  the C `stdout` object, and a diagnostic or print could interact with the
  wrong descriptor.

The same descriptor-reuse hazard applies to a closed stdin or stderr. It is a
process-startup correctness and safety issue, not a language-level output
policy.

## Decision

At the start of every generated native `main`, before `signal(SIGPIPE,
SIG_IGN)` and before any user code, check descriptors 0, 1, and 2. If one is
closed, open `/dev/null` with the appropriate access mode and, when the kernel
does not return the same number, `dup2` it into place and close the temporary
descriptor.

- stdin: `O_RDONLY`
- stdout: `O_WRONLY`
- stderr: `O_WRONLY`

This mirrors Rust's standard-descriptor sanitization, so the interpreter is
still the semantic oracle. A closed stdout now silently discards `print`
output in both engines, and a closed standard descriptor cannot be reused by
a file opened later in the program.

## Alternatives considered

- **Keep failing on a closed stdout.** Rejected: the interpreter is the
  oracle, and Rust's sanitization is observable. Making native fail would
  require changing the interpreter to match a host artifact.
- **Only sanitize stdout.** Rejected: a closed stdin or stderr has the same
  descriptor-reuse hazard, and Rust sanitizes all three.
- **Rely on libc startup.** Rejected: libc leaves closed descriptors closed;
  the `stdout`/`stderr` `FILE*` objects still cache the original numbers.
- **Claim this is a sandbox.** Rejected: opening `/dev/null` is a
  compatibility measure, not isolation. Vantyx remains unsandboxed.

## Consequences

- Closed-standard-descriptor behavior matches the interpreter on POSIX
  hosts.
- `/dev/null` must exist; if it cannot be opened, the descriptor stays closed
  and the previous failure behavior remains as a fallback.
- The sanitization runs once at startup and does not change descriptors that
  are already open.
- The backend still requires POSIX headers and is not a Windows or WASM
  portability claim.
- `print` output failures on an open but failing descriptor (for example
  `/dev/full` or a broken pipe) remain `R0013` diagnostics with the print
  location, as specified by ADR-0039.

## Tests

- `tests/native.rs::closed_stdout_matches_interpreter_and_does_not_reuse_descriptors`
  runs both engines with stdout closed through a POSIX shell, checks that both
  reach `assert(false)` with the same `R0006` headline, and verifies that a
  `write_file` before the assertion still wrote the marker. Before the fix the
  native engine failed at the first `print` with `R0013` and did not create the
  marker.
- The existing `/dev/full` and broken-pipe tests continue to cover the
  `R0013` path on open descriptors.
