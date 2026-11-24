# Native enum layout: prior art (2026-11-23)

Status: focused follow-up to T-028 and ADR-0012
Access date: 2026-11-23
Scope: how comparable languages lay out payload-carrying enums, and what Auren
should copy or avoid for the native C backend.

## Method and limitations

- Primary sources were fetched directly on 2026-11-23:
  - Rust Reference, type layout:
    https://doc.rust-lang.org/reference/type-layout.html
  - Zig Language Reference, tagged unions:
    https://ziglang.org/documentation/master/
- The Swift book page was requested but returned no usable content in this
  session, so no Swift-specific claim is made here. The earlier prior-art
  review already covers Swift enums at the language-semantics level.
- This note is about representation strategy, not benchmarks. Auren has not
  measured enum allocation cost yet; ADR-0012 records that gap explicitly.

## Findings

### Rust: `repr(C)` enums are a tag plus a union

The Rust Reference specifies that a `#[repr(C)]` enum with fields has the same
representation as a `repr(C)` struct with two fields: a `repr(C)` tag enum with
the fields removed, and a `repr(C)` union of per-variant field structs. It also
documents primitive representations (`repr(u8)`, `repr(C, u8)`) that can shrink
the tag, and notes that the default Rust representation makes no layout
guarantees beyond soundness.

This is the same strategy Auren adopted in ADR-0012: one tagged struct per
enum with a union of per-variant payload structs. Auren differs in that the
generated C is an internal, rebuilt-from-source artifact rather than a stable
external ABI, so the layout can change without a compatibility promise.

### Zig: tagged unions are first-class and switch captures the payload

Zig declares a tagged union as `union(EnumTag) { ... }` and switches on it with
payload capture (`switch (c) { .ok => |value| ... }`). The tag is part of the
union's type and switching is checked for exhaustiveness. Zig's design keeps
the tag and payload in one value; Auren currently heap-allocates the tagged
struct and passes a pointer.

### Cross-cutting lesson

The tag-plus-union representation is the common baseline. The meaningful
choices are:

1. **Tag width**: Auren uses `int64_t`. Rust's primitive representations show
   that a smaller tag is viable when variant counts are bounded. Auren should
   only shrink the tag after a benchmark shows enum-heavy code is allocation-
   or cache-bound, and after deciding how many variants are allowed.
2. **Allocation**: Auren allocates every enum value. Zig values are by-value
   and Rust values are by-value unless boxed. Auren's reference semantics for
   structs/collections make pointer identity observationally safe because enum
   payload slots are immutable, but allocation cost is real and unmeasured.
3. **ABI stability**: Auren's generated C is not a public ABI. Keeping
   declaration order as the tag is fine for rebuilds, but must be documented
   so a future FFI design does not accidentally expose it.

## Decisions for Auren

- **Adopt** the tag-plus-union layout (already implemented in ADR-0012).
- **Adopt** generated display/equality callbacks per enum so collections need
  no special cases.
- **Avoid** claiming ABI stability for the generated C layout.
- **Avoid** shrinking the tag or interning enum values before a measured
  benchmark; premature packing would complicate the backend without evidence.
- **Revisit** tag width, allocation strategy, and possible by-value
  optimization when an enum-heavy benchmark exists (new task after T-030).

## Follow-up

- Add an enum-heavy benchmark to `benchmarks/` before changing the layout.
- If FFI (T-021) exposes enums, define an explicit C ABI representation rather
  than reusing the internal layout.
