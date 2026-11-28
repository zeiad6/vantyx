//! Runtime values.
//!
//! Collections and structs use `Rc<RefCell<...>>` so that assignment aliases
//! the same instance. All public helpers are cycle-safe: user code can build a
//! list that contains itself, so display and equality use depth limits and a
//! visited set instead of recursing forever.

use crate::types::Type;
use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt;
use std::rc::Rc;

pub const MAX_VALUE_DEPTH: usize = 64;

#[derive(Clone)]
pub enum Value {
    Unit,
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(Rc<str>),
    List(Rc<ListValue>),
    /// Insertion-ordered map represented as a flat vector of pairs.
    /// Lookup is linear; v0.1 favors simplicity and determinism over speed.
    Map(Rc<MapValue>),
    /// Immutable fixed-arity tuple. Element values may still reference mutable
    /// collections or structs.
    Tuple(Vec<Value>),
    Struct(Rc<RefCell<StructValue>>),
    /// Enum values are immutable: the variant and payload slots never change
    /// after construction. Payload values may still reference mutable
    /// collections or structs.
    Enum(EnumValue),
}

/// A mutable list plus the most specific element type it is known to satisfy.
///
/// The contract starts as `Any` (no restriction) and can only be strengthened
/// at a successful typed boundary. Every write through any alias is checked
/// against it, so a `List[Int]` cannot be polluted through a `List[Any]` or
/// `Any` alias. `Any` keeps fully dynamic lists usable.
pub struct ListValue {
    pub items: RefCell<Vec<Value>>,
    pub elem_ty: RefCell<Type>,
}

/// A mutable insertion-ordered map plus its key and value contracts.
/// The contracts follow the same one-way strengthening rule as `ListValue`.
pub struct MapValue {
    pub entries: RefCell<Vec<(Value, Value)>>,
    pub key_ty: RefCell<Type>,
    pub value_ty: RefCell<Type>,
}

#[derive(Clone)]
pub struct EnumValue {
    pub enum_name: String,
    pub variant: String,
    pub fields: Vec<FieldSlot>,
}
impl EnumValue {
    pub fn field(&self, name: &str) -> Option<&FieldSlot> {
        self.fields.iter().find(|field| field.name == name)
    }
}

pub struct StructValue {
    pub type_name: String,
    pub fields: Vec<FieldSlot>,
}

#[derive(Clone)]
pub struct FieldSlot {
    pub name: String,
    pub ty: Type,
    pub value: Value,
}

impl StructValue {
    pub fn field(&self, name: &str) -> Option<&FieldSlot> {
        self.fields.iter().find(|field| field.name == name)
    }

    pub fn field_mut(&mut self, name: &str) -> Option<&mut FieldSlot> {
        self.fields.iter_mut().find(|field| field.name == name)
    }
}

impl Value {
    pub fn str(value: impl Into<String>) -> Value {
        let value = value.into();
        Value::Str(Rc::from(value))
    }

    /// Builds a fully dynamic list with no element contract.
    pub fn list(items: Vec<Value>) -> Value {
        Value::list_typed(items, Type::Any)
    }

    /// Builds a list whose elements must conform to `elem_ty`.
    pub fn list_typed(items: Vec<Value>, elem_ty: Type) -> Value {
        Value::List(Rc::new(ListValue {
            items: RefCell::new(items),
            elem_ty: RefCell::new(elem_ty),
        }))
    }

    /// Builds a fully dynamic map with no key or value contract.
    pub fn map(entries: Vec<(Value, Value)>) -> Value {
        Value::map_typed(entries, Type::Any, Type::Any)
    }

    /// Builds a map whose keys and values must conform to the given types.
    pub fn map_typed(entries: Vec<(Value, Value)>, key_ty: Type, value_ty: Type) -> Value {
        Value::Map(Rc::new(MapValue {
            entries: RefCell::new(entries),
            key_ty: RefCell::new(key_ty),
            value_ty: RefCell::new(value_ty),
        }))
    }

    /// The list's current element contract, or `None` for a non-list.
    pub fn list_elem_ty(&self) -> Option<Type> {
        match self {
            Value::List(list) => Some(list.elem_ty.borrow().clone()),
            _ => None,
        }
    }

    /// The map's current key and value contracts, or `None` for a non-map.
    pub fn map_contract(&self) -> Option<(Type, Type)> {
        match self {
            Value::Map(map) => Some((map.key_ty.borrow().clone(), map.value_ty.borrow().clone())),
            _ => None,
        }
    }

    /// Strengthens container contracts after a successful `Any -> T` check.
    ///
    /// Contracts only move from `Any` to a concrete type. Nested containers are
    /// tightened recursively with a visited set so cycles terminate. This is
    /// what prevents a list validated as `List[Int]` from later being mutated
    /// through an older `List[Any]` alias.
    pub fn tighten(&self, ty: &Type) {
        let mut seen = HashSet::new();
        tighten_value(self, ty, 0, &mut seen);
    }

    pub fn struct_value(type_name: impl Into<String>, fields: Vec<FieldSlot>) -> Value {
        Value::Struct(Rc::new(RefCell::new(StructValue {
            type_name: type_name.into(),
            fields,
        })))
    }

    pub fn enum_value(
        enum_name: impl Into<String>,
        variant: impl Into<String>,
        fields: Vec<FieldSlot>,
    ) -> Value {
        Value::Enum(EnumValue {
            enum_name: enum_name.into(),
            variant: variant.into(),
            fields,
        })
    }

    /// The name reported by `type(value)`.
    pub fn type_name(&self) -> String {
        match self {
            Value::Unit => "Unit".to_string(),
            Value::Int(_) => "Int".to_string(),
            Value::Float(_) => "Float".to_string(),
            Value::Bool(_) => "Bool".to_string(),
            Value::Str(_) => "Str".to_string(),
            Value::List(_) => "List".to_string(),
            Value::Map(_) => "Map".to_string(),
            Value::Tuple(_) => "Tuple".to_string(),
            Value::Struct(value) => value.borrow().type_name.clone(),
            Value::Enum(value) => value.enum_name.clone(),
        }
    }

    /// User-facing display form. Strings are shown without quotes at the top
    /// level, but quoted inside collections so the output is unambiguous.
    pub fn display(&self) -> String {
        let mut out = String::new();
        let mut seen = HashSet::new();
        format_value(self, true, 0, &mut seen, &mut out);
        out
    }

    pub fn display_nested(&self) -> String {
        let mut out = String::new();
        let mut seen = HashSet::new();
        format_value(self, false, 0, &mut seen, &mut out);
        out
    }

    pub fn equals(&self, other: &Value) -> bool {
        let mut seen = HashSet::new();
        values_equal(self, other, 0, &mut seen)
    }

    /// Runtime validation at an `Any -> T` boundary.
    ///
    /// `Any` accepts everything. Collection checks are deep, so a `List[Int]`
    /// annotation cannot be satisfied by a list that currently holds strings.
    pub fn conforms_to(&self, ty: &Type) -> bool {
        let mut seen = HashSet::new();
        value_conforms(self, ty, 0, &mut seen)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_nested())
    }
}

// ---- maps ----

pub fn map_find(entries: &[(Value, Value)], key: &Value) -> Option<usize> {
    entries.iter().position(|(k, _)| k.equals(key))
}

pub fn map_get(entries: &[(Value, Value)], key: &Value) -> Option<Value> {
    map_find(entries, key).map(|index| entries[index].1.clone())
}

pub fn map_contains(entries: &[(Value, Value)], key: &Value) -> bool {
    map_find(entries, key).is_some()
}

/// Inserts or replaces a key, preserving the original insertion position.
pub fn map_insert(entries: &mut Vec<(Value, Value)>, key: Value, value: Value) {
    if let Some(index) = map_find(entries, &key) {
        entries[index].1 = value;
    } else {
        entries.push((key, value));
    }
}

// ---- display ----

fn container_id<T>(value: &Rc<T>) -> usize {
    Rc::as_ptr(value) as *const () as usize
}

fn format_value(
    value: &Value,
    top_level: bool,
    depth: usize,
    seen: &mut HashSet<usize>,
    out: &mut String,
) {
    if depth > MAX_VALUE_DEPTH {
        out.push('…');
        return;
    }
    match value {
        Value::Unit => out.push_str("()"),
        Value::Int(n) => out.push_str(&n.to_string()),
        Value::Float(x) => out.push_str(&format_float(*x)),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Str(text) => {
            if top_level {
                out.push_str(text);
            } else {
                push_quoted(text, out);
            }
        }
        Value::List(list) => {
            let id = container_id(list);
            if !seen.insert(id) {
                out.push_str("[...]");
                return;
            }
            out.push('[');
            let borrowed = list.items.borrow();
            for (index, item) in borrowed.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                format_value(item, false, depth + 1, seen, out);
            }
            out.push(']');
            seen.remove(&id);
        }
        Value::Map(map) => {
            let id = container_id(map);
            if !seen.insert(id) {
                out.push_str("{...}");
                return;
            }
            out.push('{');
            let borrowed = map.entries.borrow();
            for (index, (key, item)) in borrowed.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                format_value(key, false, depth + 1, seen, out);
                out.push_str(": ");
                format_value(item, false, depth + 1, seen, out);
            }
            out.push('}');
            seen.remove(&id);
        }
        Value::Tuple(elements) => {
            out.push('(');
            for (index, element) in elements.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                format_value(element, false, depth + 1, seen, out);
            }
            if elements.len() == 1 {
                out.push(',');
            }
            out.push(')');
        }
        Value::Struct(instance) => {
            let id = container_id(instance);
            let borrowed = instance.borrow();
            out.push_str(&borrowed.type_name);
            if !seen.insert(id) {
                out.push_str("(...)");
                return;
            }
            out.push('(');
            for (index, field) in borrowed.fields.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                out.push_str(&field.name);
                out.push('=');
                format_value(&field.value, false, depth + 1, seen, out);
            }
            out.push(')');
            seen.remove(&id);
        }
        Value::Enum(value) => {
            out.push_str(&value.variant);
            if !value.fields.is_empty() {
                out.push('(');
                for (index, field) in value.fields.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    format_value(&field.value, false, depth + 1, seen, out);
                }
                out.push(')');
            }
        }
    }
}

fn push_quoted(text: &str, out: &mut String) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
}

pub(crate) fn format_float(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-Infinity".to_string()
        } else {
            "Infinity".to_string()
        };
    }
    let mut text = format!("{value}");
    if !text.contains('.') && !text.contains('e') && !text.contains('E') {
        text.push_str(".0");
    }
    text
}

// ---- equality ----

fn values_equal(a: &Value, b: &Value, depth: usize, seen: &mut HashSet<(usize, usize)>) -> bool {
    if depth > MAX_VALUE_DEPTH {
        return false;
    }
    match (a, b) {
        (Value::Unit, Value::Unit) => true,
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::List(x), Value::List(y)) => {
            if Rc::ptr_eq(x, y) {
                return true;
            }
            let pair = (container_id(x), container_id(y));
            if !seen.insert(pair) {
                return true;
            }
            let x = x.items.borrow();
            let y = y.items.borrow();
            let equal = x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|(a, b)| values_equal(a, b, depth + 1, seen));
            drop(x);
            drop(y);
            seen.remove(&pair);
            equal
        }
        (Value::Map(x), Value::Map(y)) => {
            if Rc::ptr_eq(x, y) {
                return true;
            }
            let pair = (container_id(x), container_id(y));
            if !seen.insert(pair) {
                return true;
            }
            let x = x.entries.borrow();
            let y = y.entries.borrow();
            let equal = x.len() == y.len()
                && x.iter().all(|(key, value)| {
                    map_find(y.as_slice(), key)
                        .map(|index| values_equal(value, &y[index].1, depth + 1, seen))
                        .unwrap_or(false)
                });
            drop(x);
            drop(y);
            seen.remove(&pair);
            equal
        }
        (Value::Tuple(x), Value::Tuple(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|(a, b)| values_equal(a, b, depth + 1, seen))
        }
        (Value::Struct(x), Value::Struct(y)) => {
            if Rc::ptr_eq(x, y) {
                return true;
            }
            let pair = (container_id(x), container_id(y));
            if !seen.insert(pair) {
                return true;
            }
            let x = x.borrow();
            let y = y.borrow();
            let equal = x.type_name == y.type_name
                && x.fields.len() == y.fields.len()
                && x.fields.iter().zip(y.fields.iter()).all(|(a, b)| {
                    a.name == b.name && values_equal(&a.value, &b.value, depth + 1, seen)
                });
            drop(x);
            drop(y);
            seen.remove(&pair);
            equal
        }
        (Value::Enum(x), Value::Enum(y)) => {
            x.enum_name == y.enum_name
                && x.variant == y.variant
                && x.fields.len() == y.fields.len()
                && x.fields.iter().zip(y.fields.iter()).all(|(a, b)| {
                    a.name == b.name && values_equal(&a.value, &b.value, depth + 1, seen)
                })
        }
        _ => false,
    }
}

// ---- conformance ----

fn value_conforms(
    value: &Value,
    ty: &Type,
    depth: usize,
    seen: &mut HashSet<(usize, Type)>,
) -> bool {
    if matches!(ty, Type::Any) {
        return true;
    }
    if depth > MAX_VALUE_DEPTH {
        return false;
    }
    match (value, ty) {
        (Value::Unit, Type::Unit) => true,
        (Value::Int(_), Type::Int) => true,
        (Value::Float(_), Type::Float) => true,
        (Value::Bool(_), Type::Bool) => true,
        (Value::Str(_), Type::Str) => true,
        (Value::List(list), Type::List(inner)) => {
            if matches!(**inner, Type::Any) {
                return true;
            }
            let key = (container_id(list), ty.clone());
            if !seen.insert(key.clone()) {
                // The same container is already being checked against the same
                // type on this path, so the cycle is coinductively assumed to
                // conform. A revisit under a *different* expected type is a
                // genuine mismatch and must not be accepted.
                return true;
            }
            let ok = list
                .items
                .borrow()
                .iter()
                .all(|item| value_conforms(item, inner, depth + 1, seen));
            seen.remove(&key);
            ok
        }
        (Value::Map(map), Type::Map(key_ty, value_ty)) => {
            if matches!(**key_ty, Type::Any) && matches!(**value_ty, Type::Any) {
                return true;
            }
            let key = (container_id(map), ty.clone());
            if !seen.insert(key.clone()) {
                return true;
            }
            let ok = map.entries.borrow().iter().all(|(key, item)| {
                value_conforms(key, key_ty, depth + 1, seen)
                    && value_conforms(item, value_ty, depth + 1, seen)
            });
            seen.remove(&key);
            ok
        }
        (Value::Tuple(elements), Type::Tuple(types)) => {
            elements.len() == types.len()
                && elements
                    .iter()
                    .zip(types.iter())
                    .all(|(element, ty)| value_conforms(element, ty, depth + 1, seen))
        }
        (Value::Struct(instance), Type::Struct(name)) => instance.borrow().type_name == *name,
        (Value::Enum(value), Type::Enum(name)) => value.enum_name == *name,
        _ => false,
    }
}

// ---- contract strengthening ----

fn tighten_value(value: &Value, ty: &Type, depth: usize, seen: &mut HashSet<(usize, Type)>) {
    if depth > MAX_VALUE_DEPTH || matches!(ty, Type::Any) {
        return;
    }
    match (value, ty) {
        (Value::List(list), Type::List(inner)) => {
            if matches!(**inner, Type::Any) {
                return;
            }
            if matches!(*list.elem_ty.borrow(), Type::Any) {
                *list.elem_ty.borrow_mut() = (**inner).clone();
            }
            let key = (container_id(list), ty.clone());
            if !seen.insert(key.clone()) {
                return;
            }
            for item in list.items.borrow().iter() {
                tighten_value(item, inner, depth + 1, seen);
            }
            seen.remove(&key);
        }
        (Value::Map(map), Type::Map(key_ty, value_ty)) => {
            if matches!(**key_ty, Type::Any) && matches!(**value_ty, Type::Any) {
                return;
            }
            if !matches!(**key_ty, Type::Any) && matches!(*map.key_ty.borrow(), Type::Any) {
                *map.key_ty.borrow_mut() = (**key_ty).clone();
            }
            if !matches!(**value_ty, Type::Any) && matches!(*map.value_ty.borrow(), Type::Any) {
                *map.value_ty.borrow_mut() = (**value_ty).clone();
            }
            let key = (container_id(map), ty.clone());
            if !seen.insert(key.clone()) {
                return;
            }
            for (key, item) in map.entries.borrow().iter() {
                tighten_value(key, key_ty, depth + 1, seen);
                tighten_value(item, value_ty, depth + 1, seen);
            }
            seen.remove(&key);
        }
        (Value::Tuple(elements), Type::Tuple(types)) => {
            for (element, ty) in elements.iter().zip(types.iter()) {
                tighten_value(element, ty, depth + 1, seen);
            }
        }
        (Value::Struct(instance), Type::Struct(_)) => {
            let id = container_id(instance);
            let key = (id, ty.clone());
            if !seen.insert(key.clone()) {
                return;
            }
            let fields: Vec<(Type, Value)> = instance
                .borrow()
                .fields
                .iter()
                .map(|field| (field.ty.clone(), field.value.clone()))
                .collect();
            for (field_ty, field_value) in fields {
                tighten_value(&field_value, &field_ty, depth + 1, seen);
            }
            seen.remove(&key);
        }
        (Value::Enum(value), Type::Enum(_)) => {
            for field in &value.fields {
                tighten_value(&field.value, &field.ty, depth + 1, seen);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ints_and_floats_never_compare_equal() {
        assert!(!Value::Int(1).equals(&Value::Float(1.0)));
    }

    #[test]
    fn maps_compare_by_contents_not_order() {
        let a = Value::map(vec![
            (Value::str("a"), Value::Int(1)),
            (Value::str("b"), Value::Int(2)),
        ]);
        let b = Value::map(vec![
            (Value::str("b"), Value::Int(2)),
            (Value::str("a"), Value::Int(1)),
        ]);
        assert!(a.equals(&b));
    }

    #[test]
    fn lists_compare_structurally() {
        let a = Value::list(vec![Value::Int(1), Value::str("x")]);
        let b = Value::list(vec![Value::Int(1), Value::str("x")]);
        assert!(a.equals(&b));
        assert!(!a.equals(&Value::list(vec![Value::Int(1)])));
    }

    #[test]
    fn display_quotes_nested_strings() {
        let list = Value::list(vec![Value::str("a"), Value::str("b")]);
        assert_eq!(list.display(), "[\"a\", \"b\"]");
        assert_eq!(Value::str("a").display(), "a");
    }

    #[test]
    fn floats_always_show_a_decimal_point() {
        assert_eq!(Value::Float(1.0).display(), "1.0");
        assert_eq!(Value::Float(1.5).display(), "1.5");
    }

    #[test]
    fn self_referential_list_display_terminates() {
        let list = Value::list(Vec::new());
        if let Value::List(rc) = &list {
            rc.items.borrow_mut().push(list.clone());
        }
        let text = list.display();
        assert!(text.contains("[...]"));
    }

    #[test]
    fn self_referential_list_equality_terminates() {
        let a = Value::list(Vec::new());
        let b = Value::list(Vec::new());
        if let (Value::List(arc), Value::List(brc)) = (&a, &b) {
            arc.items.borrow_mut().push(a.clone());
            brc.items.borrow_mut().push(b.clone());
        }
        assert!(a.equals(&b));
    }

    #[test]
    fn conformance_checks_nested_elements() {
        let value = Value::list(vec![Value::Int(1), Value::Int(2)]);
        assert!(value.conforms_to(&Type::List(Box::new(Type::Int))));
        assert!(!value.conforms_to(&Type::List(Box::new(Type::Str))));
        assert!(value.conforms_to(&Type::List(Box::new(Type::Any))));
    }

    #[test]
    fn any_accepts_everything() {
        assert!(Value::Unit.conforms_to(&Type::Any));
        assert!(Value::list(vec![Value::str("x")]).conforms_to(&Type::Any));
    }

    #[test]
    fn tightening_is_one_way_and_recursive() {
        let list = Value::list_typed(vec![Value::list(vec![Value::Int(1)])], Type::Any);
        list.tighten(&Type::List(Box::new(Type::List(Box::new(Type::Int)))));
        assert_eq!(list.list_elem_ty(), Some(Type::List(Box::new(Type::Int))));
        let Value::List(outer) = &list else {
            panic!("expected a list");
        };
        let inner = outer.items.borrow()[0].clone();
        assert_eq!(
            inner.list_elem_ty(),
            Some(Type::Int),
            "nested containers must be tightened too"
        );

        // A concrete contract is never widened back to `Any`.
        list.tighten(&Type::List(Box::new(Type::Any)));
        assert_eq!(list.list_elem_ty(), Some(Type::List(Box::new(Type::Int))));
    }

    #[test]
    fn typed_maps_report_their_contracts() {
        let map = Value::map_typed(vec![(Value::Int(1), Value::str("a"))], Type::Int, Type::Str);
        assert_eq!(map.map_contract(), Some((Type::Int, Type::Str)));
        assert_eq!(
            Value::map(Vec::new()).map_contract(),
            Some((Type::Any, Type::Any))
        );
    }

    #[test]
    fn conformance_rejects_a_self_cycle_under_a_mismatched_type() {
        let list = Value::list_typed(Vec::new(), Type::Any);
        if let Value::List(rc) = &list {
            rc.items.borrow_mut().push(list.clone());
        }
        assert!(list.conforms_to(&Type::List(Box::new(Type::Any))));
        assert!(list.conforms_to(&Type::List(Box::new(Type::List(Box::new(Type::Any))))));
        assert!(!list.conforms_to(&Type::List(Box::new(Type::List(Box::new(Type::Int))))));
    }

    #[test]
    fn conformance_rejects_a_self_referential_map_under_a_mismatched_type() {
        let map = Value::map_typed(Vec::new(), Type::Any, Type::Any);
        if let Value::Map(rc) = &map {
            rc.entries
                .borrow_mut()
                .push((Value::str("self"), map.clone()));
        }
        assert!(map.conforms_to(&Type::Map(
            Box::new(Type::Str),
            Box::new(Type::Map(Box::new(Type::Str), Box::new(Type::Any)))
        )));
        assert!(!map.conforms_to(&Type::Map(
            Box::new(Type::Str),
            Box::new(Type::Map(Box::new(Type::Str), Box::new(Type::Int)))
        )));
    }

    #[test]
    fn conformance_rejects_a_mutual_cycle_under_a_mismatched_type() {
        let a = Value::list_typed(Vec::new(), Type::Any);
        let b = Value::list_typed(Vec::new(), Type::Any);
        if let (Value::List(arc), Value::List(brc)) = (&a, &b) {
            arc.items.borrow_mut().push(b.clone());
            brc.items.borrow_mut().push(a.clone());
        }
        assert!(a.conforms_to(&Type::List(Box::new(Type::List(Box::new(Type::Any))))));
        assert!(!a.conforms_to(&Type::List(Box::new(Type::List(Box::new(Type::Int))))));
    }

    #[test]
    fn tightening_handles_cycles() {
        let list = Value::list_typed(Vec::new(), Type::Any);
        if let Value::List(rc) = &list {
            rc.items.borrow_mut().push(list.clone());
        }
        list.tighten(&Type::List(Box::new(Type::List(Box::new(Type::Int)))));
        assert_eq!(list.list_elem_ty(), Some(Type::List(Box::new(Type::Int))));
    }

    #[test]
    fn struct_conformance_is_nominal() {
        let value = Value::struct_value("Point", Vec::new());
        assert!(value.conforms_to(&Type::Struct("Point".into())));
        assert!(!value.conforms_to(&Type::Struct("Other".into())));
    }

    fn enum_slot(name: &str, ty: Type, value: Value) -> FieldSlot {
        FieldSlot {
            name: name.to_string(),
            ty,
            value,
        }
    }

    #[test]
    fn enums_display_without_payload_names() {
        assert_eq!(
            Value::enum_value("Shape", "Empty", Vec::new()).display(),
            "Empty"
        );
        assert_eq!(
            Value::enum_value(
                "Shape",
                "Circle",
                vec![enum_slot("radius", Type::Int, Value::Int(2))]
            )
            .display(),
            "Circle(2)"
        );
    }

    #[test]
    fn enum_equality_is_nominal_and_structural() {
        let make = |variant: &str, number: i64| {
            Value::enum_value(
                "Flag",
                variant,
                vec![enum_slot("number", Type::Int, Value::Int(number))],
            )
        };
        assert!(make("On", 1).equals(&make("On", 1)));
        assert!(!make("On", 1).equals(&make("On", 2)));
        assert!(!make("On", 1).equals(&make("Off", 1)));
    }

    #[test]
    fn enum_conformance_uses_the_enum_name() {
        let value = Value::enum_value("Flag", "On", Vec::new());
        assert!(value.conforms_to(&Type::Enum("Flag".into())));
        assert!(!value.conforms_to(&Type::Enum("Other".into())));
        assert_eq!(value.type_name(), "Flag");
    }

    #[test]
    fn tuples_display_with_nested_string_quoting() {
        let value = Value::Tuple(vec![
            Value::Int(1),
            Value::str("x"),
            Value::Tuple(vec![Value::Bool(true)]),
        ]);
        assert_eq!(value.display(), "(1, \"x\", (true,))");
        assert_eq!(Value::Tuple(vec![Value::Int(1)]).display(), "(1,)");
        assert_eq!(Value::Tuple(Vec::new()).display(), "()");
        assert_eq!(value.type_name(), "Tuple");
    }

    #[test]
    fn tuples_compare_structurally_and_never_with_other_kinds() {
        let make = |n: i64| Value::Tuple(vec![Value::Int(n), Value::str("x")]);
        assert!(make(1).equals(&make(1)));
        assert!(!make(1).equals(&make(2)));
        assert!(!make(1).equals(&Value::list(vec![Value::Int(1), Value::str("x")])));
        assert!(!Value::Tuple(vec![Value::Int(1)]).equals(&Value::Tuple(Vec::new())));
    }

    #[test]
    fn tuple_conformance_and_tightening_are_element_wise() {
        let inner = Value::list(vec![Value::Int(1)]);
        let value = Value::Tuple(vec![inner.clone(), Value::str("x")]);
        let ty = Type::Tuple(vec![Type::List(Box::new(Type::Int)), Type::Str]);
        assert!(value.conforms_to(&ty));
        assert!(!value.conforms_to(&Type::Tuple(vec![
            Type::List(Box::new(Type::Str)),
            Type::Str
        ])));
        assert!(!value.conforms_to(&Type::Tuple(vec![Type::List(Box::new(Type::Int))])));
        assert_eq!(inner.list_elem_ty(), Some(Type::Any));
        value.tighten(&ty);
        assert_eq!(inner.list_elem_ty(), Some(Type::Int));
    }
}
