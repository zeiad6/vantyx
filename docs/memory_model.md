# Memory Model

## Current model (v0.1)

- All Vantyx values live in Rust-managed memory. There is no user-visible pointer arithmetic and no `unsafe` in Vantyx code.
- Scalars (`Int`, `Float`, `Bool`, `Unit`) are copied.
- `Str` is immutable and reference-counted.
- `List`, `Map`, and structs use reference semantics via `Rc<RefCell<...>>`.
- Enum values are immutable records: the enum name, variant, and payload slots
  cannot be reassigned after construction. Payload values may be reference
  values, which keep their normal alias and mutation behavior.
- Assignment aliases reference values; it does not deep-copy.
- Bounds, arithmetic, and type checks are performed at runtime.
- The interpreter enforces a call-depth limit to convert runaway recursion into a diagnostic instead of a native stack overflow.

## Safety properties

| Property | Status |
| --- | --- |
| No use-after-free | Guaranteed by Rust ownership/`Rc` |
| No double free | Guaranteed by Rust |
| No dangling references | Guaranteed by Rust |
| No buffer overflow | Bounds-checked |
| No uninitialized reads | All bindings initialized |
| No data races | Single-threaded runtime; concurrency not implemented |
| No memory leaks | **Not guaranteed**: `Rc` cycles leak |

## Known limitations

1. **Reference cycles** between lists/maps/structs leak. A cycle collector or ownership analysis is planned.
2. **Aliasing surprises**: `let a = [1]; let b = a; push(b, 2)` changes `a`. This is documented reference semantics, not memory unsafety.
3. **Dynamic containers**: `List[Any]` can hold mixed values; typed reads are validated at annotated boundaries.
4. **Resource exhaustion**: large allocations can abort the process; there is no sandbox or memory quota.
5. **Enum payload aliasing**: copying an enum copies its payload slots, but list/map/struct payloads still alias. Mutating such a payload is visible through every copy, exactly like struct fields.

## Native backend (v0.1 subset)

- Native values are unboxed C values: `int64_t`, `double`, `bool`, `const char*`, pointers to heap-allocated structs, `VantyxList*` for lists, `VantyxMap*` for maps, and `Vantyx_e_<Enum>*` for enums.
- Strings, structs, list buffers, map buffers, and enum values are allocated with `malloc`/`realloc` and **never freed**. Native programs are short-lived CLI processes; this is a documented limitation, not a memory-safety claim.
- Struct assignment aliases the same heap object, matching the interpreter's reference semantics. List and map assignment aliases the same header; growth reallocates only the element buffer, so aliases stay valid. Enum values are immutable; assigning an enum aliases the same tagged struct, which is observationally equivalent because payload slots cannot be reassigned.
- Native lists use typed contiguous storage plus generated element descriptors (ADR-0009). Native maps use insertion-ordered parallel key/value buffers plus generated key/value descriptors (ADR-0010). Native enums use a tagged struct with a union of per-variant payload structs plus generated display/equality callbacks (ADR-0012). Scalar elements are unboxed; structs, enums, and nested collections are stored as pointers.
- `List[Any]`, `Map[Any, *]`, `Any` enum payloads, and `match` on `Any` are rejected with `E0401`. String indexing and iteration return newly allocated one-character strings, matching the interpreter's value semantics.
- Integer arithmetic, list/string indices, and map capacity growth are checked; missing map keys raise `R0008`; allocation failure terminates with a runtime error.
- List, map, struct, and enum display detect cycles through mutable containers. Structural equality tracks visited container pairs, so two distinct cycles compare coinductively, matching the interpreter.
- Native calls are limited to depth 256 (`R0012`), preventing runaway recursion from exhausting the C stack.
- Future reclamation must preserve reference semantics and document its allocation policy in an ADR before implementation. Native map performance can be revisited under ADR-0010's criteria.

## Planned evolution

1. **Escape analysis** to keep non-escaping values on the stack/unboxed.
2. **Optional ownership/borrow annotations** for zero-cost, deterministic memory management in performance-critical code.
3. **Value-semantics collections** with copy-on-write for predictable aliasing.
4. **Cycle collector** or weak references for reference types.
5. **Arena/region APIs** for scientific and systems workloads.

Ordinary Vantyx code must never be forced to write lifetime annotations. Advanced memory control is opt-in.
