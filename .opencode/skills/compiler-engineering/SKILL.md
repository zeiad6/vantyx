---
name: Compiler Engineering
description: Design and change lexers, parsers, ASTs, type systems, IR, lowering, optimizations, diagnostics, and backends with semantic preservation, differential testing, fuzzing, and reproducible validation.
---

# Compiler Engineering

## When to use

- Adding or changing syntax, semantics, types, diagnostics, IR, optimization passes, or code generation.
- Fixing a miscompile, crash, wrong diagnostic, parser ambiguity, or backend disagreement.
- Introducing a new target, calling convention, ABI, or runtime representation.
- Reviewing a compiler change for correctness, not just passing tests.

Do not use this skill for a pure documentation change or a runtime feature that does not cross a compiler stage; use the runtime or domain skill instead.

## Required tools

- The project's build and test system (`cargo`, `cmake`, `ninja`, `make`, or equivalent).
- A reference interpreter, oracle, or previous compiler version for differential testing.
- A test runner with golden/snapshot support, plus a fuzzer (`cargo-fuzz`, AFL++, libFuzzer, Jazzer, or a custom generator).
- Optional: `llvm-mca`, `objdump`/`llvm-objdump`, `gdb`/`lldb`, sanitizers, `valgrind`, `wasm-tools`, `qemu` for cross-target execution.
- Source-span and diagnostic infrastructure already present in the compiler.

## Safety constraints

- Never silently miscompile. Unsupported constructs must produce a clear error with a source span and a stable diagnostic code.
- Preserve evaluation order, short-circuiting, integer overflow behavior, bounds checks, cleanup/drop order, and error exit codes across every stage.
- Do not add a dependency, target, or ABI change without an explicit design decision and compatibility review.
- Do not weaken or delete a failing test to make a change pass.
- Treat generated code and compiler plugins as untrusted; run them in a sandbox when the input is untrusted.
- Keep debug and release semantics identical unless the language specification explicitly permits otherwise.

## Workflow

1. **Write the language change contract.** Specify syntax, static semantics, dynamic semantics, typing rules, diagnostics, ABI implications, and what is explicitly out of scope. Add positive, negative, and boundary cases to a test matrix before coding.

2. **Trace the pipeline.** Follow one minimal program through every stage:
   - lexer: tokens, trivia, spans, literals, Unicode;
   - parser: grammar productions, precedence, associativity, recovery;
   - AST: nodes, visitors, pretty-printing, serialization;
   - name resolution and type checking;
   - IR construction and verification;
   - lowering, optimization, and backend emission;
   - runtime and linking.
   Record the exact files and data structures for each stage.

3. **Extend representations with spans.** Add token/AST/IR/type nodes with source locations. Update every visitor, matcher, serializer, formatter, and exhaustive `match`. A missing case should be a compile-time error, not a runtime panic.

4. **Implement the reference semantics first.** Put the authoritative behavior in the interpreter, evaluator, or oracle. Add end-to-end tests that assert output, exit code, and diagnostics. Only then mirror the behavior in the optimizing or native path.

5. **Handle the type system deliberately.** Define inference, unification, subtyping, coercion, ownership/borrowing, and error recovery rules. Add tests for ambiguous programs, infinite types, recursive types, and diagnostics that point at the right span.

6. **Lower with explicit invariants.** Preserve evaluation order and side effects. Materialize temporaries where the target language has unspecified sequencing. Verify SSA/phi placement, dominance, lifetime boundaries, and cleanup paths.

7. **Optimize only after correctness.** Each pass needs a stated precondition, postcondition, and idempotence/fixpoint behavior. Run the pass under a verifier and compare optimized vs unoptimized output. Never optimize undefined behavior into a different observable result.

8. **Implement the backend.** Cover calling conventions, stack alignment, register allocation, relocations, symbol visibility, exceptions/panics, and debug info. Differential-test against the reference for every new construct and edge case.

9. **Design diagnostics as an API.** Every diagnostic needs a stable code, severity, primary span, message, optional help, and notes. Test snapshots, JSON output if supported, and recovery behavior. The compiler must not panic on malformed input.

10. **Fuzz and differential-test.** Generate valid and invalid programs. Compare interpreter vs native, debug vs release, and old vs new compiler where applicable. Minimize every crash and add it to the regression corpus.

11. **Update the specification and evidence.** A semantics change updates the language spec, changelog, and design record. Record exact commands, toolchains, targets, and observed results.

## Validation and acceptance criteria

```sh
# Replace with the project's actual commands.
cargo build --workspace
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo test --test e2e
cargo test --test native
cargo fuzz run parser_fuzzer -- -runs=10000
```

- Reference and optimized/native engines agree on exit code, stdout, stderr, and observable side effects for every new case.
- Unsupported constructs fail with a documented diagnostic code and help text, never a crash or wrong output.
- Every new syntax or semantics rule has a specification entry and tests.
- Parser/type-checker fuzz smoke passes; no new panics on malformed input.
- Diagnostics have stable codes and spans; snapshot tests pass.
- Performance claims are backed by a separate benchmark using `performance-benchmarking`.
- The full validation matrix and any untested targets are recorded.

## Expected output

1. A short design note: syntax/semantics, affected stages, invariants, compatibility, out of scope.
2. Implementation changes with spans, visitors, and diagnostics.
3. Tests: unit, golden, end-to-end, negative, differential, and fuzz regression.
4. A validation log with commands, toolchains, targets, and outcomes.
5. Updated specification, changelog, and ADR/design record when semantics changed.

## Troubleshooting

- **Parser conflict or ambiguity:** reduce to the smallest grammar fragment, inspect precedence/associativity, and add a negative test before changing the grammar.
- **Type inference loops or explosion:** add an occurs check, depth/iteration cap, and a diagnostic that names the ambiguous expression.
- **IR verifier failure:** check dominance, phi inputs, type consistency, and cleanup edges; fix the producer, not the verifier.
- **Native result differs from interpreter:** suspect evaluation order, missing temporaries, integer width, signedness, aliasing, or undefined behavior in generated C/LLVM.
- **Debug works, release miscompiles:** compare optimization pass order, verify each pass, and test with optimizations disabled to isolate the culprit.
- **Diagnostic points at the wrong span:** trace span propagation from lexer to AST to type checker; never reconstruct spans from text offsets.
- **Fuzzer finds a crash:** minimize the input, fix the parser/checker/runtime, and add the minimized case to the permanent corpus.

## Examples

```sh
# Minimal pipeline trace
printf 'let x = 1 + 2;\n' > /tmp/probe.src
./target/debug/compiler --emit=tokens /tmp/probe.src
./target/debug/compiler --emit=ast /tmp/probe.src
./target/debug/compiler --emit=ir /tmp/probe.src

# Differential check
./target/debug/compiler run /tmp/probe.src > /tmp/interp.out
./target/debug/compiler build /tmp/probe.src -o /tmp/probe && /tmp/probe > /tmp/native.out
diff -u /tmp/interp.out /tmp/native.out

# Fuzz smoke
cargo fuzz run parser_fuzzer -- -runs=5000 -max_len=4096
```

## Related skills and review

- Pair with `runtime-memory-safety` for representation, ownership, and cleanup changes.
- Pair with `performance-benchmarking` for optimization claims and `security-audit-engineering` for codegen, parser, and compiler-invocation risk.
- Reviewer checks: specification updated, differential tests exist, unsupported paths error cleanly, no test was weakened, and the validation matrix is honest.
