//! Vantyx types and type-expression resolution.

use crate::ast::TypeExpr;
use crate::span::Span;
use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Float,
    Bool,
    Str,
    Unit,
    /// Gradual dynamic type. Operations are checked at runtime.
    Any,
    List(Box<Type>),
    Map(Box<Type>, Box<Type>),
    /// Fixed-arity, structural tuple type. A zero-element tuple is never
    /// constructed; `Unit` remains the zero-value type.
    Tuple(Vec<Type>),
    Struct(String),
    Enum(String),
}

impl Type {
    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Int | Type::Float)
    }

    pub fn is_any(&self) -> bool {
        matches!(self, Type::Any)
    }

    /// Whether a value of `self` may be used where `expected` is required.
    /// `Any` is accepted in both directions and validated at runtime.
    pub fn assignable_to(&self, expected: &Type) -> bool {
        if matches!(self, Type::Any) || matches!(expected, Type::Any) {
            return true;
        }
        match (self, expected) {
            (Type::Int, Type::Int)
            | (Type::Float, Type::Float)
            | (Type::Bool, Type::Bool)
            | (Type::Str, Type::Str)
            | (Type::Unit, Type::Unit) => true,
            (Type::List(a), Type::List(b)) => a.assignable_to(b),
            (Type::Map(ak, av), Type::Map(bk, bv)) => ak.assignable_to(bk) && av.assignable_to(bv),
            (Type::Tuple(a), Type::Tuple(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b.iter())
                        .all(|(actual, expected)| actual.assignable_to(expected))
            }
            (Type::Struct(a), Type::Struct(b)) => a == b,
            (Type::Enum(a), Type::Enum(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Int => write!(f, "Int"),
            Type::Float => write!(f, "Float"),
            Type::Bool => write!(f, "Bool"),
            Type::Str => write!(f, "Str"),
            Type::Unit => write!(f, "Unit"),
            Type::Any => write!(f, "Any"),
            Type::List(inner) => write!(f, "List[{inner}]"),
            Type::Map(key, value) => write!(f, "Map[{key}, {value}]"),
            Type::Tuple(elements) => {
                write!(f, "(")?;
                for (index, element) in elements.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{element}")?;
                }
                if elements.len() == 1 {
                    write!(f, ",")?;
                }
                write!(f, ")")
            }
            Type::Struct(name) | Type::Enum(name) => write!(f, "{name}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FieldInfo {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct StructInfo {
    pub name: String,
    pub fields: Vec<FieldInfo>,
    pub span: Span,
}

impl StructInfo {
    pub fn field(&self, name: &str) -> Option<&FieldInfo> {
        self.fields.iter().find(|f| f.name == name)
    }
}

#[derive(Clone, Debug)]
pub struct VariantInfo {
    pub name: String,
    pub fields: Vec<FieldInfo>,
    pub span: Span,
}

impl VariantInfo {
    pub fn field(&self, name: &str) -> Option<&FieldInfo> {
        self.fields.iter().find(|f| f.name == name)
    }
}

#[derive(Clone, Debug)]
pub struct EnumInfo {
    pub name: String,
    pub variants: Vec<VariantInfo>,
    pub span: Span,
}

impl EnumInfo {
    pub fn variant(&self, name: &str) -> Option<&VariantInfo> {
        self.variants.iter().find(|v| v.name == name)
    }
}

/// Resolve a syntactic type to a semantic type.
pub fn resolve_type_expr(
    expr: &TypeExpr,
    structs: &HashMap<String, StructInfo>,
    enums: &HashMap<String, EnumInfo>,
) -> Result<Type, String> {
    let (name, args) = match &expr.kind {
        crate::ast::TypeExprKind::Name { name, args } => (name.as_str(), args),
        crate::ast::TypeExprKind::Tuple(elements) => {
            if elements.is_empty() {
                return Ok(Type::Unit);
            }
            let resolved = elements
                .iter()
                .map(|element| resolve_type_expr(element, structs, enums))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(Type::Tuple(resolved));
        }
    };
    let builtin = match name {
        "Int" => Some(Type::Int),
        "Float" => Some(Type::Float),
        "Bool" => Some(Type::Bool),
        "Str" => Some(Type::Str),
        "Unit" => Some(Type::Unit),
        "Any" => Some(Type::Any),
        "List" => {
            if args.len() != 1 {
                return Err(format!(
                    "type `List` expects 1 type argument, found {}",
                    args.len()
                ));
            }
            Some(Type::List(Box::new(resolve_type_expr(
                &args[0], structs, enums,
            )?)))
        }
        "Map" => {
            if args.len() != 2 {
                return Err(format!(
                    "type `Map` expects 2 type arguments, found {}",
                    args.len()
                ));
            }
            Some(Type::Map(
                Box::new(resolve_type_expr(&args[0], structs, enums)?),
                Box::new(resolve_type_expr(&args[1], structs, enums)?),
            ))
        }
        _ => None,
    };

    if let Some(ty) = builtin {
        if !args.is_empty() && !matches!(name, "List" | "Map") {
            return Err(format!("type `{name}` does not take type arguments"));
        }
        return Ok(ty);
    }

    if structs.contains_key(name) {
        if !args.is_empty() {
            return Err(format!("struct `{name}` does not take type arguments"));
        }
        return Ok(Type::Struct(name.to_string()));
    }

    if enums.contains_key(name) {
        if !args.is_empty() {
            return Err(format!("enum `{name}` does not take type arguments"));
        }
        return Ok(Type::Enum(name.to_string()));
    }

    Err(format!("unknown type `{name}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assignability_respects_nominal_structs() {
        let a = Type::Struct("A".into());
        let b = Type::Struct("B".into());
        assert!(a.assignable_to(&a));
        assert!(!a.assignable_to(&b));
        assert!(a.assignable_to(&Type::Any));
        assert!(Type::Any.assignable_to(&a));
    }

    #[test]
    fn lists_are_covariant_toward_any() {
        assert!(Type::List(Box::new(Type::Int)).assignable_to(&Type::List(Box::new(Type::Any))));
        assert!(!Type::List(Box::new(Type::Int)).assignable_to(&Type::List(Box::new(Type::Str))));
    }

    #[test]
    fn no_implicit_numeric_conversion() {
        assert!(!Type::Int.assignable_to(&Type::Float));
        assert!(!Type::Float.assignable_to(&Type::Int));
    }

    #[test]
    fn tuples_are_structural_and_position_sensitive() {
        let pair = Type::Tuple(vec![Type::Int, Type::Str]);
        assert!(pair.assignable_to(&Type::Tuple(vec![Type::Int, Type::Str])));
        assert!(!pair.assignable_to(&Type::Tuple(vec![Type::Str, Type::Int])));
        assert!(!pair.assignable_to(&Type::Tuple(vec![Type::Int])));
        assert!(pair.assignable_to(&Type::Tuple(vec![Type::Any, Type::Any])));
        assert!(Type::Any.assignable_to(&pair));
    }

    #[test]
    fn tuple_display_uses_a_trailing_comma_for_one_element() {
        assert_eq!(Type::Tuple(vec![Type::Int]).to_string(), "(Int,)");
        assert_eq!(
            Type::Tuple(vec![Type::Int, Type::Str]).to_string(),
            "(Int, Str)"
        );
        assert_eq!(
            Type::Tuple(vec![Type::Tuple(vec![Type::Int]), Type::Bool]).to_string(),
            "((Int,), Bool)"
        );
    }
}
