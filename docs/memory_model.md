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
3. **Dynamic containers**: `List[Any]` can hold mixed values; typed reads are validated at annotated boundaries. Each list stores an element contract and each map a key/value contract; a successful typed validation strengthens `Any` contracts recursively, and later writes through any alias are checked against them (ADR-0057). This metadata does not change reference identity, display, or equality.
4. **Resource exhaustion**: large allocations can abort the process; there is no sandbox or memory quota.
5. **Enum payload aliasing**: copying an enum copies its payload slots, but list/map/struct payloads still alias. Mutating such a payload is visible through every copy, exactly like struct fields.

## Native backend (v0.1 subset)

- Native values are unboxed C values: `int64_t`, `double`, `bool`, `const char*`, pointers to heap-allocated structs, `VantyxList*` for lists, `VantyxMap*` for maps, and `Vantyx_e_<Enum>*` for enums.
- Strings, structs, list buffers, map key/value buffers, scalar-key map indexes, and enum values are allocated with `malloc`/`calloc`/`realloc` and are not generally reclaimed. Native programs are short-lived CLI processes; this is a documented limitation, not a memory-safety claim. Interpolation, string `+` chains, and list/map/struct/enum display build their result in one growable buffer (`VantyxStrBuilder`, ADR-0052, ADR-0053) instead of retaining every intermediate concatenation; the final buffer is still leaked. Builder growth uses `realloc`, which releases the previous smaller block. Nested display renders each element, key, value, and field into a temporary, appends it, and immediately frees it (`vantyx_display_free`, ADR-0054), so large displays no longer retain one string per rendered component. Top-level `print` also frees its rendered `Int`, `Float`, `List`, `Map`, `Struct`, and `Enum` strings after `fputs` copies them (`vantyx_print_owned`, T-121), while borrowed `Str` and static `Bool`/`Unit` renderings are never freed. Final display results from `str`/interpolation and every other native allocation remain intentionally unreclaimed; this is not a general reclamation strategy.
- Struct assignment aliases the same heap object, matching the interpreter's reference semantics. List and map assignment aliases the same header; growth reallocates only the element buffer, so aliases stay valid. Enum values are immutable; assigning an enum aliases the same tagged struct, which is observationally equivalent because payload slots cannot be reassigned.
- Native lists use typed contiguous storage plus generated element descriptors (ADR-0009). Native maps use insertion-ordered parallel key/value buffers plus generated key/value descriptors (ADR-0010); scalar keys additionally use an open-addressing index of entry positions at a 50% load factor (ADR-0016), while struct/enum/list keys keep linear structural search. `remove(map, key)` compacts the entry buffers in place, rebuilds the scalar-key index, and leaves the allocated buffers and removed values unreclaimed. Native enums use a tagged struct with a union of per-variant payload structs plus generated display/equality callbacks (ADR-0012). Scalar elements are unboxed; structs, enums, and nested collections are stored as pointers.
- `List[Any]`, `Map[Any, *]`, `Any` enum payloads, and `match` on `Any` are rejected with `E0401`. String indexing and iteration return newly allocated one-character strings, matching the interpreter's value semantics.
- Integer arithmetic, list/string indices, and map capacity growth are checked; missing map keys raise `R0008`; allocation failure terminates with a runtime error.
- List, map, struct, and enum display detect cycles through mutable containers. Structural equality tracks visited container pairs, so two distinct cycles compare coinductively, matching the interpreter.
- Native calls are limited to depth 256 (`R0012`), preventing runaway recursion from exhausting the C stack. Both engines report the failing call site, including across module boundaries (T-104).
- Future reclamation must preserve reference semantics and document its allocation policy in an ADR before implementation. The scalar-key map index (ADR-0016) adds roughly 16 bytes per entry at its 50% load factor; structural-key hashing remains a documented follow-up, not an implemented feature.

## ADR-0056 Stage 1 — conservative ownership facts (T-122)

`src/ownership.rs` classifies typed IR expressions as `Fresh`/`Borrowed`/`Aliased`/`MayEscape` with a conservative pure `ownership()` and a `verify_reclamation()` that would reject a reclamation site unless `Fresh`. **No generated C changes; nothing is reclaimed by this analysis; unknown=>`MayEscape`; `str(Str)`/`trim` are not `Fresh`.** `Fresh` means not aliased at construction, not last-use. This is Stage 1 only; nothing from Stage 2+ beyond T-121 is wired.

## Planned evolution

ADR-0056 defines the staged native reclamation plan. The order is deliberate:

1. **Ownership facts in typed IR.** Classify expressions and locals as fresh,
   borrowed, aliased, or escaping; no generated-C change.
2. **Fresh temporaries.** Free compiler-generated strings and other values at
   their last use when no alias can observe them.
3. **Escape analysis and arenas.** Keep non-escaping locals on the stack or in
   a function/loop arena, with conservative promotion of anything that may
   escape.
4. **Escaping values.** A separately measured ADR amendment chooses between
   reference counting plus cycle collection and a precise tracing collector.
   Neither is implemented yet.
5. **Interpreter cycles.** A separate decision if long-running interpreter use
   becomes a target.

Optional ownership/borrow annotations, value-semantics collections with
copy-on-write, and arena/region APIs for scientific and systems workloads
remain long-term language-design questions, not part of the staged backend
plan. Ordinary Vantyx code must never be forced to write lifetime annotations.
Advanced memory control is opt-in.

Until a stage lands and is verified, the current intentional-leak behavior
stands, including the native display-temporary frees from ADR-0054.
