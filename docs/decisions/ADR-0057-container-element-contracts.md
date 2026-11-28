# ADR-0057: Container element contracts for gradual soundness

- Status: Accepted; implemented by T-123 for the interpreter. The native backend
  already rejects `Any`, `List[Any]`, and `Map[Any, *]` with `E0401`, so no
  native code generation changes.
- Date: 2026-11-27
- Related: ADR-0003 (gradual typing), ADR-0005 (reference semantics), T-123

## Context

ADR-0003 promises that every `Any -> T` boundary is validated at runtime and
that typed code cannot observe a wrong-typed value. T-123 found that the promise
did not cover mutable containers:

```vantyx
fn main():
    let xs: List[Int] = [1]
    let alias: Any = xs
    push(alias, "s")      # accepted; the list now holds a Str
    print(xs)             # [1, "s"]
```

The same hole existed for `List[Any]` parameters, list index assignment, map key
and value assignment, and nested containers reached through struct fields or
enum payloads. A typed read after the pollution raised `R0005` only when the
read itself crossed a dynamic boundary; a direct typed read such as `xs[0]`
returned the wrong-typed value silently. Reference semantics (ADR-0005) makes
the problem observable through every alias.

The interpreter stores only runtime values, so a list or map did not remember
which element types it had been validated against. The checker's `expr_types`
are available at construction and boundary sites, but not at every later
mutation through an alias.

## Options

1. **No container state; validate every typed read and write.** The interpreter
   would need the static type of each indexing, iteration, and builtin site and
   would check elements on every access. This adds repeated work, misses writes
   that are only observed later by typed code in another frame unless reads are
   exhaustive, and still requires map-key checks on insertion.
2. **Copy the container at each `Any -> T` boundary.** This preserves dynamic
   aliases but changes reference identity: mutations through the original would
   no longer be visible through the validated value, contradicting ADR-0005 and
   existing tests for aliasing and equality.
3. **Attach a permanent contract to each container and strengthen it at
   successful typed boundaries.** Writes through any alias are checked against
   the contract. Contracts start at `Any` and only move to a concrete type.
4. **Full runtime seals with temporary suspension.** A boundary would seal the
   container for the dynamic extent of typed code and restore the previous
   contract afterwards. This is the most permissive design, but it requires
   scoped enter/exit bookkeeping around every call, early return, runtime error,
   and re-entrant callback, and it still has to reject writes while the seal is
   active.

## Decision

Option 3. Every interpreter list stores an element contract; every map stores a
key contract and a value contract. A contract is a `Type`:

- A list or map literal adopts the checker's static type for that expression.
  `[1, "a"]` and `{}` therefore start as `Any`; `[1]` in a `List[Int]` position
  starts as `Int`.
- A successful runtime validation at an annotated binding, parameter, return,
  field, or constructor payload strengthens `Any` contracts to the validated
  type. Strengthening is one-way: a concrete contract is never widened back to
  `Any`.
- Strengthening recurses through nested lists, maps, structs, and enum payloads
  with a visited set keyed by container identity and expected type, and the
  existing `MAX_VALUE_DEPTH`, so cyclic values terminate without skipping a
  different expected type.
- `push`, list index assignment, and map key/value assignment validate the
  written value against the contract before mutating. A mismatch is `R0005`
  with the operation named in the message.
- `range`, `split`, `keys`, `values`, `sorted`, `reverse`, `list_dir`, and
  `glob` construct results with the element type their checker signature
  guarantees.

`List[Any]` and `Map[Any, Any]` contracts stay `Any`, so fully dynamic
containers remain usable for heterogeneous data.

## Consequences

- A container that has crossed a typed boundary carries an invariant for the
  rest of its lifetime. A later write through a `List[Any]` or `Any` alias that
  the static type would allow can now raise `R0005`. This is a runtime type
  error, which the gradual guarantee permits; it is never silent corruption.
  For example, after `take(xs)` validates a `List[Any]` as `List[Int]`,
  `push(xs, "s")` in the caller is rejected.
- Empty containers are strengthened too. If an empty list were left at `Any`,
  dynamic code called from inside a typed function could add a wrong-typed
  element while the typed frame still held the `List[Int]` view.
- The contract is metadata, not a new value; display, equality, hashing,
  iteration order, aliasing, and copying are unchanged.
- Structural map keys are strengthened like other nested values. A key inserted
  through a typed map is checked before insertion.
- The native backend is unaffected: it rejects the dynamic containers that
  could create the hole with `E0401`. The interpreter remains the semantic
  oracle for programs the native subset does not accept.
- This is not a general seal or ownership system. There is no way to suspend or
  reset a contract, no per-alias capability, and no static effect tracking.

### Cycle conformance (T-126)

Runtime conformance uses the same `(container identity, expected type)` key as
strengthening. A self-referential list `xs = [xs]` conforms to
`List[List[Any]]` because the inner `Any` accepts every element, but it does
not conform to `List[List[Int]]`: the outer check revisits `xs` under
`List[Int]`, whose element check requires `Int` and finds a list. Returning
`true` on any identity-only revisit, as the earlier implementation did, made
the mismatched binding succeed and let a statically `Int` expression evaluate
to a list. Mutual cycles and self-referential maps use the same rule. The
change is interpreter-only because the native backend rejects the dynamic
containers with `E0401`.

## Rejected alternatives

Option 1 was rejected because exhaustive read checking would duplicate work on
every access and still needs the same write checks for map keys and aliased
containers. Option 2 was rejected because copy-on-boundary breaks the documented
reference semantics. Option 4 was rejected for v0.1 because scoped seals require
dynamic-wind-style bookkeeping across calls, returns, and errors, and no current
test or user workflow needs temporarily dynamic access to a validated container.

## Reversibility

Medium. The contracts are internal to `Value`; removing them restores the prior
unsound behavior, while extending them to scoped seals would add state to the
same structures. A future ownership or effect system can replace the runtime
contracts without changing syntax.

## Reconsideration criteria

Revisit if one-way strengthening rejects common gradual workflows, if a scoped
seal is needed for callback-heavy APIs, or if typed-read checking becomes cheap
enough to replace permanent contracts. Any change must keep the ADR-0003
guarantee and the interpreter/native boundary honest.
