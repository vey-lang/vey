//! The part of the checker that works on expressions.

use std::collections::HashMap;

use crate::ast::*;
use crate::lexer::Pos;
use crate::types::builtins::builtin_type;
use crate::types::core::{
    assignable, collect_type_vars, erase_type_vars, substitute, substitute_bound, unify_types,
    union, FunctionType, ObjectField, Type,
};
use crate::VeyError;

use super::checker::TypeChecker;
use super::helpers::{argument_label, set_object_field, type_argument_label};

impl TypeChecker<'_> {
    pub(crate) fn infer_expr(&mut self, expr: &Expr) -> Result<Type, VeyError> {
        match expr {
            Expr::Number(_, _) => Ok(Type::Number),
            Expr::String(parts, _) => {
                for part in parts {
                    if let Some(expr) = &part.expr {
                        self.infer_expr(expr)?;
                    }
                }
                Ok(Type::String)
            }
            Expr::Bool(_, _) => Ok(Type::Bool),
            Expr::Null(_) => Ok(Type::Null),
            Expr::Ident(name, _) => Ok(self
                .lookup(name)
                .or_else(|| builtin_type(name))
                .unwrap_or(Type::Any)),
            Expr::Array(items, _) => {
                let mut types = Vec::new();
                for item in items {
                    match item {
                        Expr::Spread(inner, _) => {
                            let ty = self.infer_expr(inner)?;
                            match ty {
                                Type::Array(element) => types.push(*element),
                                _ => types.push(Type::Any),
                            }
                        }
                        other => types.push(self.infer_expr(other)?),
                    }
                }
                if types.is_empty() {
                    Ok(Type::Array(Box::new(Type::Any)))
                } else {
                    Ok(Type::Array(Box::new(union(types))))
                }
            }
            Expr::Object(props, _) => {
                let mut out = Vec::new();
                let mut unknown = false;
                for prop in props {
                    match prop {
                        ObjectProp::Field(name, value) => {
                            let ty = self.infer_expr(value)?;
                            set_object_field(
                                &mut out,
                                ObjectField {
                                    name: name.clone(),
                                    ty,
                                    optional: false,
                                },
                            );
                        }
                        ObjectProp::Spread(value) => {
                            let ty = self.infer_expr(value)?;
                            match ty {
                                Type::Object(fields) | Type::Named(_, fields) => {
                                    for field in fields {
                                        set_object_field(&mut out, field);
                                    }
                                }
                                _ => unknown = true,
                            }
                        }
                    }
                }
                if unknown {
                    Ok(Type::Any)
                } else {
                    Ok(Type::Object(out))
                }
            }
            Expr::Unary(op, inner, pos) => {
                let ty = self.infer_expr(inner)?;
                match op.as_str() {
                    "!" | "not" => Ok(Type::Bool),
                    "-" | "+" => {
                        if matches!(ty, Type::Any) || assignable(&ty, &Type::Number) {
                            Ok(Type::Number)
                        } else {
                            Err(self.error(
                                *pos,
                                format!(
                                    "operator '{}' requires a number, found {}",
                                    op,
                                    ty.display()
                                ),
                            ))
                        }
                    }
                    _ => Ok(Type::Any),
                }
            }
            Expr::Binary(op, left, right, pos) => {
                let left = self.infer_expr(left)?;
                let right = self.infer_expr(right)?;
                self.binary_result(op, &left, &right, *pos)
            }
            Expr::Logical(_, left, right, _) => {
                let left = self.infer_expr(left)?;
                let right = self.infer_expr(right)?;
                if matches!(left, Type::Any) || matches!(right, Type::Any) {
                    Ok(Type::Any)
                } else if assignable(&left, &Type::Bool) && assignable(&right, &Type::Bool) {
                    Ok(Type::Bool)
                } else {
                    Ok(Type::Any)
                }
            }
            Expr::Nullish(left, right, _) => {
                let left = self.infer_expr(left)?;
                let right = self.infer_expr(right)?;
                Ok(union(vec![left.without_null(), right]))
            }
            Expr::Ternary(test, then_value, else_value, _) => {
                self.infer_expr(test)?;
                let then_ty = self.infer_expr(then_value)?;
                let else_ty = self.infer_expr(else_value)?;
                Ok(union(vec![then_ty, else_ty]))
            }
            Expr::Call(callee, args, _, pos) => {
                let callee_ty = self.infer_expr(callee)?;
                let (arg_types, has_spread) = self.call_argument_types(args)?;
                self.check_call(&callee_ty, &arg_types, has_spread, *pos, None, None)
            }
            Expr::CallGeneric(callee, type_args, args, _, pos) => {
                let callee_ty = self.infer_expr(callee)?;
                let (arg_types, has_spread) = self.call_argument_types(args)?;
                let mut explicit = Vec::new();
                for arg in type_args {
                    explicit.push(self.resolve_type(arg)?);
                }
                self.check_call(
                    &callee_ty,
                    &arg_types,
                    has_spread,
                    *pos,
                    Some(&explicit),
                    None,
                )
            }
            Expr::Member(object, prop, optional, pos) => {
                self.member_type(object, prop, *optional, *pos)
            }
            Expr::Fn(params, body, ret_ann, pos) => {
                let ret = self.check_function(params, body, ret_ann.as_ref(), *pos)?;
                let checked = ret_ann.is_some() || params.iter().any(|param| param.ty.is_some());
                if checked {
                    self.function_type_from_parts(params, ret, ret_ann.is_some(), &[])
                } else {
                    self.function_type_from_parts(params, Type::Any, false, &[])
                }
            }
            Expr::Arrow(params, body, pos) => {
                let ret = self.check_arrow(params, body, *pos)?;
                let checked = params.iter().any(|param| param.ty.is_some());
                if checked {
                    self.function_type_from_parts(params, ret, false, &[])
                } else {
                    self.function_type_from_parts(params, Type::Any, false, &[])
                }
            }
            Expr::Range(start, end, pos) => {
                let start_ty = self.infer_expr(start)?;
                let end_ty = self.infer_expr(end)?;
                if !matches!(start_ty, Type::Any) && !assignable(&start_ty, &Type::Number) {
                    return Err(self.type_error(*pos, &start_ty, &Type::Number));
                }
                if !matches!(end_ty, Type::Any) && !assignable(&end_ty, &Type::Number) {
                    return Err(self.type_error(*pos, &end_ty, &Type::Number));
                }
                Ok(Type::Array(Box::new(Type::Number)))
            }
            Expr::Match(subject, arms, _) => {
                let subject_ty = self.infer_expr(subject)?;
                let mut types = Vec::new();
                let mut covered = std::collections::HashSet::new();
                let mut has_wildcard = false;
                for arm in arms {
                    self.push_scope();
                    for pattern in &arm.patterns {
                        match pattern {
                            Pattern::Enum {
                                enum_name,
                                variant,
                                bindings,
                                pos,
                            } => {
                                // resolve enum
                                if !self.enums.contains_key(enum_name) {
                                    return Err(
                                        self.error(*pos, format!("unknown enum '{}'", enum_name))
                                    );
                                }
                                let v_fields: Vec<(String, Option<TypeExpr>)> = {
                                    let Some(info) = self.enums.get(enum_name) else {
                                        return Err(self
                                            .error(*pos, format!("unknown enum '{}'", enum_name)));
                                    };
                                    let v = info
                                        .variants
                                        .iter()
                                        .find(|v| v.name == *variant)
                                        .ok_or_else(|| {
                                            self.error(
                                                *pos,
                                                format!(
                                                    "enum '{}' has no variant '{}'",
                                                    enum_name, variant
                                                ),
                                            )
                                        })?;
                                    v.fields
                                        .iter()
                                        .map(|f| (f.name.clone(), f.ty.clone()))
                                        .collect()
                                };
                                // subject compatibility
                                match &subject_ty {
                                    Type::EnumUnion(e) if e != enum_name => {
                                        return Err(self.error(
                                            *pos,
                                            format!(
                                                "pattern '{}.{}' does not match '{}'",
                                                enum_name, variant, e
                                            ),
                                        ));
                                    }
                                    Type::Enum(e, _) if e != enum_name => {
                                        return Err(self.error(
                                            *pos,
                                            format!(
                                                "pattern '{}.{}' does not match '{}'",
                                                enum_name, variant, e
                                            ),
                                        ));
                                    }
                                    _ => {}
                                }
                                // binding count check
                                if bindings.len() != v_fields.len() {
                                    return Err(self.error(
                                        *pos,
                                        format!(
                                            "variant '{}.{}' expects {} binding(s), found {}",
                                            enum_name,
                                            variant,
                                            v_fields.len(),
                                            bindings.len()
                                        ),
                                    ));
                                }
                                // resolve field types and declare bindings
                                // positional vs named
                                let mut positional_idx = 0;
                                for b in bindings {
                                    let field_name = if let Some(f) = &b.field {
                                        f.clone()
                                    } else {
                                        if positional_idx >= v_fields.len() {
                                            return Err(self.error(b.pos, format!("variant '{}.{}' expects {} binding(s), found {}", enum_name, variant, v_fields.len(), bindings.len())));
                                        }
                                        let name = v_fields[positional_idx].0.clone();
                                        positional_idx += 1;
                                        name
                                    };
                                    let field_ty = v_fields
                                        .iter()
                                        .find(|(n, _)| n == &field_name)
                                        .map(|(_, t)| t.clone())
                                        .ok_or_else(|| {
                                            self.error(
                                                b.pos,
                                                format!(
                                                    "variant '{}.{}' has no field '{}'",
                                                    enum_name, variant, field_name
                                                ),
                                            )
                                        })?;
                                    let ty = match &field_ty {
                                        Some(ty) => self.resolve_type(ty)?,
                                        None => Type::Any,
                                    };
                                    self.declare(&b.name, ty);
                                }
                                covered.insert(format!("{}.{}", enum_name, variant));
                            }
                            Pattern::Wildcard => {
                                has_wildcard = true;
                            }
                            _ => {
                                self.check_pattern(pattern)?;
                            }
                        }
                    }
                    types.push(self.infer_expr(&arm.body)?);
                    self.pop_scope();
                }
                // exhaustiveness
                if !has_wildcard {
                    match &subject_ty {
                        Type::EnumUnion(name) => {
                            if let Some(info) = self.enums.get(name) {
                                let missing: Vec<String> = info
                                    .variants
                                    .iter()
                                    .filter(|v| {
                                        !covered
                                            .iter()
                                            .any(|c| c == &format!("{}.{}", name, v.name))
                                    })
                                    .map(|v| v.name.clone())
                                    .collect();
                                if !missing.is_empty() {
                                    return Err(self.error(
                                        subject.pos(),
                                        format!(
                                            "match on '{}' is not exhaustive, missing: {}",
                                            name,
                                            missing.join(", ")
                                        ),
                                    ));
                                }
                            }
                        }
                        Type::Enum(name, v) if !covered.contains(&format!("{}.{}", name, v)) => {
                            return Err(self.error(
                                subject.pos(),
                                format!(
                                    "match on '{}.{}' is not exhaustive, missing: {}",
                                    name, v, v
                                ),
                            ));
                        }
                        _ => {}
                    }
                }
                if types.is_empty() {
                    Ok(Type::Void)
                } else {
                    Ok(union(types))
                }
            }
            Expr::Await(inner, _) => {
                self.infer_expr(inner)?;
                Ok(Type::Any)
            }
            Expr::Spread(inner, _) => self.infer_expr(inner),
        }
    }

    /// The types of the arguments of a call.
    pub(crate) fn call_argument_types(
        &mut self,
        args: &[Expr],
    ) -> Result<(Vec<Type>, bool), VeyError> {
        let mut types = Vec::new();
        let mut has_spread = false;
        for arg in args {
            match arg {
                Expr::Spread(inner, _) => {
                    has_spread = true;
                    types.push(self.infer_expr(inner)?);
                }
                other => types.push(self.infer_expr(other)?),
            }
        }
        Ok((types, has_spread))
    }

    /// Infer the type of an expression that is used as a certain type.
    ///
    /// The expected type settles the type parameters of a generic call that
    /// the arguments do not determine.
    pub(crate) fn infer_expr_as(
        &mut self,
        expr: &Expr,
        expected: Option<&Type>,
    ) -> Result<Type, VeyError> {
        match expr {
            Expr::Call(callee, args, _, pos) => {
                let callee_ty = self.infer_expr(callee)?;
                let (arg_types, has_spread) = self.call_argument_types(args)?;
                self.check_call(&callee_ty, &arg_types, has_spread, *pos, None, expected)
            }
            other => self.infer_expr(other),
        }
    }

    pub(crate) fn check_call(
        &self,
        callee: &Type,
        args: &[Type],
        has_spread: bool,
        pos: Pos,
        type_args: Option<&[Type]>,
        expected: Option<&Type>,
    ) -> Result<Type, VeyError> {
        match callee {
            Type::Any => Ok(Type::Any),
            Type::Function(function) => {
                if has_spread || !function.checked {
                    return Ok(substitute(&function.ret, &HashMap::new()));
                }

                if function.rest {
                    if args.len() < function.required {
                        return Err(self.error(
                            pos,
                            format!(
                                "expected at least {} {}, got {}",
                                function.required,
                                argument_label(function.required),
                                args.len()
                            ),
                        ));
                    }
                } else {
                    if args.len() < function.required {
                        return Err(self.error(
                            pos,
                            format!(
                                "expected at least {} {}, got {}",
                                function.required,
                                argument_label(function.required),
                                args.len()
                            ),
                        ));
                    }
                    if args.len() > function.params.len() {
                        return Err(self.error(
                            pos,
                            format!(
                                "expected at most {} {}, got {}",
                                function.params.len(),
                                argument_label(function.params.len()),
                                args.len()
                            ),
                        ));
                    }
                }

                let mut subst = HashMap::new();
                // The type arguments of the call fill the type parameters of
                // the function, so they do not have to be inferred.
                if let Some(explicit) = type_args {
                    let mut free = Vec::new();
                    for param in &function.params {
                        collect_type_vars(param, &mut free);
                    }
                    collect_type_vars(&function.ret, &mut free);

                    let names: Vec<String> = if function.type_params.is_empty() {
                        free.clone()
                    } else {
                        function.type_params.clone()
                    };
                    if explicit.len() != names.len() {
                        return Err(self.error(
                            pos,
                            format!(
                                "expected {} type {}, got {}",
                                names.len(),
                                type_argument_label(names.len()),
                                explicit.len()
                            ),
                        ));
                    }
                    for (name, ty) in names.iter().zip(explicit) {
                        // A parameter that was settled already keeps its type.
                        if free.iter().any(|item| item == name) {
                            subst.insert(name.clone(), ty.clone());
                        }
                    }
                }
                for (index, arg) in args.iter().enumerate() {
                    // A rest parameter stands for the type of a single extra
                    // argument, so the last entry is used for all of them.
                    let param = if index < function.params.len() {
                        &function.params[index]
                    } else {
                        function.params.last().unwrap_or(&Type::Any)
                    };
                    // A type parameter that is already bound stands for the
                    // type it was bound to.
                    let expected = match param {
                        Type::Var(name) => {
                            subst.get(name).cloned().unwrap_or_else(|| param.clone())
                        }
                        other => other.clone(),
                    };
                    let mut trial = subst.clone();
                    if unify_types(arg, &expected, &mut trial) {
                        subst = trial;
                    } else if !assignable(arg, &expected) {
                        return Err(self.error(
                            pos,
                            format!(
                                "argument {} expects {}, found {}",
                                index + 1,
                                expected.display(),
                                arg.display()
                            ),
                        ));
                    }
                }

                // The type the call is used as may settle a type parameter
                // that the arguments do not determine.
                if let Some(expected) = expected {
                    let mut trial = subst.clone();
                    if unify_types(expected, &function.ret, &mut trial) {
                        subst = trial;
                    }
                }

                // A type parameter has to satisfy the constraint of the
                // function it was inferred for.
                for (name, constraint) in &function.constraints {
                    let Some(actual) = subst.get(name) else {
                        continue;
                    };
                    let constraint = substitute_bound(constraint, &subst);
                    if !assignable(actual, &constraint) {
                        return Err(self.error(
                            pos,
                            format!(
                                "type argument '{}' expects {}, found {}",
                                name,
                                constraint.display(),
                                actual.display()
                            ),
                        ));
                    }
                }

                // A type parameter that could not be inferred stays unknown.
                Ok(erase_type_vars(&substitute(&function.ret, &subst)))
            }
            Type::Union(_) => Ok(Type::Any),
            other => Err(self.error(
                pos,
                format!("value of type {} is not callable", other.display()),
            )),
        }
    }

    pub(crate) fn binary_result(
        &self,
        op: &str,
        left: &Type,
        right: &Type,
        pos: Pos,
    ) -> Result<Type, VeyError> {
        match op {
            "+" => {
                if matches!(left, Type::Any) || matches!(right, Type::Any) {
                    return Ok(Type::Any);
                }
                if assignable(left, &Type::Number) && assignable(right, &Type::Number) {
                    Ok(Type::Number)
                } else if assignable(left, &Type::String) || assignable(right, &Type::String) {
                    Ok(Type::String)
                } else {
                    Ok(Type::Any)
                }
            }
            "-" | "*" | "/" | "%" | "**" => {
                if matches!(left, Type::Any) || matches!(right, Type::Any) {
                    return Ok(Type::Any);
                }
                if assignable(left, &Type::Number) && assignable(right, &Type::Number) {
                    Ok(Type::Number)
                } else {
                    Err(self.error(
                        pos,
                        format!(
                            "operator '{}' requires numbers, found {} and {}",
                            op,
                            left.display(),
                            right.display()
                        ),
                    ))
                }
            }
            "==" | "!=" | "===" | "!==" | "<" | "<=" | ">" | ">=" => Ok(Type::Bool),
            "&" | "|" | "^" | "<<" | ">>" => {
                if matches!(left, Type::Any) || matches!(right, Type::Any) {
                    Ok(Type::Any)
                } else if assignable(left, &Type::Number) && assignable(right, &Type::Number) {
                    Ok(Type::Number)
                } else {
                    Ok(Type::Any)
                }
            }
            _ => Ok(Type::Any),
        }
    }

    pub(crate) fn member_type(
        &mut self,
        object: &Expr,
        prop: &MemberProp,
        optional: bool,
        pos: Pos,
    ) -> Result<Type, VeyError> {
        // A member of another module is looked up in the environment of that
        // module, so that its signature is checked.
        if let (MemberProp::Name(name), Expr::Ident(alias, _)) = (prop, object) {
            if let Some(env) = self.imports.get(alias) {
                let ty = env
                    .values
                    .get(name)
                    .cloned()
                    .or_else(|| env.types.get(name).cloned())
                    .unwrap_or(Type::Any);
                return Ok(if optional {
                    union(vec![ty, Type::Null])
                } else {
                    ty
                });
            }
        }

        let object_ty = self.infer_expr(object)?;
        let ty = match prop {
            MemberProp::Name(name) => self.lookup_member(&object_ty, name, pos)?,
            MemberProp::Computed(index) => {
                let index_ty = self.infer_expr(index)?;
                match &object_ty {
                    Type::Array(inner) if assignable(&index_ty, &Type::Number) => (**inner).clone(),
                    Type::String if assignable(&index_ty, &Type::Number) => Type::String,
                    Type::Array(_) | Type::String => {
                        return Err(self.type_error(pos, &index_ty, &Type::Number));
                    }
                    _ => Type::Any,
                }
            }
        };
        if optional {
            Ok(union(vec![ty, Type::Null]))
        } else {
            Ok(ty)
        }
    }

    pub(crate) fn lookup_member(&self, ty: &Type, name: &str, pos: Pos) -> Result<Type, VeyError> {
        match ty {
            Type::Any => Ok(Type::Any),
            Type::EnumNamespace(enum_name) => {
                let info = self
                    .enums
                    .get(enum_name)
                    .ok_or_else(|| self.error(pos, format!("unknown enum '{}'", enum_name)))?;
                let variant = info
                    .variants
                    .iter()
                    .find(|v| v.name == name)
                    .ok_or_else(|| {
                        self.error(
                            pos,
                            format!("enum '{}' has no variant '{}'", enum_name, name),
                        )
                    })?;
                if variant.fields.is_empty() {
                    Ok(Type::Enum(enum_name.clone(), name.to_string()))
                } else {
                    let mut params = Vec::new();
                    for field in &variant.fields {
                        params.push(match &field.ty {
                            Some(ty) => self.resolve_type(ty)?,
                            None => Type::Any,
                        });
                    }
                    Ok(Type::Function(FunctionType {
                        params,
                        required: variant.fields.len(),
                        rest: false,
                        ret: Box::new(Type::Enum(enum_name.clone(), name.to_string())),
                        checked: true,
                        constraints: Vec::new(),
                        type_params: Vec::new(),
                    }))
                }
            }
            Type::Enum(enum_name, variant) => {
                let info = self
                    .enums
                    .get(enum_name)
                    .ok_or_else(|| self.error(pos, format!("unknown enum '{}'", enum_name)))?;
                let v = info
                    .variants
                    .iter()
                    .find(|v| v.name == *variant)
                    .ok_or_else(|| {
                        self.error(
                            pos,
                            format!("enum '{}' has no variant '{}'", enum_name, variant),
                        )
                    })?;
                v.fields
                    .iter()
                    .find(|f| f.name == name)
                    .map(|f| match &f.ty {
                        Some(ty) => self.resolve_type(ty),
                        None => Ok(Type::Any),
                    })
                    .unwrap_or(Ok(Type::Any))
            }
            Type::Object(fields) | Type::Named(_, fields) => fields
                .iter()
                .find(|field| field.name == name)
                .map(|field| field.ty.clone())
                .ok_or_else(|| {
                    self.error(
                        pos,
                        format!("type {} has no field '{}'", ty.display(), name),
                    )
                }),
            Type::Array(_) if name == "length" => Ok(Type::Number),
            Type::String if name == "length" => Ok(Type::Number),
            Type::Union(items) => {
                let mut out = Vec::new();
                for item in items {
                    if matches!(item, Type::Null | Type::Void) {
                        continue;
                    }
                    match self.lookup_member(item, name, pos) {
                        Ok(ty) => out.push(ty),
                        Err(_) => return Ok(Type::Any),
                    }
                }
                if out.is_empty() {
                    Ok(Type::Null)
                } else {
                    Ok(union(out))
                }
            }
            // The values of the language have no fields of their own.
            Type::Number
            | Type::Bool
            | Type::Null
            | Type::Void
            | Type::Never
            | Type::Array(_)
            | Type::String => Err(self.error(
                pos,
                format!("type {} has no field '{}'", ty.display(), name),
            )),
            _ => Ok(Type::Any),
        }
    }
}
