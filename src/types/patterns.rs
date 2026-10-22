//! The part of the checker that works on patterns.

use crate::ast::*;
use crate::types::core::{
    element_type, is_empty, object_rest_type, project_field, remove_type, string_literal,
    type_from_kind, union, without_empty, Type,
};
use crate::VeyError;

use super::checker::TypeChecker;

impl TypeChecker<'_> {
    pub(crate) fn narrow_condition(&self, expr: &Expr) -> Option<(String, Type, Type)> {
        // The truth of a value: `if user { }`. A value that may be nothing
        // is asked with its truth, and the branch knows that it is there.
        if let Expr::Ident(name, _) = expr {
            let original = self.lookup(name)?;
            let then_ty = without_empty(&original);
            if then_ty == original {
                return None;
            }
            let else_ty = union(vec![Type::Null, Type::Undefined]);
            return Some((name.clone(), then_ty, else_ty));
        }

        let Expr::Binary(op, left, right, _) = expr else {
            return None;
        };
        if op == "instanceof" {
            return self.narrow_instanceof(left, right);
        }
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

    /// The narrowing of a check with `instanceof`.
    ///
    /// The value is of the type on the right, when that is a type of the
    /// program.
    fn narrow_instanceof(&self, left: &Expr, right: &Expr) -> Option<(String, Type, Type)> {
        let Expr::Ident(name, _) = left else {
            return None;
        };
        let Expr::Ident(type_name, _) = right else {
            return None;
        };
        let original = self.lookup(name)?;
        let target = self.type_by_name(type_name)?;
        let then_ty = union(vec![target.clone(), original.clone()]);
        let else_ty = remove_type(&original, &target);
        Some((name.clone(), then_ty, else_ty))
    }

    /// The type of the program with this name, when it is one.
    pub(crate) fn type_by_name(&self, name: &str) -> Option<Type> {
        if self.interfaces.contains_key(name) {
            return self.interface_type(name).ok();
        }
        if let Some(ty) = self.alias_types.get(name) {
            return Some(ty.clone());
        }
        None
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

        // A value that is compared with nothing is narrowed to nothing, and
        // the other way around.
        if let Expr::Ident(name, _) = left {
            if is_empty(right) {
                let original = self.lookup(name)?;
                let else_ty = without_empty(&original);
                return Some((name.clone(), Type::Null, else_ty));
            }
        }

        if let Expr::Ident(name, _) = right {
            if is_empty(left) {
                let original = self.lookup(name)?;
                let else_ty = without_empty(&original);
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
            Pattern::List(items, _) => {
                for item in items {
                    self.check_pattern(item)?;
                }
            }
            Pattern::Object(fields, _) => {
                for (_, item) in fields {
                    self.check_pattern(item)?;
                }
            }
            Pattern::Prefix(expr, _) => {
                self.infer_expr(expr)?;
            }
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
