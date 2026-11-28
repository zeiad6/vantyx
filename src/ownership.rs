//! Conservative ownership facts over typed IR (ADR-0056 Stage 1).
//!
//! This module adds **no code generation change**. It classifies each typed
//! [`IrExpr`] into an [`Ownership`] fact and provides a verifier that would
//! reject a hypothetical reclamation site unless its value is proven `Fresh`
//! (or a local proven non-escaping, which Stage 1 does not attempt). Nothing
//! calls the verifier to alter output; existing reclamation (ADR-0054,
//! T-121) is unchanged. The rule is **conservative**: any unknown construct is
//! `MayEscape`.
//!
//! Crucially, ownership is proven from the *runtime contract*, not syntax.
//! In particular `str(Str)` returns its argument and `trim(...)` can return
//! its input unchanged, so neither is `Fresh`. `Fresh` here means "a new
//! allocation not aliased at construction"; it is **not** a last-use proof
//! (that needs Stage-2 flow/liveness analysis) and `Fresh` scalars carry no
//! reclamation payload.

use crate::ir::{IrExpr, IrExprKind, IrStrPart};

/// Ownership/aliasing fact for a typed IR expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ownership {
    /// Produces a new allocation (or trivially owned scalar) not aliased yet.
    Fresh,
    /// Borrowed from elsewhere (a string literal, a `Str` value).
    Borrowed,
    /// Aliased with a binding, container slot, or owner.
    Aliased,
    /// May be retained, returned, stored, or otherwise escape; or unknown.
    MayEscape,
}

/// Why a reclamation site was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReclamationRejected {
    pub ownership: Ownership,
}

impl std::fmt::Display for ReclamationRejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "reclamation requires a Fresh (or proven non-escaping local) value, found {:?}",
            self.ownership
        )
    }
}

impl std::error::Error for ReclamationRejected {}

/// Classify a typed expression. Unknown constructs become `MayEscape`.
pub fn ownership(expr: &IrExpr) -> Ownership {
    match &expr.kind {
        // Trivially owned scalars; nothing to reclaim.
        IrExprKind::Int(_) | IrExprKind::Float(_) | IrExprKind::Bool(_) => Ownership::Fresh,

        // Strings: a literal-only string is static/borrowed. An interpolated
        // string allocates a fresh buffer in codegen, but its ownership is not
        // proven at this site, so be conservative. Never `Fresh` for `Str`,
        // matching T-121 (borrowed `Str` is never freed).
        IrExprKind::Str(parts) => {
            if parts.iter().all(|p| matches!(p, IrStrPart::Text(_))) {
                Ownership::Borrowed
            } else {
                Ownership::MayEscape
            }
        }

        // Reads from a binding / owner are aliased.
        IrExprKind::Var(_) | IrExprKind::Global(_) => Ownership::Aliased,
        IrExprKind::Field { .. } | IrExprKind::Index { .. } | IrExprKind::StrIndex { .. } => {
            Ownership::Aliased
        }

        // Freshly constructed aggregates are not aliased *at construction*.
        // They may still escape; Stage 1 does not prove last-use.
        IrExprKind::List(_)
        | IrExprKind::Map(_)
        | IrExprKind::Construct { .. }
        | IrExprKind::ConstructVariant { .. } => Ownership::Fresh,

        // Calls, builtins, operators, `match`, and anything else: conservative.
        // Notably `str(Str)` returns its argument and `trim` may return its
        // input unchanged; `vantyx_text_trim` conditional ownership is not
        // `Fresh`. Unknown -> `MayEscape`.
        _ => Ownership::MayEscape,
    }
}

/// Stage-1 verifier. A reclamation site is allowed only if the value is
/// `Fresh`. (A separate proven-non-escaping-local path is reserved for
/// Stage 2/3 and is intentionally not claimed here.) Stage 1 does not call
/// this from codegen; nothing is reclaimed by this check.
pub fn verify_reclamation(expr: &IrExpr) -> Result<(), ReclamationRejected> {
    match ownership(expr) {
        Ownership::Fresh => Ok(()),
        ownership => Err(ReclamationRejected { ownership }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::BinOp;
    use crate::ir::IrType;
    use crate::span::Span;

    fn expr(kind: IrExprKind) -> IrExpr {
        IrExpr {
            kind,
            ty: IrType::Int,
            span: Span::new(0, 1),
        }
    }

    fn str_expr(parts: Vec<IrStrPart>) -> IrExpr {
        IrExpr {
            kind: IrExprKind::Str(parts),
            ty: IrType::Str,
            span: Span::new(0, 1),
        }
    }

    #[test]
    fn scalars_are_fresh() {
        assert_eq!(ownership(&expr(IrExprKind::Int(1))), Ownership::Fresh);
        assert_eq!(ownership(&expr(IrExprKind::Float(1.5))), Ownership::Fresh);
        assert_eq!(ownership(&expr(IrExprKind::Bool(true))), Ownership::Fresh);
    }

    #[test]
    fn string_literal_is_borrowed_interpolated_is_not_fresh() {
        assert_eq!(
            ownership(&str_expr(vec![IrStrPart::Text("x".into())])),
            Ownership::Borrowed
        );
        assert_eq!(ownership(&str_expr(vec![])), Ownership::Borrowed);
        let interp = str_expr(vec![
            IrStrPart::Text("a".into()),
            IrStrPart::Value(Box::new(expr(IrExprKind::Int(1)))),
        ]);
        assert_eq!(ownership(&interp), Ownership::MayEscape);
        assert_ne!(ownership(&interp), Ownership::Fresh);
    }

    #[test]
    fn bindings_and_projections_are_aliased() {
        assert_eq!(
            ownership(&expr(IrExprKind::Var("x".into()))),
            Ownership::Aliased
        );
        assert_eq!(
            ownership(&expr(IrExprKind::Global("g".into()))),
            Ownership::Aliased
        );
        assert_eq!(
            ownership(&expr(IrExprKind::Field {
                base: Box::new(expr(IrExprKind::Var("x".into()))),
                name: "f".into()
            })),
            Ownership::Aliased
        );
        assert_eq!(
            ownership(&expr(IrExprKind::Index {
                base: Box::new(expr(IrExprKind::Var("x".into()))),
                index: Box::new(expr(IrExprKind::Int(0)))
            })),
            Ownership::Aliased
        );
        assert_eq!(
            ownership(&expr(IrExprKind::StrIndex {
                base: Box::new(expr(IrExprKind::Var("x".into()))),
                index: Box::new(expr(IrExprKind::Int(0)))
            })),
            Ownership::Aliased
        );
    }

    #[test]
    fn constructions_are_fresh_not_aliased_at_construction() {
        assert_eq!(ownership(&expr(IrExprKind::List(vec![]))), Ownership::Fresh);
        assert_eq!(ownership(&expr(IrExprKind::Map(vec![]))), Ownership::Fresh);
        assert_eq!(
            ownership(&expr(IrExprKind::Construct {
                name: "P".into(),
                args: vec![],
                eval_order: vec![]
            })),
            Ownership::Fresh
        );
        assert_eq!(
            ownership(&expr(IrExprKind::ConstructVariant {
                enum_name: "E".into(),
                variant: "V".into(),
                tag: 0,
                args: vec![]
            })),
            Ownership::Fresh
        );
    }

    #[test]
    fn calls_builtins_operators_match_are_may_escape() {
        assert_eq!(
            ownership(&expr(IrExprKind::Call {
                name: "f".into(),
                args: vec![]
            })),
            Ownership::MayEscape
        );
        // str(Str) returns its argument; trim may return input unchanged: not Fresh.
        assert_eq!(
            ownership(&expr(IrExprKind::Builtin {
                name: "str".into(),
                args: vec![]
            })),
            Ownership::MayEscape
        );
        assert_eq!(
            ownership(&expr(IrExprKind::Builtin {
                name: "trim".into(),
                args: vec![]
            })),
            Ownership::MayEscape
        );
        assert_eq!(
            ownership(&expr(IrExprKind::Binary {
                op: BinOp::Add,
                lhs: Box::new(expr(IrExprKind::Int(1))),
                rhs: Box::new(expr(IrExprKind::Int(2)))
            })),
            Ownership::MayEscape
        );
    }

    #[test]
    fn verifier_accepts_fresh_rejects_otherwise() {
        assert!(verify_reclamation(&expr(IrExprKind::Int(1))).is_ok());
        assert!(verify_reclamation(&expr(IrExprKind::List(vec![]))).is_ok());
        assert_eq!(
            verify_reclamation(&expr(IrExprKind::Var("x".into()))),
            Err(ReclamationRejected {
                ownership: Ownership::Aliased
            })
        );
        assert_eq!(
            verify_reclamation(&str_expr(vec![IrStrPart::Text("x".into())])),
            Err(ReclamationRejected {
                ownership: Ownership::Borrowed
            })
        );
        assert_eq!(
            verify_reclamation(&expr(IrExprKind::Call {
                name: "f".into(),
                args: vec![]
            })),
            Err(ReclamationRejected {
                ownership: Ownership::MayEscape
            })
        );
    }
}
