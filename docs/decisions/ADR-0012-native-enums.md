# ADR-0012 — Native enum representation

Status: accepted
Date: 2026-11-22

## Context

ADR-0011 added closed enums with typed payload variants and exhaustive `match`
to the language and interpreter. The native C backend rejected every enum type,
variant constructor, and `match` with `E0401` (T-030). That rejection is safe
but leaves a large part of the language interpreter-only. T-030 must lower
enums without changing observable semantics.

The native backend already has:

- unboxed scalar types, heap pointers for strings, structs, lists, and maps
- generated per-type display and equality callbacks
- a type descriptor (`VantyxTypeDesc`) used by lists, maps, and structural
  equality
- intentional allocation leaks and no reclamation, documented in the memory
  model
- GNU C11 statement expressions for left-to-right evaluation and expression
  forms

## Problem

Choose a native enum representation that:

- preserves interpreter semantics for construction, payload access, display,
  equality, `type`, and matching
- supports recursive and mutually recursive enum payload types
- works inside lists, maps, struct fields, globals, parameters, and returns
- keeps enum values immutable while allowing payload references to alias
- integrates with the existing descriptor-based equality/display machinery
- does not require a dynamic `Any` representation
- is auditable and replaceable later

## Options considered

### A. Heap-allocated tagged struct with a union of variant payloads

Each enum type becomes a C struct:

```c
struct Vantyx_e_Shape {
    int64_t tag;
    union {
        int64_t vantyx_dummy;
        struct { double vantyx_f_radius; } vantyx_v_Circle;
        struct { double vantyx_f_width; double vantyx_f_height; } vantyx_v_Rect;
    } payload;
};
```

Values are `Vantyx_e_Shape*`. A constructor allocates, zeroes, sets the tag, and
assigns payload fields. Display and equality are generated per enum.

Advantages: static payload layout; recursive payloads are pointers; enum values
fit the existing pointer-based collection/struct machinery; no dynamic tags;
copying the pointer preserves immutability and payload aliasing.

Disadvantages: each enum value is a separate allocation; no reclamation in the
current backend; pattern matching needs generated accessors.

### B. Generic tagged value with a descriptor and byte buffer

A single `VantyxValue` carries a tag and a payload buffer, with descriptors for
display/equality.

Advantages: one representation for all enums; easier future dynamic support.

Disadvantages: loses static payload typing, duplicates the dynamic machinery
that native deliberately rejects, and makes generated code harder to audit.
`Any` remains unsupported, so the flexibility is unused.

### C. Integer tag plus parallel payload fields in every value

Store all possible payload fields in one flat struct with a tag.

Advantages: simple field access; no union.

Disadvantages: wastes space for large enums; invalid field combinations are
representable; recursive payloads still require pointers; easy to construct
invalid values accidentally.

### D. By-value tagged union with inline payloads

Advantages: no per-value allocation; value semantics match copying.

Disadvantages: recursive enums cannot be inline; C cannot have a struct contain
itself by value. A pointer indirection would be needed for recursion anyway,
and copying large payloads would diverge from the interpreter's reference
behavior for lists/maps/structs.

## Decision

Use **option A**: a heap-allocated tagged struct per enum, with a union of
per-variant payload structs. The native type is `Vantyx_e_<Enum>*`.

### Layout

- `tag` is the zero-based declaration index of the variant, stored as
  `int64_t`.
- The union always contains an `int64_t vantyx_dummy` member so enums with only
  zero-payload variants remain valid C.
- Each variant with at least one payload field gets an anonymous struct member
  named `vantyx_v_<Variant>` containing fields named `vantyx_f_<field>`.
- Zero-payload variants have no union member; their constructor only sets the
  tag.
- Enum, variant, and field identifiers are prefixed in generated C so user
  names cannot collide with C keywords or runtime symbols.

### Construction

Generated function `vantyx_variant_<Enum>_<Variant>`:

1. allocates `sizeof(Vantyx_e_<Enum>)` with `vantyx_alloc`
2. zeroes the value with `memset`
3. sets `tag`
4. assigns payload fields in declaration order
5. returns the pointer

Construction always takes positional payload arguments, matching the checker.
Zero-payload variants take `void`.

### Display and equality

- Display is `Variant` or `Variant(value, ...)`; payload field names are not
  printed. String payloads are quoted, matching the interpreter's nested-value
  display.
- Equality first checks pointer identity, then `NULL`, then the tag, then every
  payload field structurally. Container payloads use the existing coinductive
  equality machinery.
- Enum display does not mark enum pointers in the container cycle set. A finite
  enum value is a tree; cycles can only arise through mutable lists, maps, or
  structs, whose existing display/equality guards terminate the traversal.
  This matches the interpreter, which formats enum payloads without marking the
  enum value itself.
- Each enum gets an `VantyxTypeDesc` of kind `VANTYX_T_ENUM` with display and
  equality callbacks, so lists, maps, and struct fields can hold enums without
  special cases at every use site.

### Matching

`match` lowers to an ordered `if` / `else if` chain on `tag`:

- the scrutinee is evaluated exactly once into a temporary
- the first matching arm wins, matching interpreter arm order
- guards are combined with the tag/literal condition; payload-reading guards
  use a GNU C statement expression to declare bindings and evaluate the guard
- pattern bindings become immutable C locals initialized from payload fields
- `_` becomes the final `else`; later arms are unreachable and omitted
- a defensive final branch raises `R0014` if no arm matches, even though the
  checker requires exhaustiveness for closed enums
- match expressions use a GNU C statement expression with a result temporary;
  match statements emit ordinary control flow

An `if` chain was chosen over `switch` so duplicate or reordered arms cannot
produce duplicate C case labels and so first-match semantics are explicit.

### Memory

Enum allocations are never freed, consistent with native strings, structs,
lists, and maps. Enum payload slots are immutable after construction; payload
references (strings, lists, maps, structs, nested enums) keep their normal
alias semantics. No cycle collector is added. This is a documented limitation,
not a memory-safety claim.

## Rationale

- Option A uses the backend's existing strengths: typed pointers, generated
  callbacks, and descriptor-based collection equality.
- It supports recursive enums naturally because payload fields that name an
  enum lower to pointers.
- It avoids a second dynamic value representation while `Any` remains
  unsupported.
- The union prevents invalid payload combinations at the C type level.
- The IR remains backend-neutral: a future backend can choose a different tag
  width, packing, or allocation strategy without changing source semantics.

## Consequences

- Each enum value allocates. Programs that construct many enums pay allocation
  and pointer-chasing costs; benchmarks must measure this before optimizing.
- Enum values have reference identity internally, but identity is not exposed;
  immutability makes this observationally equivalent to the interpreter's
  copied enum records.
- `Any` enum payloads are rejected with `E0401`. Native enums are limited to
  statically known payload types.
- Generated C gains one struct, one constructor per variant, one display
  function, and one equality function per enum. This increases code size but
  keeps each function simple.
- Adding a variant remains source-breaking for exhaustive matches (ADR-0011);
  the native layout uses declaration order for tags, so variant order is part
  of the generated ABI. Native programs are rebuilt from source, so this is not
  a stable external ABI.

## Reversibility

High at the source level, medium at the generated-C level. The IR exposes
`IrType::Enum`, `ConstructVariant`, and `Match`; code generation can change
layout without touching the checker or interpreter. A future backend can use a
packed tag, interned values, or arena allocation while preserving semantics.

## Future reconsideration criteria

Revisit when any of the following becomes true:

1. Benchmarks show enum allocation dominates a real workload.
2. Native `Any` or dynamic enums are proposed (would need a different layout).
3. FFI needs a stable enum ABI or explicit discriminants (T-021).
4. Generics introduce `Option[T]`/`Result[T, E]` (T-018) and expose
   monomorphization or layout pressure.
5. A cycle collector or ownership model changes the allocation policy (T-012).
