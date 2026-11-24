# Security policy

## Supported versions

Vantyx is pre-1.0. Only the latest `0.1.x` source tree is supported. There are no
long-term-support branches yet.

## Reporting a vulnerability

Please use the repository's **Security** tab and open a private advisory. Do not
open a public issue for a vulnerability. Include:

- affected command (`run`, `check`, `build`, `repl`),
- Vantyx source and the exact invocation,
- platform, Rust version, and C compiler version,
- impact and a minimal reproduction,
- whether the issue is interpreter-only, native-only, or both.

This is a volunteer project without a paid security team. Reports are handled on
a best-effort basis; no response or fix deadline is promised.

## Scope

In scope:

- compiler or interpreter crashes on valid or malformed input,
- memory-safety bugs in generated C,
- silent miscompilation,
- path or command injection in the CLI or native build,
- denial of service caused by unbounded recursion, allocation, or parsing,
- history-file permission or disclosure bugs in the REPL.

Out of scope:

- the documented native-subset limits (`E0401`),
- intentionally leaked native allocations,
- the absence of a sandbox,
- performance issues,
- vulnerabilities in `clang`, libc, or the host toolchain,
- social-engineering or physical attacks.

## Known limitations

These are documented design limits, not vulnerabilities:

- Native allocations are intentionally leaked in v0.1. There is no reclamation
  and no cycle collector.
- Native map lookup is O(n).
- The native backend supports a statically typed subset. `Any`, `List[Any]`,
  maps with non-inferable types, `Any` enum payloads, and `match` on `Any` are
  rejected with `E0401`.
- Vantyx programs are not sandboxed. Running an untrusted program is equivalent
  to running untrusted native code.
- REPL history is plain text. It is created with mode 0600 on Unix, but it can
  contain secrets. Set `VANTYX_HISTORY=off` to disable persistence.
- The REPL invokes `/bin/stty` or `/usr/bin/stty` from fixed paths for raw-mode
  editing and falls back to canonical input on failure.

See [docs/security.md](docs/security.md) and
[docs/security-audit-2026-11-22.md](docs/security-audit-2026-11-22.md) for the
full audit record.
