# Contributing to Vantyx

Vantyx is a pre-1.0 language implementation. Contributions are welcome when they
preserve the interpreter's role as the semantic oracle and keep the native
backend honest about what it supports.

## Ground rules

1. **The interpreter is the oracle.** A native change must be accompanied by a
   differential test in `tests/native.rs` that checks exit status and stdout
   against `vantyx run`.
2. **Never silently miscompile.** Unsupported native constructs must be rejected
   with `E0401` and a concrete help message.
3. **No third-party Rust dependencies.** Adding one requires an ADR. The native
   backend links only libc/libm and invokes the system C compiler.
4. **No unverified claims.** Performance numbers need a stored methodology and
   raw results under `benchmarks/results/`. A task is `VERIFIED` only after a
   real command exercises its acceptance criteria.
5. **Update the record in the same change.** Behavior changes update `TASKS.md`,
   `PROJECT_STATUS.md`, and the specification or an ADR when semantics change.

## Setup

```sh
rustup toolchain install stable
cargo build
cargo test
```

Native tests require `clang` or `VANTYX_CC` pointing at a C11 compiler. They skip
when no compiler is available.

## Before opening a pull request

Run the full gate:

```sh
cargo build
cargo build --release
cargo clippy --all-targets -- -D warnings
cargo test
for f in examples/*.vtx; do ./target/debug/vantyx run "$f"; done
```

For parser or performance changes, also run:

```sh
python3 tools/fuzz_smoke.py --binary target/debug/vantyx
python3 benchmarks/run.py --reps 5
```

## What makes a good change

- A focused behavior change with tests that fail before the fix and pass after.
- A diagnostic that names the problem, the span, and a next step.
- A native feature that either matches the interpreter or is rejected with
  `E0401`; a silent fallback is a bug.
- Documentation that describes what the code actually does, including limits.
- Small commits with a clear subject line. Do not rewrite published history.

## AI-assisted contributions

AI-assisted work is acceptable. The person opening the pull request is
responsible for understanding it, running the verification gate, and removing
generated claims that are not backed by tests or measurements. Do not submit
unreviewed generated code or invented benchmark results.

## Reporting bugs

Open an issue with:

- the Vantyx source (minimal reproduction),
- the exact command,
- expected and actual output,
- `vantyx --version`, OS, Rust version, and C compiler version,
- whether the failure is interpreter-only, native-only, or both.

Security issues should follow [SECURITY.md](SECURITY.md) instead of a public
issue.

## License

By contributing, you agree that your contribution is licensed under the MIT
License in [LICENSE](LICENSE).
