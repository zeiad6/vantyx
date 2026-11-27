# ADR-0045: JSON Schema conformance fixes and shared preflight

- Status: Accepted
- Date: 2026-11-26
- Task: T-095

## Context

T-094 added `--schema` to `vantyx ai dataset` and introduced
`schema::check_schema` as a structural preflight. It also exposed several
places where the in-tree validator did not follow JSON Schema semantics:

- `required` was enforced against non-object values, although it applies only
  to objects.
- `check_schema` accepted an empty `enum` and an empty `allOf`. An empty
  `enum` rejects every value, while an empty `allOf` accepts every value, so a
  malformed schema could fail closed with a misleading model-output error or
  fail open and admit arbitrary tool arguments.
- A successful `anyOf` branch returned immediately and skipped sibling
  keywords such as `minimum` and `required`. `additionalProperties` and
  `items` also returned early for non-object and non-array values, skipping
  sibling keywords that do apply.
- `const` and `enum` used Rust's derived `PartialEq`: `1` did not equal `1.0`,
  and objects with the same members in a different order were unequal.
- `minimum` and `maximum` converted both operands to `f64`, so integers above
  `2^53` could be compared as equal when they were not.
- `extract` and `ToolRegistry::call` validated values without preflighting the
  schema, so a malformed schema could be sent to a provider or used to judge
  model arguments before anyone reported that the schema itself was invalid.

## Decision

Make the supported subset behave consistently with JSON Schema and apply one
preflight everywhere.

- `check_schema` rejects empty `enum` and `allOf` arrays, requires `required`
  to be an array of strings, and recurses through `properties`,
  `additionalProperties`, and `items` within the existing depth limit.
- `validate` evaluates all applicable keywords cumulatively. A matching
  `anyOf` branch no longer suppresses sibling keywords, and
  `additionalProperties`/`items` apply only to objects/arrays without
  returning early from the rest of the schema.
- Add `json::value_eq` for JSON Schema instance equality. Numbers compare by
  mathematical value (`1 == 1.0`) without losing integer precision; objects
  compare regardless of member order; arrays remain order-sensitive.
  `const` and `enum` use it.
- Add `json::number_cmp` for exact numeric ordering across `Int`, `UInt`, and
  `Float`. `minimum` and `maximum` use it instead of `f64` conversion. The
  `2^63` and `2^64` boundaries are checked explicitly so Rust's saturating
  float-to-integer casts cannot create false equality or ordering.
- `extract` calls `check_schema` before building a provider request and maps a
  failure to `AI_E_INVALID_REQUEST` with `extraction schema is invalid`.
  `ToolRegistry::call` calls it before validating arguments and maps a failure
  to `AI_E_SCHEMA` with `tool <name> has an invalid parameter schema`. The tool
  itself is never invoked.
- Unsupported keywords remain ignored. This is still a strict subset, not full
  JSON Schema compliance.

## Consequences

- A schema accepted by dataset validation is preflighted by extraction and
  tools as well, so malformed schemas fail before spending tokens or running
  a tool.
- Precision-sensitive schemas such as `{"minimum": 9007199254740993}` now
  distinguish adjacent 64-bit integers and adjacent mixed integer/float
  values.
- `{"allOf": []}` can no longer silently accept every tool argument, and
  `{"enum": []}` is reported as a schema error rather than as a model-output
  violation.
- JSON equality is available to future `uniqueItems`, `contains`, or
  dependency keywords without changing the derived `PartialEq` used by
  unrelated Rust code.
- A `NaN` float cannot be parsed from JSON input; programmatically constructed
  `NaN` values compare as unordered and therefore never violate `minimum` or
  `maximum`.

## Alternatives considered

- **Change the derived `PartialEq` for `Json` globally:** rejected. Other
  consumers use structural equality for serialization and tests; JSON Schema
  instance equality is a separate operation.
- **Compare all numbers through `f64`:** rejected. It loses integer precision
  above `2^53` and already produced wrong `const`, `enum`, and bound results.
- **Compare integers by decimal string:** rejected. Exact integer and
  mixed-number comparison is shorter, allocation-free, and easier to test.
- **Make malformed schemas a warning:** rejected. A schema that accepts
  everything is a security-relevant fail-open for tool arguments.
- **Preflight only in the CLI:** rejected. Library callers use `extract` and
  `ToolRegistry` directly; the check belongs at the public boundary.

## Limits

- The subset still omits `$ref`, `$defs`, `oneOf`, `not`, `if`/`then`/`else`,
  `pattern`, `format`, `uniqueItems`, `contains`, `propertyNames`, tuple
  `items`, and remote references.
- `check_schema` verifies supported keyword shapes, not satisfiability. A
  schema such as `{"type":"string","const":1}` is structurally valid but
  unsatisfiable.
- Schema and value are assumed to be parsed by the in-tree JSON parser, which
  rejects duplicate keys and non-finite numbers. Programmatically constructed
  duplicate object keys are not detected by `value_eq`.
- Preflight does not hash the schema or make it immutable; a caller that
  mutates its own `Json` between preflight and use is responsible for the
  result.
