# Type System

Vantyx uses **gradual typing**: static checking where types are known, runtime validation where they are not.

## Types

```
Int, Float, Bool, Str, Unit, Any
List[T], Map[K, V]
Struct(Name), Enum(Name)
```

`Any` is the top dynamic type. It is not a union; it means "unknown until runtime". Operations on `Any` are compiled to checked dynamic operations.

## Inference rules (v0.1)

- Literals have their natural types.
- `let x = expr` gets the type of `expr`; an annotation constrains and checks it.
- Arithmetic: `Int op Int → Int` (except `/ → Float`), `Float op Float → Float`; mixing is an error.
- Comparison → `Bool`; logical operators require `Bool`.
- List literals unify element types; failure to unify yields `List[Any]`.
- Map literals unify key and value types independently; failure yields `Map[Any, Any]`.
- Calls use declared parameter types; unannotated parameters are `Any`.
- Unannotated return types are inferred from returns. If returns disagree, the result is `Any` (runtime checked) unless an annotation demands otherwise.
- Struct constructor arguments are checked against field types.
- Variant constructors return their owning enum type; payload arguments are checked positionally against field types. Variant constructor names are globally unique.
- A match on a closed enum is exhaustive or must include `_` (`E0316`). Pattern bindings receive the payload field types. A match on `Any` warns (`W0004`) and is checked at runtime.
- Match-expression arms must agree on one type; otherwise `E0301`.
- Indexing: `List[T] → T`, `Str → Str`, `Map[K,V] → V`.
- Field access requires a struct type; on `Any` it is dynamic. Enum values have no fields; use `match`.

## Soundness boundary

Static types are trusted. At every boundary where an `Any` value enters a typed position (annotated binding, parameter, return), the runtime validates the value. Therefore a well-typed program cannot observe a value of the wrong type without a runtime error.

## Mutability

- `let` freezes the binding, not the referenced value.
- `var` allows rebinding.
- Function parameters are immutable bindings.
- Struct fields and collection contents are mutable through any binding; this is documented reference semantics.
- Enum values are immutable: variant and payload slots cannot be reassigned. Payload values that are references keep their normal mutability and aliasing.

## Native subset

The native backend only accepts statically known types it can represent: `Unit`, `Int`, `Float`, `Bool`, `Str`, `List[T]`, `Map[K, V]`, nominal structs, and nominal enums. `Any`, `List[Any]`, `Map[Any, *]`, `Any` enum payloads, `match` on `Any`, and unannotated value-returning functions are rejected with `E0401` rather than dynamically checked. List/string indexing, map indexing, key iteration, and `match` over closed enums and scalar literals with optional guards are supported for the static subset. Gradual programs that need `Any` must run under the interpreter.

## Planned

- Flow-sensitive narrowing (`if x != nil`, tagged unions).
- Generics with monomorphization or dictionaries.
- Traits/constraints.
- Effect annotations.
- Ownership/borrow types as an opt-in advanced layer.
- Nested/or-patterns and generic enums. Scalar literal patterns for `Int`,
  `Str`, and `Bool` (T-032) and `Bool`-checked guards (T-033) are supported.
