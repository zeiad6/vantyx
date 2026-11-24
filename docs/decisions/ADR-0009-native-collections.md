# ADR-0009: Native collection representation

- Status: Accepted
- Date: 2026-11-21
- Task: T-024
- Supersedes: none

## Context

The interpreter gives `List` reference semantics: `let b = a` aliases the same
list, `push(b, x)` is visible through `a`, and elements may be nested lists or
structs. The native backend lowers checked Auren to self-contained GNU C11 and
must preserve those semantics for the statically typed subset. It has no
installed runtime library, links only libc/libm, and intentionally leaks
allocations for short-lived CLI programs.

The IR already had to answer one question before code generation: how is a
`List[T]` laid out, displayed, compared, grown, and bounds-checked when `T` can
be `Unit`, `Int`, `Float`, `Bool`, `Str`, a struct, or another list?

## Options considered

### 1. Tagged `AurenValue` element union

Every element is a `{ kind, union }` value, closely matching the interpreter's
dynamic `Value`.

- Advantages: one representation for every element type; `List[Any]` would be
  straightforward later; display and equality switch on the runtime tag.
- Disadvantages: 16+ bytes per scalar element, runtime tags on every access,
  and the native backend would quietly become dynamically typed. It does not
  match the project rule that native code is a statically typed subset.

### 2. Typed contiguous storage plus a type descriptor (chosen)

`AurenList` stores `void* items`, `len`, `cap`, `elem_size`, and a pointer to a
static `AurenTypeDesc`. Elements are stored by value (`int64_t`, `double`,
`bool`, `auren_str`, `AurenList*`, `Auren_s_*`). The descriptor describes the
element kind and, for structs, generated display/equality functions.

- Advantages: unboxed scalar storage, one small runtime header per list,
  compile-time typed accessors, nested lists and lists of structs fall out of
  the same representation, and no third-party runtime dependency.
- Disadvantages: `List[Any]` is not representable (it is rejected with
  `E0401`), and display/equality need generated descriptors.

### 3. One generated C list type and helper set per element type

Generate `AurenList_Int`, `AurenList_Point`, and so on.

- Advantages: fully typed C with no descriptors.
- Disadvantages: code duplication, awkward identity when the same logical
  list type crosses function boundaries, and recursive types require
  forward-declaration machinery per instantiation.

### 4. Link a C++ `std::vector` runtime

- Advantages: mature growth and algorithms.
- Disadvantages: introduces a C++ ABI/runtime, complicates the "C11 + libc"
  guarantee, and still needs Auren-specific display/equality.

## Decision

Use option 2.

- `AurenList` is a stable heap header. Growth reallocates only `items`, so
  aliases remain valid because they point at the header, not the buffer.
- List literals allocate a header and push elements left-to-right.
- Reads and writes go through `auren_list_slot`, which reports negative and
  out-of-bounds indices as `R0003` with the same messages as the interpreter.
- `push` grows capacity from 4 and doubles, with overflow checks. `pop`
  reports `R0003` on an empty list.
- `len` returns the header length; `contains` and display use the element
  descriptor, so lists of structs compare and print structurally.
- `for x in list` evaluates the list once, snapshots its length, reads elements
  live, and stops if the list shrinks. This matches the interpreter.
- `type(list)` reports `List`, matching the interpreter.
- All list storage is allocated with `malloc`/`realloc` and intentionally never
  freed, consistent with native strings and structs in v0.1.

Descriptors are emitted inner-first for every list and struct type reachable
from the IR. Primitive descriptors are emitted once. Struct descriptors carry
generated display and structural-equality functions. Display detects list and
struct cycles and prints `[...]` / `TypeName(...)`; equality is depth-limited.

## Consequences

- The native subset now covers `List[T]` for every statically known `T`,
  including nested lists, lists of structs, list fields, parameters, returns,
  globals, and aliases.
- `Map` and `List[Any]` remain rejected with `E0401`; `Map` support will
  need a separate representation decision. String indexing and iteration are
  independent of this list representation (T-025). *(Map support was added in
  ADR-0010.)*
- Native programs still leak all allocations and have no native call-depth
  guard. These remain documented limitations, not safety claims. *(A call-depth
  guard was added later; leaks remain.)*
- Cyclic structures display correctly, but structural equality of two distinct
  cycles is depth-limited rather than coinductive. *(Superseded: equality is
  now coinductive.)*
- Because Auren has no user-visible interior pointers, `realloc` cannot leave a
  dangling element reference in source programs.

## Reversibility

High. The IR expresses list operations as `IrExprKind::List`, `Index`,
`IrPlace::Index`, `ForList`, and builtins; the C layout is an implementation
detail behind those nodes. A later tagged representation for `List[Any]`, or a
reference-counted/arena representation, can be introduced without changing the
surface language.

## Reconsideration criteria

Revisit this decision when any of the following becomes a goal:

1. `List[Any]` or heterogeneous collections are required.
2. Native memory reclamation (reference counting, arenas, or ownership) is
   introduced.
3. Benchmarks show list access or growth is a bottleneck.
4. `Map` or tensor types need a shared runtime value representation.
5. Native concurrency requires allocator or descriptor thread-safety rules.

## Update (2026-11-22)

- `Map[K, V]` support was subsequently added under ADR-0010; it reuses the
  descriptor mechanism described here with separate key/value descriptors.
- Native structural equality is now cycle-safe (coinductive visited-pair
  tracking), so the "depth-limited" equality note above is historical.
- A native call-depth guard (`R0012`, limit 256) now exists.
- `List[Any]` remains rejected with `E0401`.
