//! The part of the checker that works on patterns.

use crate::ast::*;
use crate::types::core::{
    is_empty, object_rest_type, project_field, remove_type, string_literal, type_from_kind, union,
    without_empty, Type,
};
use crate::VeyError;

use super::checker::TypeChecker;

fn destructured_element_type(ty: &Type) -> Type {
    match ty {
        Type::Array(element) => union(vec![(**element).clone(), Type::Undefined]),
        Type::String => union(vec![Type::String, Type::Undefined]),
        Type::Any => Type::Any,
        Type::Unknown => Type::Unknown,
        Type::Union(items) => union(items.iter().map(destructured_element_type).collect()),
        _ => Type::Unknown,
    }
}

fn without_undefined(ty: &Type) -> Type {
    match ty {
        Type::Undefined => Type::Never,
        Type::Union(items) => union(
            items
                .iter()
                .map(without_undefined)
                .filter(|item| !matches!(item, Type::Never))
                .collect(),
        ),
        other => other.clone(),
    }
}

impl TypeChecker<'_> {
    /// The path of a value that a check may narrow, and its type.
    ///
    /// A check narrows a name, such as `value`, or a member of it, such as
    /// `user.name`. A member is known by its path, which is not a name of
    /// the program.
    pub(crate) fn place(&self, expr: &Expr) -> Option<(String, Type)> {
        match expr {
            Expr::Ident(name, _) => self.lookup(name).map(|ty| (name.clone(), ty)),
            Expr::Member(object, MemberProp::Name(name), _, pos) => {
                let (base, base_ty) = self.place(object)?;
                let path = format!("{}.{}", base, name);
                let ty = match self.lookup_member_path(&path) {
                    Some(ty) => ty,
                    None => self.lookup_member(&base_ty, name, *pos).ok()?,
                };
                Some((path, ty))
            }
            _ => None,
        }
    }

    pub(crate) fn narrow_condition(&self, expr: &Expr) -> Option<(String, Type, Type)> {
        // A check of the type: `if value is string { }`.
        if let Expr::Is(inner, ty, _) = expr {
            let (name, original) = self.place(inner)?;
            let wanted = self.resolve_type(ty).ok()?;
            // In the branch the value is of the type that was asked for, and
            // outside it is the rest.
            let else_ty = remove_type(&original, &wanted);
            return Some((name, wanted, else_ty));
        }
        // The truth of a value: `if user { }`. A value that may be nothing
        // is asked with its truth, and the branch knows that it is there.
        if let Some((name, original)) = self.place(expr) {
            let then_ty = without_empty(&original);
            if then_ty == original {
                return None;
            }
            let else_ty = union(vec![Type::Null, Type::Undefined]);
            return Some((name, then_ty, else_ty));
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
        let (name, original) = self.place(left)?;
        let Expr::Ident(type_name, _) = right else {
            return None;
        };
        let target = self.type_by_name(type_name)?;
        let then_ty = union(vec![target.clone(), original.clone()]);
        let else_ty = remove_type(&original, &target);
        Some((name, then_ty, else_ty))
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
        if let Some((name, original)) = self.place(left) {
            if is_empty(right) {
                let else_ty = without_empty(&original);
                return Some((name, Type::Null, else_ty));
            }
        }

        if let Some((name, original)) = self.place(right) {
            if is_empty(left) {
                let else_ty = without_empty(&original);
                return Some((name, Type::Null, else_ty));
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
        self.place(&args[0])
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
            Pattern::Wildcard { .. } => {}
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
        self.check_pattern_defaults_against(pattern, &Type::Any)
    }

    /// The value that a name has by default is checked against the type of
    /// the name.
    pub(crate) fn check_pattern_defaults_against(
        &mut self,
        pattern: &BindingPattern,
        ty: &Type,
    ) -> Result<(), VeyError> {
        match pattern {
            BindingPattern::Name { default, .. } => {
                if let Some(default) = default {
                    let actual = self.infer_expr(default)?;
                    let wanted = without_undefined(ty);
                    if wanted != Type::Never
                        && !matches!(actual, Type::Any)
                        && !self.fits(&actual, &wanted)
                    {
                        return Err(self.type_error(default.pos(), &actual, &wanted));
                    }
                }
                Ok(())
            }
            BindingPattern::Array { items, .. } => {
                for (index, item) in items.iter().enumerate() {
                    let element = match ty {
                        Type::Tuple(parts) => parts.get(index).cloned().unwrap_or(Type::Undefined),
                        other => destructured_element_type(other),
                    };
                    self.check_pattern_defaults_against(item, &element)?;
                }
                Ok(())
            }
            BindingPattern::Object { fields, .. } => {
                for field in fields {
                    let field_ty = project_field(ty, &field.key);
                    self.check_pattern_defaults_against(&field.pattern, &field_ty)?;
                }
                Ok(())
            }
        }
    }

    pub(crate) fn declare_pattern_typed(&mut self, pattern: &BindingPattern, ty: &Type) {
        match pattern {
            BindingPattern::Name { name, default, .. } => {
                let found = if default.is_some() {
                    let found = without_undefined(ty);
                    if found == Type::Never {
                        Type::Unknown
                    } else {
                        found
                    }
                } else {
                    ty.clone()
                };
                self.set_binding(name, found);
            }
            BindingPattern::Array { items, rest, .. } => {
                // A tuple gives every name the type of its own place.
                let item_ty = match ty {
                    Type::Tuple(parts) => {
                        for (index, item) in items.iter().enumerate() {
                            let part = parts.get(index).cloned().unwrap_or(Type::Undefined);
                            self.declare_pattern_typed(item, &part);
                        }
                        if let Some(rest) = rest {
                            let remainder = parts.iter().skip(items.len()).cloned().collect();
                            self.set_binding(rest, Type::Tuple(remainder));
                        }
                        return;
                    }
                    other => destructured_element_type(other),
                };
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
