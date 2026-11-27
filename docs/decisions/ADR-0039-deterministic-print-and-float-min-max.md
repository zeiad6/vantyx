# ADR-0039: Deterministic `print` evaluation and float `min`/`max` semantics

- Status: Accepted
- Date: 2026-11-24
- Related: T-079, T-078, ADR-0008, ADR-0016, ADR-0036

## Context

A differential probe of numeric and display edge cases found two silent
interpreter/native divergences.

1. **Signed zero in float `min`/`max`.** The interpreter called Rust's
   `f64::min`/`f64::max`, while native called C `fmin`/`fmax`. Rust documents
   that a tie returns the second operand, so `min(-0.0, 0.0)` is `0.0` and
   `min(0.0, -0.0)` is `-0.0`. glibc's `fmin`/`fmax` return the first operand
   on a tie, so `print(min(-0.0, 0.0), max(-0.0, 0.0), ...)` printed
   `0.0 0.0 -0.0 -0.0` in the interpreter and `-0.0 0.0 -0.0 0.0` natively.
   Both operands are observationally different because float display preserves
   the sign of zero. Relying on `f64::min` also left the oracle's tie behavior
   tied to LLVM lowering rather than an explicit language rule.

2. **`print` interleaved evaluation and output in native code.** The
   interpreter evaluates every argument into a `Vec<Value>`, then renders the
   values, then writes one line. Native code emitted
   `vantyx_print_text(<render arg 1>)`, `vantyx_print_sep()`, and so on inside
   one comma expression, so each argument was rendered and written before the
   next argument was evaluated. Two observable differences followed:
   - If a later argument raised a runtime error, native had already written a
     prefix of the line while the interpreter wrote nothing. For example,
     `print("partial", floor(5e300))` left `partial ` on native stdout.
   - If a later argument mutated a collection passed as an earlier argument,
     interpreter displayed the mutated value, because rendering happened after
     all arguments were evaluated; native displayed the pre-mutation value.

The interpreter is the semantic oracle (repository rule 1), so both are
correctness defects, not cosmetic differences.

## Decision

### Float `min`/`max`

- Define the rule explicitly in both engines:
  - If exactly one operand is NaN, return the other operand.
  - Otherwise, if the operands compare equal (including `-0.0` and `0.0`),
    return the second operand.
  - Otherwise return the smaller (`min`) or larger (`max`) operand.
- The interpreter uses local `float_min`/`float_max` helpers instead of
  `f64::min`/`f64::max`. This keeps the oracle stable across optimization
  levels and documents the tie rule in source.
- Native uses `vantyx_min_float`/`vantyx_max_float` with the same branches
  instead of `fmin`/`fmax`.
- Both-NaN results are still NaN. The choice of which NaN payload/sign is
  returned is not observable through Vantyx display or equality.

### `print`

- Evaluate all arguments left to right into typed C temporaries.
- Only after every argument has been evaluated, render each temporary with the
  existing display helpers.
- Then write the values separated by spaces and end the line with the existing
  `vantyx_print_end` check (`R0013` on a write failure).
- A failing argument therefore produces no partial line, and mutations caused
  by later arguments are visible when the line is rendered, matching the
  interpreter.

This is a code-generation and runtime-helper correction. It adds no syntax,
type, or dependency, and does not change the `E0401` boundary.

## Consequences

- `min(-0.0, 0.0)` prints `0.0` and `min(0.0, -0.0)` prints `-0.0` in both
  engines; the same ordering applies to `max`.
- Interpreter output for these cases is identical in debug and release builds.
- A runtime error in any `print` argument leaves stdout at the previous
  completed line in both engines. Errors are still exit code `2`.
- `print(xs, mutate(xs))` displays the post-mutation state in both engines.
- Native `print` keeps every argument value live until the line is rendered.
  This is consistent with the v0.1 leaked-allocation policy and adds no
  reclamation requirement.
- Other output paths (`write_file`, `append_file`, `assert`) are unchanged.

## Tests

- `tests/e2e.rs::float_min_max_ties_and_nan_are_stable` pins the signed-zero
  and NaN results in the interpreter.
- `tests/e2e.rs::print_evaluates_every_argument_before_writing` pins the
  mutation-visible display and the empty stdout after a failing argument.
- `tests/native.rs::float_min_max_ties_match` and
  `tests/native.rs::print_evaluates_arguments_before_writing` compare both
  engines on the same programs.
- `tools/differential_fuzz.py` now prints signed-zero and NaN `min`/`max`
  combinations in `numeric_program` and includes a failing `print` with a
  leading argument in `error_program`.
- Manual probes before the fix reproduced both divergences; after the fix the
  interpreter and native stdout were byte-identical, including empty stdout on
  the failing `print` cases.

## Amendment — 2026-11-27 (T-107)

The original `vantyx_print_end` checked only `ferror(stdout)` after writing the
newline. Native stdout is fully buffered when it is not a terminal, so a full
disk was usually detected by the final `fflush` in `main` as
`error: failed to flush output` (exit 2), without the `R0013` code or the
print's location; a closed output pipe could terminate the process with
SIGPIPE (exit 141). The interpreter's line-buffered stdout reports
`R0013: failed to write output: <host error> (os error N)` at the print.

`vantyx_print_end` now checks `fputc`, calls `fflush(stdout)`, and checks
`ferror`, capturing `errno` before formatting `strerror(errno)` and
`(os error N)`. `main` calls `signal(SIGPIPE, SIG_IGN)` to match Rust's
default. Tests cover `/dev/full` and a closed pipe. This changes only the
stdout failure path; successful output and evaluation order are unchanged.

## Alternatives considered

1. **Adopt C `fmin`/`fmax` semantics in the interpreter (first operand on a
   tie).** Rejected: it would silently change existing interpreter output,
   including possibly changing behavior across Rust optimization levels, and
   the oracle is the interpreter. The explicit second-operand rule is stable
   and already matches the observed interpreter behavior.
2. **Leave `min`/`max` unspecified for signed zero.** Rejected: signed zero is
   observable through display, so "unspecified" means the two engines can print
   different programs. The language needs one rule.
3. **Buffer native output in a temporary string and print it once.** Viable,
   but it would duplicate the interpreter's display formatting and allocation
   behavior. Typed temporaries plus the existing display helpers are the
   smaller change and preserve the existing rendering code.
4. **Render each argument only after all have been evaluated but reuse the
   original comma expression.** Rejected: C argument evaluation order is
   unspecified, and interleaving would remain. The statement expression with
   ordered assignments is explicit.
5. **Treat a partial line as acceptable and document it.** Rejected: it is
   observable stdout, and the repository rule requires native behavior to match
   the interpreter rather than document a mismatch.
