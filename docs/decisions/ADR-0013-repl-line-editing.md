# ADR-0013: REPL line editing, history, completion, and expression echo

- Status: Accepted
- Date: 2026-11-23
- Related: ADR-0007 (zero dependencies), T-009 (REPL persistence), T-029

## Context

The v0.1 REPL could execute statements, persist declarations, and recover from
errors, but it read input with `BufRead::read_line`. That has three practical
consequences:

- the terminal's canonical line discipline handles editing, so the REPL cannot
  offer its own history navigation or Tab completion;
- expression statements are executed and their values are discarded, so a user
  who types `1 + 2` sees nothing;
- submitted entries are not available for recall after a restart.

The compiler core forbids third-party Rust dependencies (ADR-0007). A REPL
editor is the first tooling feature where that rule is visible to users.

## Options

1. **Add `rustyline` or `reedline`.** Mature editors, but they violate ADR-0007
   and add a large transitive supply chain to the binary.
2. **Call termios directly through `extern "C"`.** No crate is required, but
   the compiler core currently contains no `unsafe`, and hand-written
   `struct termios` layouts are platform-specific and easy to get wrong.
3. **Use the system `stty` utility for raw mode, with a canonical fallback.**
   Safe Rust, no Rust dependency, and a small, isolated failure mode. `stty`
   is part of coreutils on Linux and is present on macOS.
4. **Keep canonical input and expose `:history`/`:complete` commands.** Simple,
   but it does not deliver the interaction users expect from a REPL.

## Decision

Option 3, with a strict fallback.

### Line editing

- When stdin is a terminal and `/bin/stty` or `/usr/bin/stty` exists, the REPL
  enters raw mode for one line at a time and restores the saved terminal state
  with a `Drop` guard.
- Supported keys: printable UTF-8 input, Backspace, Delete, Left/Right/Home/End,
  Up/Down history, Tab completion, Ctrl-A/E (start/end), Ctrl-K (kill to end),
  Ctrl-U (kill to start), Ctrl-W (kill word), Ctrl-L (clear screen), Ctrl-C
  (cancel the current entry and return to the prompt), and Ctrl-D (exit on an
  empty line).
- If stdin is not a terminal, or `stty` is missing or fails, the REPL uses
  canonical `read_line`. Piped scripts therefore keep working unchanged.

`stty` is resolved from the fixed paths `/bin/stty` and `/usr/bin/stty`; the
REPL does not search `PATH` for it.

### History

- History is an in-memory list capped at 1000 entries. Consecutive duplicates
  are ignored.
- Entries are persisted one per line to `$AUREN_HISTORY` when set, otherwise to
  `$HOME/.auren_history`. `AUREN_HISTORY=off` disables persistence.
- The file is created with mode 0600 on Unix because REPL entries can contain
  secrets. Persistence is best-effort: a read-only home directory does not
  break the REPL.
- Multi-line blocks are not persisted and are not recalled by Up/Down. The
  editor is single-line; storing a block on one line would change its source.

### Completion

- Candidates are keywords, the public `BUILTINS` list, and names collected from
  successful top-level entries: `let` bindings, functions, structs, enums, and
  enum variants.
- The completed token is the run of alphanumeric/underscore characters before
  the cursor. Tab extends to the longest common prefix; if no progress is
  possible, it lists the matches, four per line.
- Completion does not inspect fields, local scopes, or names from a previous
  session. Those require a richer model and are tracked as a follow-up.

### Expression echo

- After a new entry executes, each new top-level expression statement prints
  its value once, unless the value is `Unit`.
- Entries replayed from earlier history are muted and are never echoed.
- Values use `Value::display()`, the same form as `print`, so a string echoes
  without quotes. This is documented rather than implying a repr format.
- Echo output goes through the interpreter's existing `R0013` write-error
  path.

### Meta-commands

`:help`, `:history`, `:names`, and `:quit` are handled by the REPL and are not
compiled or added to history. `exit` and `quit` remain aliases for `:quit`.

## Rationale

- It satisfies the acceptance criteria without weakening ADR-0007.
- The unsafe-free core stays intact.
- The raw-mode path is optional and isolated in `src/repl.rs`; failure degrades
  to the previous behavior instead of breaking the REPL.
- Expression echo is implemented in the interpreter's REPL entry point, not by
  rewriting source, so side effects run exactly once.

## Consequences

- Interactive editing depends on a system `stty` binary. This is documented and
  has a fallback; it is not a Rust dependency.
- The terminal is restored per line. With `panic = "abort"` in release builds,
  an unexpected panic while raw mode is active could leave the terminal in raw
  mode; the user can recover with `stty sane`.
- History recall is single-line only, and completion is top-level-only.
- History may contain sensitive input. Mode 0600 is a mitigation, not a
  guarantee against a compromised account.
- Echoing `Str` values without quotes is a deliberate display choice, not a
  serialization format.

## Reversibility

High. The editor is behind `Editor::read_line`; replacing the `stty` transport
with termios FFI (with an ADR for the first `unsafe` block) or an approved
dependency does not change the interpreter, checker, or CLI contract.

## Reconsideration criteria

- A target platform lacks a usable `stty` and canonical fallback is judged
  insufficient.
- Multi-line recall or field completion becomes a demonstrated user need.
- ADR-0007 is amended to permit a vetted line-editing crate.
- The first `unsafe` block is justified for terminal control and reviewed by
  the security auditor.
