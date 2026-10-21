//! The part of the checker that works on patterns.

use crate::ast::*;
use crate::types::core::{
    element_type, object_rest_type, project_field, remove_type, string_literal, type_from_kind,
    Type,
};
use crate::VeyError;

use super::checker::TypeChecker;

impl TypeChecker<'_> {
    pub(crate) fn narrow_condition(&self, expr: &Expr) -> Option<(String, Type, Type)> {
        let Expr::Binary(op, left, right, _) = expr else {
            return None;
        };
        if !matches!(op.as_str(), "==" | "===" | "!=" | "!==") {
            return None;
        }
        let (name, then_ty, else_ty) = self.narrow_comparison(left, right)?;
        if op == "!=" || op == "!==" {
            Some((name, else_ty, then_ty))
        } else {
            Some((name, then_ty, else_ty))
        }
    }

    pub(crate) fn narrow_comparison(
        &self,
        left: &Expr,
        right: &Expr,
    ) -> Option<(String, Type, Type)> {
        if let Some((name, original)) = self.type_call(left) {
            if let Some(kind) = string_literal(right) {
                let target = type_from_kind(kind, &original)?;
                let else_ty = remove_type(&original, &target);
                return Some((name, target, else_ty));
            }
        }

        if let Some((name, original)) = self.type_call(right) {
            if let Some(kind) = string_literal(left) {
                let target = type_from_kind(kind, &original)?;
                let else_ty = remove_type(&original, &target);
                return Some((name, target, else_ty));
            }
        }

        if let Expr::Ident(name, _) = left {
            if matches!(right, Expr::Null(_)) {
                let original = self.lookup(name)?;
                let else_ty = original.without_null();
                return Some((name.clone(), Type::Null, else_ty));
            }
        }

        if let Expr::Ident(name, _) = right {
            if matches!(left, Expr::Null(_)) {
                let original = self.lookup(name)?;
                let else_ty = original.without_null();
                return Some((name.clone(), Type::Null, else_ty));
            }
        }

        None
    }

    pub(crate) fn type_call(&self, expr: &Expr) -> Option<(String, Type)> {
        let Expr::Call(callee, args, _, _) = expr else {
            return None;
        };
        let Expr::Ident(name, _) = &**callee else {
            return None;
        };
        if name != "type" || args.len() != 1 {
            return None;
        }
        let Expr::Ident(variable, _) = &args[0] else {
            return None;
        };
        self.lookup(variable).map(|ty| (variable.clone(), ty))
    }

    pub(crate) fn check_pattern(&mut self, pattern: &Pattern) -> Result<(), VeyError> {
        match pattern {
            Pattern::Literal(expr) => {
                self.infer_expr(expr)?;
            }
            Pattern::Range(start, end) => {
                self.infer_expr(start)?;
                self.infer_expr(end)?;
            }
            Pattern::Wildcard => {}
            Pattern::Binding { .. } => {}
            Pattern::Enum { .. } => {}
        }
        Ok(())
    }

    /// Look at the values that are used when a pattern has none.
    pub(crate) fn check_pattern_defaults(
        &mut self,
        pattern: &BindingPattern,
    ) -> Result<(), VeyError> {
        match pattern {
            BindingPattern::Name { default, .. } => {
                if let Some(default) = default {
                    self.infer_expr(default)?;
                }
                Ok(())
            }
            BindingPattern::Array { items, .. } => {
                for item in items {
                    self.check_pattern_defaults(item)?;
                }
                Ok(())
            }
            BindingPattern::Object { fields, .. } => {
                for field in fields {
                    self.check_pattern_defaults(&field.pattern)?;
                }
                Ok(())
            }
        }
    }

    pub(crate) fn declare_pattern_typed(&mut self, pattern: &BindingPattern, ty: &Type) {
        match pattern {
            BindingPattern::Name { name, .. } => self.set_binding(name, ty.clone()),
            BindingPattern::Array { items, rest, .. } => {
                let item_ty = element_type(ty);
                for item in items {
                    self.declare_pattern_typed(item, &item_ty);
                }
                if let Some(rest) = rest {
                    self.set_binding(rest, Type::Array(Box::new(item_ty)));
                }
            }
            BindingPattern::Object { fields, rest, .. } => {
                for field in fields {
                    let field_ty = project_field(ty, &field.key);
                    self.declare_pattern_typed(&field.pattern, &field_ty);
                }
                if let Some(rest) = rest {
                    self.set_binding(rest, object_rest_type(ty, fields));
                }
            }
        }
    }
}
