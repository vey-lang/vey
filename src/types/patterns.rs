//! The part of the checker that works on patterns.

use crate::ast::*;
use crate::types::core::{element_type, object_rest_type, project_field, Type};
use crate::VeyError;

use super::checker::TypeChecker;

impl TypeChecker<'_> {
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
            Pattern::Enum { .. } => {}
        }
        Ok(())
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
